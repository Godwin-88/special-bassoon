use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Represents a user's allocation across chains, protocols, and asset pairs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Portfolio {
    pub id: String,
    pub user_id: String,
    pub created_at: DateTime<Utc>,
    pub last_rebalanced: Option<DateTime<Utc>>,
    pub total_value_usd: f64,
    pub allocations: Vec<Allocation>,
    pub metrics: PortfolioMetrics,
}

/// Individual allocation to a specific protocol/strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Allocation {
    pub id: String,
    pub chain: String,                           // "solana", "ethereum", "arbitrum", etc.
    pub protocol: String,                        // "uniswap_v3", "aave", "curve", etc.
    pub pool_id: Option<String>,                 // e.g., Uniswap V3 pool address
    pub strategy: String,                        // "stablecoin_carry", "delta_neutral_yield", etc.
    pub amount_usd: f64,
    pub percentage: f64,                         // % of total portfolio
    pub entry_price: f64,
    pub entry_time: DateTime<Utc>,
    pub current_value_usd: f64,
    pub unrealized_pnl_usd: f64,
    pub apy: f64,                                // Current APY in basis points
    pub risk_score: f64,                         // 0.0-1.0 (0=low, 1=high risk)
}

/// Portfolio-level performance metrics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PortfolioMetrics {
    pub sortino_ratio: f64,                      // Primary KPI: downside risk focus
    pub sharpe_ratio: f64,
    pub total_apy: f64,                          // Blended APY across allocations
    pub max_drawdown_pct: f64,                   // Worst peak-to-trough % loss
    pub win_rate: f64,                           // % of profitable trades
    pub current_gas_spent_usd: f64,              // Cumulative gas costs (current period)
    pub cumulative_slippage_pct: f64,            // Total slippage incurred
    pub realized_pnl_usd: f64,
    pub unrealized_pnl_usd: f64,
    pub days_in_deployment: u32,

    // Advanced Risk Metrics (Phase 2)
    pub expected_shortfall: f64,                 // Tail risk measure (EVT-based)
    pub contagion_risk_index: f64,               // Systemic fragility (Graph-based)
    pub liquidity_instability_index: f64,        // Probabilistic liquidity stress
}

impl Portfolio {
    pub fn new(user_id: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            user_id,
            created_at: Utc::now(),
            last_rebalanced: None,
            total_value_usd: 0.0,
            allocations: vec![],
            metrics: PortfolioMetrics::default(),
        }
    }

    pub fn total_allocation_pct(&self) -> f64 {
        self.allocations.iter().map(|a| a.percentage).sum()
    }

    pub fn rebalance_time(&self) -> Option<DateTime<Utc>> {
        self.last_rebalanced
    }
}

/// Asset type for allocations
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum AssetType {
    Stablecoin,
    LiquidStaking,
    Governance,
    DeFiLPToken,
}

/// Bridge route information for cross-chain moves
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeRoute {
    pub source_chain: String,
    pub dest_chain: String,
    pub bridge_name: String,                     // "stargate", "wormhole", "hyperlane"
    pub estimated_fee_usd: f64,
    pub estimated_time_seconds: u32,
    pub liquidity_available: f64,
}
