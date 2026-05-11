use solfest_core::SocialSignals;
use anyhow::Result;
use reqwest::Client;
use serde_json::Value;
use tracing::info;

pub struct SocialSignalsAggregator {
    http_client: Client,
    twitter_token: String,
}

impl SocialSignalsAggregator {
    pub fn new(twitter_token: String) -> Self {
        Self {
            http_client: Client::new(),
            twitter_token,
        }
    }

    /// Fetch aggregate social signals for a protocol
    pub async fn fetch_signals(&self, protocol: &str) -> Result<SocialSignals> {
        info!("Aggregating social signals for {}", protocol);
        
        let twitter_sentiment = self.fetch_twitter_sentiment(protocol).await?;
        let gov_activity = self.fetch_governance_activity(protocol).await?;
        
        // Advanced Risk Metrics (Phase 3 placeholders for Bayesian/EVT logic)
        Ok(SocialSignals {
            twitter_sentiment,
            governance_activity: gov_activity,
            whale_inflow: 0.0, // Updated by WhaleTracker
            defi_risk_score: 0.1,
            oracle_freshness: 60,
            expected_shortfall: 0.05,
            contagion_index: 0.02,
            liquidity_stress: 0.1,
        })
    }

    async fn fetch_twitter_sentiment(&self, protocol: &str) -> Result<f64> {
        let request = self.http_client
            .get("https://api.twitter.com/2/tweets/search/recent")
            .bearer_auth(&self.twitter_token)
            .query(&[("query", protocol), ("max_results", "10")])
            .build()?;

        info!("Building Twitter request: {}", request.url());
        let dummy: Value = serde_json::json!({"sentiment": 0.35});
        Ok(dummy["sentiment"].as_f64().unwrap_or(0.0))
    }

    async fn fetch_governance_activity(&self, protocol: &str) -> Result<f64> {
        let request = self.http_client
            .get("https://hub.snapshot.org/graphql")
            .bearer_auth(&self.twitter_token)
            .query(&[("query", protocol)])
            .build()?;

        info!("Building governance request: {}", request.url());
        let dummy: Value = serde_json::json!({"governance_activity": 0.6});
        Ok(dummy["governance_activity"].as_f64().unwrap_or(0.0))
    }
}
