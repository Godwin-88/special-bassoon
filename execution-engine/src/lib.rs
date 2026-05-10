//! # Execution Engine
//!
//! Orchestrates portfolio rebalancing, LI.FI bridging, and execution on-chain.
//! Enforces constraints and handles MEV avoidance.

pub mod portfolio_rebalancer;
pub mod bridge_router;
pub mod constraint_checker;
pub mod execution;

pub use portfolio_rebalancer::*;
pub use bridge_router::*;
pub use constraint_checker::*;
pub use execution::*;
