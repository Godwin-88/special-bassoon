use chrono::{DateTime, Utc};
use ndarray::Array1;
use serde::{Deserialize, Serialize};

/// RL State vector = on-chain signals + graph embeddings + social signals
/// Total dimensions: ~256
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RLState {
    /// On-chain metrics (128-dim):
    /// TVL, APY, liquidity depth, slippage curves, volatility, utilization
    pub on_chain_metrics: Array1<f64>,

    /// Protocol/asset embeddings (64-dim):
    /// From Neo4j graph embeddings: protocol similarity, risk correlation
    pub graph_embeddings: Array1<f64>,

    /// Social/whale signals (64-dim):
    /// Twitter sentiment, whale movements, governance events, funding rates
    pub signal_features: Array1<f64>,

    pub timestamp: DateTime<Utc>,
}

impl RLState {
    /// Construct state vector as flat tensor [on_chain (128), embeddings (64), signals (64)]
    pub fn to_tensor(&self) -> Array1<f64> {
        let mut tensor = Array1::zeros(256);
        tensor.slice_mut(s![0..128]).assign(&self.on_chain_metrics);
        tensor.slice_mut(s![128..192]).assign(&self.graph_embeddings);
        tensor.slice_mut(s![192..256]).assign(&self.signal_features);
        tensor
    }

    pub fn from_tensor(tensor: &Array1<f64>) -> Self {
        assert_eq!(tensor.len(), 256, "State tensor must be 256-dimensional");
        
        Self {
            on_chain_metrics: tensor.slice(s![0..128]).to_owned(),
            graph_embeddings: tensor.slice(s![128..192]).to_owned(),
            signal_features: tensor.slice(s![192..256]).to_owned(),
            timestamp: Utc::now(),
        }
    }
}

/// On-chain market metrics component
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnChainMetrics {
    pub tvl_usd: f64,                            // Total Value Locked in protocol
    pub current_apy: f64,                        // Current annual percentage yield
    pub slippage_bps: f64,                       // Slippage in basis points for typical trade
    pub liquidity_depth: f64,                    // Available liquidity at 0.5% slippage
    pub volatility_30d: f64,                     // 30-day rolling volatility
    pub utilization_ratio: f64,                  // For lending: borrow/supply ratio
    pub price_usd: f64,                          // Token price in USD
    pub daily_volume: f64,                       // Daily trading volume
}

/// Social and whale movement signals + Advanced Risk Scores
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocialSignals {
    pub twitter_sentiment: f64,                  // -1.0 (negative) to 1.0 (positive)
    pub governance_activity: f64,                // 0.0-1.0, activity index
    pub whale_inflow: f64,                       // +/- flow from top 10 wallets
    pub defi_risk_score: f64,                    // 0.0-1.0 (derived from graph)
    pub oracle_freshness: i32,                   // Seconds since last price update
    
    // Phase 2 Risk Additions
    pub expected_shortfall: f64,                 // EVT-based tail risk
    pub contagion_index: f64,                    // Graph-based systemic risk
    pub liquidity_stress: f64,                   // Dynamic Bayesian stress score
}

/// Raw state components before aggregation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateComponents {
    pub on_chain: OnChainMetrics,
    pub social: SocialSignals,
    pub graph_embeddings: Vec<f64>,              // Protocol/risk embeddings
    pub portfolio_current_value: f64,
    pub portfolio_previous_value: f64,
    pub time_since_last_rebalance_hours: f64,
    pub risk_metrics: RiskMetrics,               // Component-level risk metrics
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RiskMetrics {
    pub evt_volatility: f64,
    pub absorption_ratio: f64,
    pub systemic_fragility: f64,
    pub bridge_failure_prob: f64,
}

use ndarray::s;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_tensor_roundtrip() {
        let state = RLState {
            on_chain_metrics: Array1::ones(128),
            graph_embeddings: Array1::zeros(64),
            signal_features: Array1::ones(64) * 0.5,
            timestamp: Utc::now(),
        };

        let tensor = state.to_tensor();
        assert_eq!(tensor.len(), 256);
        
        let reconstructed = RLState::from_tensor(&tensor);
        assert_eq!(reconstructed.on_chain_metrics.len(), 128);
        assert_eq!(reconstructed.graph_embeddings.len(), 64);
        assert_eq!(reconstructed.signal_features.len(), 64);
    }
}
