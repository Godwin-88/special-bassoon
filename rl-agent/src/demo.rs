//! Simplified PPO Training Demo
//!
//! Demonstrates the core RL training logic without heavy dependencies.
//! This can be run to validate the algorithm structure.

use std::collections::VecDeque;

/// Simplified state representation
#[derive(Debug, Clone)]
pub struct SimpleState {
    pub portfolio_value: f64,
    pub market_condition: f64, // -1 to 1
}

#[derive(Debug, Clone)]
pub struct SimpleAction {
    pub allocation: f64, // 0 to 1
}

#[derive(Debug, Clone)]
pub struct Experience {
    pub state: SimpleState,
    pub action: SimpleAction,
    pub reward: f64,
    pub next_state: SimpleState,
    pub done: bool,
}

/// Simplified PPO Policy (placeholder for actual neural network)
pub struct SimplePPOPolicy {
    pub weights: Vec<f64>, // Simplified weights
}

impl SimplePPOPolicy {
    pub fn new() -> Self {
        Self {
            weights: vec![0.1, 0.2, 0.3, 0.4], // Dummy weights
        }
    }

    /// Sample action from policy
    pub fn sample_action(&self, state: &SimpleState) -> SimpleAction {
        // Simple linear policy: allocation = sigmoid(w0 * value + w1 * condition + w2)
        let logit = self.weights[0] * state.portfolio_value / 10000.0
                  + self.weights[1] * state.market_condition
                  + self.weights[2];

        let allocation = 1.0 / (1.0 + (-logit).exp()); // sigmoid

        SimpleAction {
            allocation: allocation.clamp(0.0, 1.0),
        }
    }

    /// Update policy with PPO
    pub fn update(&mut self, experiences: &[Experience], learning_rate: f64) {
        for experience in experiences {
            // Simplified policy gradient update
            let action = self.sample_action(&experience.state);
            let advantage = experience.reward; // Simplified

            // Update weights based on advantage
            let grad = advantage * action.allocation * (1.0 - action.allocation);

            self.weights[0] += learning_rate * grad * experience.state.portfolio_value / 10000.0;
            self.weights[1] += learning_rate * grad * experience.state.market_condition;
            self.weights[2] += learning_rate * grad;
        }
    }
}

/// Simplified environment for testing
pub struct SimpleEnvironment {
    pub current_value: f64,
    pub market_trend: f64,
    pub steps: usize,
}

impl SimpleEnvironment {
    pub fn new() -> Self {
        Self {
            current_value: 10000.0,
            market_trend: 0.0,
            steps: 0,
        }
    }

    pub fn reset(&mut self) {
        self.current_value = 10000.0;
        self.market_trend = 0.0;
        self.steps = 0;
    }

    pub fn step(&mut self, action: &SimpleAction) -> (SimpleState, f64, bool) {
        // Simulate market movement
        let market_change = self.market_trend + (rand::random::<f64>() - 0.5) * 0.1;
        self.market_trend = market_change * 0.9; // Momentum

        // Portfolio return based on allocation
        let portfolio_return = action.allocation * market_change * 0.8 // Beta < 1
                             + (1.0 - action.allocation) * 0.02 / 52.0; // Risk-free return

        self.current_value *= (1.0 + portfolio_return);

        let state = SimpleState {
            portfolio_value: self.current_value,
            market_condition: self.market_trend,
        };

        // Reward is Sharpe-like: return / volatility (simplified)
        let reward = portfolio_return * 10.0; // Scale up for learning

        self.steps += 1;
        let done = self.steps >= 52; // 1 year of weekly steps

        (state, reward, done)
    }

    pub fn get_state(&self) -> SimpleState {
        SimpleState {
            portfolio_value: self.current_value,
            market_condition: self.market_trend,
        }
    }
}

/// Training demonstration
pub fn demo_training() {
    println!("🤖 PPO Training Demo");
    println!("===================");

    let mut policy = SimplePPOPolicy::new();
    let mut env = SimpleEnvironment::new();

    let episodes = 100;
    let learning_rate = 0.01;

    for episode in 0..episodes {
        env.reset();
        let mut episode_reward = 0.0;
        let mut experiences: VecDeque<Experience> = VecDeque::new();

        // Run episode
        loop {
            let state = env.get_state();
            let action = policy.sample_action(&state);
            let (next_state, reward, done) = env.step(&action);

            experiences.push_back(Experience {
                state,
                action,
                reward,
                next_state: next_state.clone(),
                done,
            });

            episode_reward += reward;

            if done {
                break;
            }
        }

        // Update policy
        let experience_vec: Vec<Experience> = experiences.iter().cloned().collect();
        policy.update(&experience_vec, learning_rate);

        if (episode + 1) % 20 == 0 {
            println!("Episode {}: Total Reward = {:.2}, Final Value = {:.0}",
                    episode + 1, episode_reward, env.current_value);
        }
    }

    println!("\n✅ Training complete!");
    println!("Final policy weights: {:?}", policy.weights);

    // Test final policy
    env.reset();
    let mut total_return = 0.0;
    for _ in 0..52 {
        let state = env.get_state();
        let action = policy.sample_action(&state);
        let (_, reward, _) = env.step(&action);
        total_return += reward;
    }

    println!("Final policy total return: {:.2}", total_return);
    println!("Final portfolio value: {:.0}", env.current_value);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_ppo() {
        let mut policy = SimplePPOPolicy::new();
        let initial_weights = policy.weights.clone();

        let mut env = SimpleEnvironment::new();
        let state = env.get_state();
        let action = policy.sample_action(&state);

        assert!(action.allocation >= 0.0 && action.allocation <= 1.0);

        // Test update
        let experiences = vec![Experience {
            state,
            action,
            reward: 1.0,
            next_state: env.get_state(),
            done: false,
        }];

        policy.update(&experiences, 0.1);
        assert_ne!(policy.weights, initial_weights);
    }
}