use solfest_core::OnChainMetrics;
use solana_client::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use anyhow::{Result, anyhow};
use serde_json::Value;
use reqwest::Client;
use std::collections::HashMap;
use tracing::{info, warn};

pub struct OnChainClient {
    solana_rpc: Option<RpcClient>,
    evm_rpcs: HashMap<String, String>,
    http_client: Client,
}

impl OnChainClient {
    pub fn new() -> Self {
        // In production, URLs would be loaded from env
        Self {
            solana_rpc: None,
            evm_rpcs: HashMap::new(),
            http_client: Client::new(),
        }
    }

    pub fn with_solana(mut self, url: &str) -> Self {
        self.solana_rpc = Some(RpcClient::new(url.to_string()));
        self
    }

    pub fn with_evm(mut self, chain: &str, url: &str) -> Self {
        self.evm_rpcs.insert(chain.to_string(), url.to_string());
        self
    }

    /// Fetch metrics for a specific protocol on a given chain
    pub async fn fetch_protocol_metrics(&self, chain: &str, protocol: &str) -> Result<OnChainMetrics> {
        match chain {
            "solana" => self.fetch_solana_metrics(protocol).await,
            "ethereum" | "base" | "arbitrum" => self.fetch_evm_metrics(chain, protocol).await,
            _ => Err(anyhow!("Unsupported chain: {}", chain)),
        }
    }

    async fn fetch_solana_metrics(&self, protocol: &str) -> Result<OnChainMetrics> {
        let _rpc = self.solana_rpc.as_ref().ok_or_else(|| anyhow!("Solana RPC not configured"))?;
        
        // Example: Fetch TVL from a known program/account for the protocol
        // In reality, we'd have a mapping of protocol -> accounts
        info!("Fetching Solana metrics for {}", protocol);
        
        // Mocking the extraction logic for Phase 3 prototype
        Ok(OnChainMetrics {
            tvl_usd: 500_000_000.0,
            current_apy: 0.045,
            slippage_bps: 12.0,
            liquidity_depth: 100_000_000.0,
            volatility_30d: 0.25,
            utilization_ratio: 0.8,
            price_usd: 1.0,
            daily_volume: 50_000_000.0,
        })
    }

    async fn fetch_evm_metrics(&self, chain: &str, protocol: &str) -> Result<OnChainMetrics> {
        let rpc_url = self.evm_rpcs.get(chain).ok_or_else(|| anyhow!("{} RPC not configured", chain))?;
        
        info!("Fetching {} metrics for {} via QuickNode", chain, protocol);
        
        // Phase 3: Implement JSON-RPC calls for specific protocols
        // e.g., Aave V3 data provider for lending rates, Uniswap V3 pools for liquidity
        
        // Fallback to high-level aggregator if direct RPC fails or for complex metrics (TVL)
        self.fetch_from_aggregator(chain, protocol).await
    }

    async fn fetch_from_aggregator(&self, _chain: &str, protocol: &str) -> Result<OnChainMetrics> {
        // Using DeFiLlama as a reliable source for TVL and APY as configured in .env
        let url = format!("https://api.llama.fi/protocol/{}", protocol.to_lowercase().replace("_", "-"));
        
        let resp = self.http_client.get(&url).send().await?.json::<Value>().await?;
        
        let tvl = resp["tvl"].as_f64().unwrap_or(0.0);
        
        // Construct metrics with best available data
        Ok(OnChainMetrics {
            tvl_usd: tvl,
            current_apy: 0.05, // Would be fetched from a separate /yields endpoint
            slippage_bps: 15.0,
            liquidity_depth: tvl * 0.1, // Heuristic
            volatility_30d: 0.1,
            utilization_ratio: 0.5,
            price_usd: 1.0,
            daily_volume: 0.0,
        })
    }
}
