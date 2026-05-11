use solfest_core::{RLState, OnChainMetrics, SocialSignals, StateComponents, RiskMetrics};
use neo4j_graph::embeddings::GraphEmbeddings;
use crate::on_chain::OnChainClient;
use crate::social_signals::SocialSignalsAggregator;
use crate::whale_tracking::WhaleTracker;
use ndarray::Array1;
use chrono::Utc;
use anyhow::Result;
use std::collections::HashMap;
use tracing::info;

/// StateConstructor assembles the 256-dim RLState vector
pub struct StateConstructor {
    graph_embeddings: GraphEmbeddings,
    on_chain_client: OnChainClient,
    social_aggregator: SocialSignalsAggregator,
    whale_tracker: WhaleTracker,
}

impl StateConstructor {
    pub fn new(
        embeddings: GraphEmbeddings,
        on_chain: OnChainClient,
        social: SocialSignalsAggregator,
        whale: WhaleTracker,
    ) -> Self {
        Self {
            graph_embeddings: embeddings,
            on_chain_client: on_chain,
            social_aggregator: social,
            whale_tracker: whale,
        }
    }

    /// Construct the full 256-dim RLState for live operations
    pub async fn construct_live_state(
        &self, 
        chain: &str,
        protocol_name: &str, 
        portfolio_value: f64, 
        prev_value: f64, 
        hours: f64
    ) -> Result<RLState> {
        // 1. Get On-Chain Metrics from QuickNode
        let on_chain = self.on_chain_client.fetch_protocol_metrics(chain, protocol_name).await?;

        // 2. Get Social Signals & Whale Movements
        let mut social = self.social_aggregator.fetch_signals(protocol_name).await?;
        social.whale_inflow = self.whale_tracker.track_movements(protocol_name).await?;

        let components = StateComponents {
            on_chain,
            social,
            graph_embeddings: self.graph_embeddings.get_protocol_embedding(protocol_name)
                .unwrap_or_else(|| Array1::zeros(64))
                .to_vec(),
            portfolio_current_value: portfolio_value,
            portfolio_previous_value: prev_value,
            time_since_last_rebalance_hours: hours,
            risk_metrics: RiskMetrics::default(), // Populated by systemic risk engine
        };

        self.construct_from_components(&components, protocol_name)
    }

    /// Construct RLState from pre-fetched components (used by backtester)
    pub fn construct_from_components(
        &self,
        components: &StateComponents,
        protocol_name: &str,
    ) -> Result<RLState> {
        let on_chain_vec = self.on_chain_to_vector(&components.on_chain);
        
        // Add portfolio context to on-chain vector
        info!(protocol = protocol_name, "Constructing RL state from components");

        let mut final_on_chain = on_chain_vec;
        final_on_chain[8] = components.portfolio_current_value.ln_1p();
        final_on_chain[9] = (components.portfolio_current_value / components.portfolio_previous_value).ln_1p();
        final_on_chain[10] = components.time_since_last_rebalance_hours / 24.0;

        let graph_vec = Array1::from_vec(components.graph_embeddings.clone());
        let social_vec = self.social_to_vector(&components.social);

        Ok(RLState {
            on_chain_metrics: final_on_chain,
            graph_embeddings: graph_vec,
            signal_features: social_vec,
            timestamp: Utc::now(),
        })
    }

    /// Simplified version for BacktestEnvironment alignment
    pub fn construct_state(
        &self,
        on_chain_map: &HashMap<String, OnChainMetrics>,
        social: &SocialSignals,
        portfolio_value: f64,
        prev_value: f64,
        hours: f64,
    ) -> Result<RLState> {
        // For backtesting, we might need a specific protocol or an aggregate.
        // Assuming we pick one for now or handle aggregation.
        let protocol_name = on_chain_map.keys().next()
            .map(|s| s.as_str())
            .unwrap_or("unknown");
            
        let on_chain = on_chain_map.get(protocol_name)
            .cloned()
            .unwrap_or_else(|| OnChainMetrics {
                tvl_usd: 0.0,
                current_apy: 0.0,
                slippage_bps: 0.0,
                liquidity_depth: 0.0,
                volatility_30d: 0.0,
                utilization_ratio: 0.0,
                price_usd: 0.0,
                daily_volume: 0.0,
            });

        let components = StateComponents {
            on_chain,
            social: social.clone(),
            graph_embeddings: self.graph_embeddings.get_protocol_embedding(protocol_name)
                .unwrap_or_else(|| Array1::zeros(64))
                .to_vec(),
            portfolio_current_value: portfolio_value,
            portfolio_previous_value: prev_value,
            time_since_last_rebalance_hours: hours,
            risk_metrics: RiskMetrics::default(),
        };

        self.construct_from_components(&components, protocol_name)
    }

    /// Map OnChainMetrics to 128-dim vector
    fn on_chain_to_vector(&self, metrics: &OnChainMetrics) -> Array1<f64> {
        let mut v = Array1::zeros(128);
        v[0] = metrics.tvl_usd.ln_1p();
        v[1] = metrics.current_apy;
        v[2] = metrics.slippage_bps / 100.0;
        v[3] = metrics.liquidity_depth.ln_1p();
        v[4] = metrics.volatility_30d;
        v[5] = metrics.utilization_ratio;
        v[6] = metrics.price_usd;
        v[7] = metrics.daily_volume.ln_1p();
        v
    }

    /// Map SocialSignals and Risk Scores to 64-dim vector
    fn social_to_vector(&self, signals: &SocialSignals) -> Array1<f64> {
        let mut v = Array1::zeros(64);
        v[0] = signals.twitter_sentiment;
        v[1] = signals.governance_activity;
        v[2] = signals.whale_inflow.signum() * signals.whale_inflow.abs().ln_1p();
        v[3] = signals.defi_risk_score;
        v[4] = signals.oracle_freshness as f64 / 3600.0;
        
        // Phase 2 Risk Scores
        v[5] = signals.expected_shortfall;
        v[6] = signals.contagion_index;
        v[7] = signals.liquidity_stress;
        v
    }
}
