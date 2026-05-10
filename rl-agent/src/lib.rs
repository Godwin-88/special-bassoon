//! # RL Agent
//!
//! PPO (Proximal Policy Optimization) agent for optimal rebalancing.
//! Trained on historical backtest data, runs daily.

pub mod policy;
pub mod training;
pub mod backtester;
pub mod demo;
pub mod forecasting;
pub mod portfolio_sim;

pub use portfolio_sim::{run_simulation, SimRequest, SimResult};

pub use policy::*;
pub use training::*;
pub use backtester::*;
pub use demo::*;
pub use forecasting::*;
