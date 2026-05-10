use solfest_core::BridgeRoute;
use anyhow::{Result, anyhow};
use reqwest::{Client, header::{HeaderMap, HeaderValue}};
use serde_json::Value;
use std::env;

pub struct BridgeRouter {
    client: Client,
    api_key: String,
    base_url: String,
}

impl BridgeRouter {
    pub fn new() -> Self {
        let api_key = env::var("LIFI_API_KEY").unwrap_or_else(|_| "mock".to_string());
        let base_url = env::var("LIFI_BASE_URL").unwrap_or_else(|_| "https://api.li.finance/v1".to_string());
        Self {
            client: Client::new(),
            api_key,
            base_url,
        }
    }

    /// Find best bridge route between chains using LI.FI API
    pub async fn find_best_route(&self, source: &str, dest: &str, amount: f64) -> Result<BridgeRoute> {
        let url = format!("{}/quote", self.base_url);
        
        // Map common chain names to LI.FI chain IDs
        let from_chain = self.get_chain_id(source)?;
        let to_chain = self.get_chain_id(dest)?;
        
        let mut headers = HeaderMap::new();
        headers.insert("x-api-key", HeaderValue::from_str(&self.api_key)?);

        let params = [
            ("fromChain", from_chain),
            ("toChain", to_chain),
            ("fromToken", "USDC"),
            ("toToken", "USDC"),
            ("fromAmount", &amount.to_string()),
        ];

        let response = self.client.get(&url)
            .headers(headers)
            .query(&params)
            .send()
            .await?
            .json::<Value>()
            .await?;

        // Parse response (Simplified logic)
        let estimation = response.get("estimate")
            .ok_or_else(|| anyhow!("Failed to get route estimate"))?;

        Ok(BridgeRoute {
            source_chain: source.to_string(),
            dest_chain: dest.to_string(),
            bridge_name: estimation["tool"].as_str().unwrap_or("li.fi").to_string(),
            estimated_fee_usd: estimation["feeCosts"][0]["amountUSD"].as_str().unwrap_or("0.0").parse()?,
            estimated_time_seconds: estimation["executionDuration"].as_u64().unwrap_or(300) as u32,
            liquidity_available: 1_000_000.0, // LI.FI API provides depth in other endpoints
        })
    }

    fn get_chain_id(&self, chain: &str) -> Result<&'static str> {
        match chain.to_lowercase().as_str() {
            "solana" => Ok("SOL"),
            "ethereum" => Ok("ETH"),
            "arbitrum" => Ok("ARB"),
            "base" => Ok("BASE"),
            _ => Err(anyhow!("Unsupported chain: {}", chain)),
        }
    }
}
