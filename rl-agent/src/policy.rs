//! PPO (Proximal Policy Optimization) policy network
//!
//! Implements the core RL algorithm for optimal portfolio rebalancing.
//! Uses tch-rs (Rust bindings for PyTorch) for neural network operations when
//! the "torch" feature is enabled; otherwise falls back to random sampling.

use solfest_core::{RLState, RLAction, ExecutionConstraints};
use ndarray::Array1;
use anyhow::Result;
use itertools::Itertools;
use rand::Rng;

/// Operating mode of the policy — supports cold-start random sampling without a model file.
enum PolicyMode {
    #[cfg(feature = "torch")]
    /// Loaded from a TorchScript .pt model file (production inference)
    Loaded(tch::CModule),
    /// Uniform random sampling for cold-start / untrained initialization
    Random,
}

/// PPO Policy Network
/// Architecture: 256-dim state → 512 → 512 → 720 actions (discrete)
pub struct PPOPolicy {
    mode: PolicyMode,

    /// Action space size (720 discrete actions)
    action_space_size: usize,

    /// State space size (256 dimensions)
    state_space_size: usize,

    /// PPO hyperparameters
    clip_ratio: f64,
    value_coeff: f64,
    entropy_coeff: f64,
}

impl PPOPolicy {
    /// Load PPO policy from TorchScript .pt file (production inference).
    /// Only available when compiled with the "torch" feature.
    #[cfg(feature = "torch")]
    pub fn load_from_file(model_path: &str) -> Result<Self> {
        let model = tch::CModule::load(model_path)?;
        Ok(Self {
            mode: PolicyMode::Loaded(model),
            action_space_size: 720,
            state_space_size: 256,
            clip_ratio: 0.2,
            value_coeff: 0.5,
            entropy_coeff: 0.01,
        })
    }

    /// Create an untrained random policy for cold-start training.
    /// Actions are sampled uniformly; value estimates return 0.0.
    pub fn new() -> Result<Self> {
        Ok(Self {
            mode: PolicyMode::Random,
            action_space_size: 720,
            state_space_size: 256,
            clip_ratio: 0.2,
            value_coeff: 0.5,
            entropy_coeff: 0.01,
        })
    }

    /// Sample an action and its value from the policy given a state
    pub fn sample_action_and_value(&self, state: &RLState, constraints: &ExecutionConstraints) -> Result<(RLAction, f64)> {
        match &self.mode {
            #[cfg(feature = "torch")]
            PolicyMode::Loaded(model) => {
                let state_tensor = state.to_tensor();
                let input = tch::Tensor::from_slice(&state_tensor.as_slice().unwrap())
                    .view([1, self.state_space_size as i64]);
                let output = model.forward_ts(&[input])?;
                let (logits, value_tensor) = if let tch::IValue::Tuple(t) = output {
                    (t[0].to_tensor(), t[1].to_tensor())
                } else {
                    return Err(anyhow::anyhow!("Unexpected model output shape"));
                };
                let action_idx = {
                    let probs = logits.softmax(-1, tch::Kind::Float);
                    let sample = probs.multinomial(1, false);
                    i64::try_from(sample.int64_value(&[]))? as usize
                };
                let action = RLAction::from_action_index(action_idx)
                    .ok_or_else(|| anyhow::anyhow!("Invalid action index: {}", action_idx))?;
                self.validate_action(&action, constraints)?;
                let value: f64 = value_tensor.double_value(&[]);
                Ok((action, value))
            }
            PolicyMode::Random => {
                let action_idx = rand::thread_rng().gen_range(0..self.action_space_size);
                let action = RLAction::from_action_index(action_idx)
                    .ok_or_else(|| anyhow::anyhow!("Invalid random action index: {}", action_idx))?;
                Ok((action, 0.0))
            }
        }
    }

    /// Get value estimate for a state (Critic output); returns 0.0 in Random mode
    pub fn get_value(&self, state: &RLState) -> Result<f64> {
        match &self.mode {
            #[cfg(feature = "torch")]
            PolicyMode::Loaded(model) => {
                let state_tensor = state.to_tensor();
                let input = tch::Tensor::from_slice(&state_tensor.as_slice().unwrap())
                    .view([1, self.state_space_size as i64]);
                let output = model.forward_ts(&[input])?;
                let value_tensor = if let tch::IValue::Tuple(t) = output {
                    t[1].to_tensor()
                } else {
                    tch::Tensor::from(0.0f64)
                };
                Ok(value_tensor.double_value(&[]))
            }
            PolicyMode::Random => Ok(0.0),
        }
    }

    /// Get action probabilities for all actions.
    /// In Random mode returns uniform distribution over 720 actions.
    pub fn get_action_probabilities(&self, state: &RLState) -> Result<Array1<f64>> {
        match &self.mode {
            #[cfg(feature = "torch")]
            PolicyMode::Loaded(model) => {
                let state_tensor = state.to_tensor();
                let input = tch::Tensor::from_slice(&state_tensor.as_slice().unwrap())
                    .view([1, self.state_space_size as i64]);
                let logits = model.forward_ts(&[input])?;
                let probs = logits.softmax(-1, tch::Kind::Float);
                let probs_vec: Vec<f64> = probs.into();
                Ok(Array1::from_vec(probs_vec))
            }
            PolicyMode::Random => {
                let uniform = 1.0 / self.action_space_size as f64;
                Ok(Array1::from_elem(self.action_space_size, uniform))
            }
        }
    }

    /// Validate action against execution constraints
    fn validate_action(&self, action: &RLAction, constraints: &ExecutionConstraints) -> Result<()> {
        if !action.protocol.supported_chains().contains(&action.chain) {
            return Err(anyhow::anyhow!(
                "Protocol {:?} not supported on chain {:?}",
                action.protocol, action.chain
            ));
        }
        Ok(())
    }

    /// Calculate PPO loss with academic rigor (Schulman et al. 2017)
    ///
    /// L^CLIP(θ) = E_t[min(r_t(θ)·Â_t, clip(r_t(θ), 1-ε, 1+ε)·Â_t)]
    pub fn calculate_ppo_loss(
        &self,
        states: &[RLState],
        actions: &[RLAction],
        old_log_probs: &[f64],
        advantages: &[f64],
        returns: &[f64],
        clip_ratio: f64,
    ) -> Result<f64> {
        if states.is_empty() { return Ok(0.0); }

        let mean_adv = advantages.iter().sum::<f64>() / advantages.len() as f64;
        let var_adv = advantages.iter().map(|a| (a - mean_adv).powi(2)).sum::<f64>() / advantages.len() as f64;
        let std_adv = (var_adv + 1e-8).sqrt();
        let normalized_advantages: Vec<f64> = advantages.iter()
            .map(|a| (a - mean_adv) / (std_adv + 1e-8))
            .collect();

        let mut policy_loss_sum = 0.0;
        let mut value_loss_sum = 0.0;
        let mut entropy_loss_sum = 0.0;
        let batch_size = states.len();

        for (state, action, old_log_prob, norm_advantage, return_val) in
            itertools::izip!(states, actions, old_log_probs, &normalized_advantages, returns) {

            let current_probs = self.get_action_probabilities(state)?;
            let action_idx = action.to_action_index();
            let action_prob = current_probs[action_idx].max(1e-8);
            let current_log_prob = action_prob.ln();

            let ratio = (current_log_prob - old_log_prob).exp();
            let clipped_ratio = ratio.clamp(1.0 - clip_ratio, 1.0 + clip_ratio);
            let surrogate1 = ratio * norm_advantage;
            let surrogate2 = clipped_ratio * norm_advantage;
            policy_loss_sum += -surrogate1.min(surrogate2);

            let current_value = self.get_value(state).unwrap_or(0.0);
            value_loss_sum += 0.5 * (return_val - current_value).powi(2);

            let entropy: f64 = current_probs.iter()
                .filter(|&&p| p > 1e-8)
                .map(|&p| -p * p.ln())
                .sum();
            entropy_loss_sum += -self.entropy_coeff * entropy;
        }

        let avg_policy_loss = policy_loss_sum / batch_size as f64;
        let avg_value_loss = value_loss_sum / batch_size as f64;
        let avg_entropy_loss = entropy_loss_sum / batch_size as f64;

        Ok(avg_policy_loss + self.value_coeff * avg_value_loss + avg_entropy_loss)
    }

    pub fn export_for_inference(&self) -> Result<Vec<u8>> {
        Ok(vec![])
    }
}

/// PPO Training Configuration
#[derive(Debug, Clone)]
pub struct PPOConfig {
    pub learning_rate: f64,
    pub batch_size: usize,
    pub epochs_per_update: usize,
    pub clip_ratio: f64,
    pub value_coeff: f64,
    pub entropy_coeff: f64,
    pub max_grad_norm: f64,
    pub gamma: f64,
    pub lambda: f64,
}

impl Default for PPOConfig {
    fn default() -> Self {
        Self {
            learning_rate: 3e-4,
            batch_size: 64,
            epochs_per_update: 10,
            clip_ratio: 0.2,
            value_coeff: 0.5,
            entropy_coeff: 0.01,
            max_grad_norm: 0.5,
            gamma: 0.99,
            lambda: 0.95,
        }
    }
}

/// Training statistics
#[derive(Debug, Clone, Default)]
pub struct TrainingStats {
    pub episode_count: usize,
    pub total_steps: usize,
    pub average_reward: f64,
    pub average_sortino: f64,
    pub max_drawdown: f64,
    pub policy_loss: f64,
    pub value_loss: f64,
    pub entropy: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use solfest_core::{Chain, Protocol, AllocationPercentage};

    #[test]
    fn test_action_indexing_consistency() {
        let action = RLAction {
            chain: Chain::Ethereum,
            protocol: Protocol::Aave,
            allocation_pct: AllocationPercentage::Pct50,
            timestamp: chrono::Utc::now(),
        };

        let idx = action.to_action_index();
        let reconstructed = RLAction::from_action_index(idx).unwrap();

        assert_eq!(reconstructed.chain, action.chain);
        assert_eq!(reconstructed.protocol, action.protocol);
        assert_eq!(reconstructed.allocation_pct, action.allocation_pct);
    }

    #[test]
    fn test_action_space_size() {
        assert_eq!(Chain::all().len(), 4);
        assert_eq!(Protocol::all().len(), 9);
        assert_eq!(AllocationPercentage::all().len(), 20);
        assert_eq!(4 * 9 * 20, 720);
    }
}
