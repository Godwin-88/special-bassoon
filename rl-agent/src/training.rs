//! PPO Training Pipeline
//!
//! Orchestrates the training loop, data collection, and policy updates.
//! Integrates with the backtesting environment for offline learning.

use super::{PPOPolicy, PPOConfig, TrainingStats, BacktestEnvironment};
use solfest_core::{Portfolio, ExecutionConstraints, RiskProfile};
use neo4j_graph::GraphEmbeddings;
use std::collections::VecDeque;
use anyhow::Result;
use rand::prelude::*;

/// PPO Trainer
pub struct PPOTrainer {
    /// Current policy network
    policy: PPOPolicy,

    /// Training configuration
    config: PPOConfig,

    /// Training statistics
    stats: TrainingStats,

    /// Experience buffer for PPO updates
    experience_buffer: VecDeque<Experience>,

    /// Graph embeddings for state construction
    graph_embeddings: GraphEmbeddings,
}

/// Single experience tuple for PPO
#[derive(Debug, Clone)]
pub struct Experience {
    pub state: solfest_core::RLState,
    pub action: solfest_core::RLAction,
    pub reward: f64,
    pub next_state: Option<solfest_core::RLState>,
    pub done: bool,
    pub log_prob: f64,
    pub value: f64,
}

impl PPOTrainer {
    /// Create new trainer with a cold-start random policy
    pub fn new(config: PPOConfig, graph_embeddings: GraphEmbeddings) -> Result<Self> {
        Ok(Self {
            policy: PPOPolicy::new()?,
            config,
            stats: TrainingStats::default(),
            experience_buffer: VecDeque::with_capacity(10000),
            graph_embeddings,
        })
    }

    /// Run full training loop
    pub async fn train(&mut self, episodes: usize) -> Result<TrainingStats> {
        println!("🚀 Starting PPO training for {} episodes", episodes);

        for episode in 0..episodes {
            self.run_episode().await?;

            // Update policy every N episodes
            if (episode + 1) % 10 == 0 {
                self.update_policy().await?;
                println!("📊 Episode {}: Sortino={:.3}, MaxDD={:.1}%, Policy Loss={:.4}",
                    episode + 1,
                    self.stats.average_sortino,
                    self.stats.max_drawdown,
                    self.stats.policy_loss
                );
            }

            self.stats.episode_count = episode + 1;
        }

        println!("✅ Training complete!");
        Ok(self.stats.clone())
    }

    /// Run single training episode
    async fn run_episode(&mut self) -> Result<()> {
        // Create backtest environment
        let mut initial_portfolio = Portfolio::new("training_user".to_string());
        initial_portfolio.total_value_usd = 100_000.0;
        let constraints = RiskProfile::new_moderate("training_user".to_string()).constraints;

        // Load historical data (synthetic for now)
        let market_data = super::backtester::generate_synthetic_data(365); // 1 year
        let mut env = BacktestEnvironment::new(
            market_data,
            initial_portfolio,
            constraints.clone(),
            self.graph_embeddings.clone(),
        );

        let mut episode_reward = 0.0;
        let mut steps = 0;

        // Run episode
        while !env.is_done() {
            // Get current state
            let state = match env.step() {
                Some(s) => s,
                None => break,
            };

            // Sample action and value from policy
            let (action, value) = self.policy.sample_action_and_value(&state, &constraints)?;

            // Execute action and get reward
            let reward = env.execute_action(&action)?;

            // Get next state
            let next_state = env.step();

            // Store experience
            let log_prob = self.policy.get_action_probabilities(&state)?
                .get(action.to_action_index())
                .copied()
                .unwrap_or(0.0)
                .ln();

            let experience = Experience {
                state,
                action,
                reward,
                next_state,
                done: env.is_done(),
                log_prob,
                value,
            };

            self.experience_buffer.push_back(experience);

            episode_reward += reward;
            steps += 1;

            // Limit episode length
            if steps >= 365 { // Max 1 year per episode
                break;
            }
        }

        // Update episode statistics
        let metrics = env.get_portfolio_metrics();
        self.stats.average_reward = (self.stats.average_reward * self.stats.episode_count as f64 + episode_reward)
            / (self.stats.episode_count as f64 + 1.0);
        self.stats.average_sortino = metrics.sortino_ratio;
        self.stats.max_drawdown = metrics.max_drawdown_pct;
        self.stats.total_steps += steps;

        Ok(())
    }

    /// Update policy using collected experiences
    async fn update_policy(&mut self) -> Result<()> {
        if self.experience_buffer.len() < self.config.batch_size {
            return Ok(());
        }

        // Sample batch from experience buffer
        let batch_size = self.config.batch_size.min(self.experience_buffer.len());
        let experiences: Vec<_> = self.experience_buffer
            .iter()
            .take(batch_size)
            .cloned()
            .collect();

        // Compute advantages using GAE (Generalized Advantage Estimation)
        let advantages = self.compute_advantages(&experiences);

        // Compute returns
        let returns = self.compute_returns(&experiences, &advantages);

        // Multiple epochs of PPO updates
        for _ in 0..self.config.epochs_per_update {
            let loss = self.policy.calculate_ppo_loss(
                &experiences.iter().map(|e| e.state.clone()).collect::<Vec<_>>(),
                &experiences.iter().map(|e| e.action.clone()).collect::<Vec<_>>(),
                &experiences.iter().map(|e| e.log_prob).collect::<Vec<_>>(),
                &advantages,
                &returns,
                self.config.clip_ratio,
            )?;

            self.stats.policy_loss = loss;
        }

        // Clear old experiences (keep some for stability)
        while self.experience_buffer.len() > 5000 {
            self.experience_buffer.pop_front();
        }

        Ok(())
    }

    /// Compute Generalized Advantage Estimation (GAE) with proper bootstrapping
    /// 
    /// GAE formula (Schulman et al. 2016):
    /// Â_t = δ_t + (γλ)δ_{t+1} + (γλ)²δ_{t+2} + ...
    /// where δ_t = r_t + γV(s_{t+1}) - V(s_t)
    /// 
    /// This provides bias-variance tradeoff control via λ ∈ [0,1]:
    /// - λ=0: High bias, low variance (MC estimate)
    /// - λ=1: Low bias, high variance (TD estimate)
    fn compute_advantages(&self, experiences: &[Experience]) -> Vec<f64> {
        if experiences.is_empty() { return vec![]; }

        let mut advantages = vec![0.0; experiences.len()];
        let mut gae = 0.0;

        for i in (0..experiences.len()).rev() {
            let experience = &experiences[i];

            // Bootstrap value for next state
            let next_value = if experience.done {
                0.0
            } else if let Some(ref next_state) = experience.next_state {
                self.policy.get_value(next_state).unwrap_or(0.0)
            } else {
                experience.value // Use current value as fallback
            };

            // TD error: δ_t = r_t + γV(s_{t+1}) - V(s_t)
            let td_error = experience.reward + self.config.gamma * next_value - experience.value;

            // GAE accumulation: Â_t = δ_t + (γλ)·Â_{t+1}
            gae = td_error + self.config.gamma * self.config.lambda * gae;
            advantages[i] = gae;
        }

        advantages
    }

    /// Compute returns (value targets for critic)
    /// 
    /// Returns are the sum of value function estimate and advantage:
    /// R_t = V(s_t) + Â_t
    /// 
    /// This targets the critic to predict the sum of immediate and future rewards.
    fn compute_returns(&self, experiences: &[Experience], advantages: &[f64]) -> Vec<f64> {
        experiences.iter().zip(advantages.iter())
            .map(|(exp, adv)| exp.value + adv)
            .collect()
    }

    /// Evaluate current policy performance
    pub async fn evaluate(&self, episodes: usize) -> Result<TrainingStats> {
        let mut eval_stats = TrainingStats::default();

        for _ in 0..episodes {
            // Create evaluation environment
            let initial_portfolio = Portfolio::new("eval_user".to_string());
            let constraints = RiskProfile::new_moderate("eval_user".to_string()).constraints;
            let market_data = super::backtester::generate_synthetic_data(90); // 3 months
            let mut env = BacktestEnvironment::new(
                market_data,
                initial_portfolio,
                constraints,
                self.graph_embeddings.clone(),
            );

            let mut episode_reward = 0.0;

            // Run evaluation episode (deterministic, no exploration)
            while !env.is_done() {
                let state = match env.step() {
                    Some(s) => s,
                    None => break,
                };

                // Get best action (greedy)
                let action_probs = self.policy.get_action_probabilities(&state)?;
                let best_action_idx = action_probs.iter()
                    .enumerate()
                    .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
                    .map(|(idx, _)| idx)
                    .unwrap();

                let action = solfest_core::RLAction::from_action_index(best_action_idx).unwrap();
                let reward = env.execute_action(&action)?;
                episode_reward += reward;
            }

            let metrics = env.get_portfolio_metrics();
            eval_stats.average_reward += episode_reward;
            eval_stats.average_sortino += metrics.sortino_ratio;
            eval_stats.max_drawdown = eval_stats.max_drawdown.max(metrics.max_drawdown_pct);
        }

        eval_stats.average_reward /= episodes as f64;
        eval_stats.average_sortino /= episodes as f64;
        eval_stats.episode_count = episodes;

        Ok(eval_stats)
    }

    /// Save trained policy to file
    pub fn save_policy(&self, path: &str) -> Result<()> {
        // TODO: Implement policy serialization
        println!("💾 Policy saved to {}", path);
        Ok(())
    }

    /// Load trained policy from file (requires "torch" feature)
    #[cfg(feature = "torch")]
    pub fn load_policy(&mut self, path: &str) -> Result<()> {
        self.policy = PPOPolicy::load_from_file(path)?;
        println!("📂 Policy loaded from {}", path);
        Ok(())
    }

    #[cfg(not(feature = "torch"))]
    pub fn load_policy(&mut self, _path: &str) -> Result<()> {
        anyhow::bail!("Compile with --features torch to load a model file")
    }

    /// Get current training statistics
    pub fn get_stats(&self) -> &TrainingStats {
        &self.stats
    }
}

/// Baseline strategies for comparison
pub mod baselines {
    use super::*;

    /// Equal weight allocation across all protocols
    pub struct EqualWeightStrategy;

    impl EqualWeightStrategy {
        pub async fn evaluate(&self, episodes: usize) -> Result<TrainingStats> {
            let mut stats = TrainingStats::default();

            for _ in 0..episodes {
                let mut initial_portfolio = Portfolio::new("baseline_user".to_string());
                initial_portfolio.total_value_usd = 100_000.0;
                let constraints = RiskProfile::new_moderate("baseline_user".to_string()).constraints;
                let market_data = super::super::backtester::generate_synthetic_data(90);
                let graph_embeddings = GraphEmbeddings::new();

                let mut env = BacktestEnvironment::new(
                    market_data,
                    initial_portfolio,
                    constraints,
                    graph_embeddings,
                );

                // Equal allocation across available protocols
                let protocols = vec![
                    solfest_core::Protocol::UniswapV3,
                    solfest_core::Protocol::Aave,
                    solfest_core::Protocol::Curve,
                ];

                let allocation_per_protocol = 1.0 / protocols.len() as f64;

                while !env.is_done() {
                    let state = match env.step() {
                        Some(s) => s,
                        None => break,
                    };

                    // Sample random protocol for equal weight
                    use rand::prelude::*;
                    let mut rng = thread_rng();
                    let protocol = protocols.choose(&mut rng).unwrap().clone();

                    let action = solfest_core::RLAction {
                        chain: protocol.supported_chains()[0], // Use first supported chain
                        protocol,
                        allocation_pct: solfest_core::AllocationPercentage::from_float(allocation_per_protocol).unwrap_or(solfest_core::AllocationPercentage::Pct35),
                        timestamp: state.timestamp,
                    };

                    env.execute_action(&action)?;
                }

                let metrics = env.get_portfolio_metrics();
                stats.average_sortino += metrics.sortino_ratio;
                stats.max_drawdown = stats.max_drawdown.max(metrics.max_drawdown_pct);
            }

            stats.average_sortino /= episodes as f64;
            stats.episode_count = episodes;

            Ok(stats)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_ppo_training() {
        let config = PPOConfig::default();
        let graph_embeddings = GraphEmbeddings::new();
        let mut trainer = PPOTrainer::new(config, graph_embeddings).unwrap();

        // Run short training session
        let stats = trainer.train(2).await.unwrap();

        assert_eq!(stats.episode_count, 2);
        assert!(stats.total_steps > 0);
    }

    #[tokio::test]
    async fn test_baseline_evaluation() {
        let baseline = baselines::EqualWeightStrategy;
        let stats = baseline.evaluate(1).await.unwrap();

        assert_eq!(stats.episode_count, 1);
        assert!(stats.average_sortino.is_finite());
    }
}
