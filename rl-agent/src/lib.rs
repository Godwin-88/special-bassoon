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
pub mod causality;
pub mod jump_var;

pub use portfolio_sim::{
    run_simulation, SimRequest, SimResult, SimMetrics,
    UniverseId, StrategyId, PortfolioPoint, AssetInfo,
    build_synthetic_universe, WorkAsset,
    list_catalogue, CatalogueEntry,
};
pub use causality::{
    transfer_entropy, net_information_flow, te_with_significance,
    mutual_information, log_returns, compute_causality_matrix,
    CausalityConfig, CausalityMatrix, CausalLink,
};
pub use jump_var::{
    detect_jumps, jump_adjusted_var, delta_covar,
    JumpDetectorConfig, JumpVarResult, JumpDecomposition,
    normal_quantile, historical_simulation_var,
};

pub use policy::*;
pub use training::*;
pub use backtester::*;
pub use demo::*;
pub use forecasting::*;
