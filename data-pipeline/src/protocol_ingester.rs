use anyhow::Result;
use neo4j_graph::GraphClient;
use reqwest::Client;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{info, warn};

pub struct ProtocolIngester {
    client: Client,
    graph: Arc<GraphClient>,
    defillama_url: String,
    lifi_key: String,
}

// ── DeFiLlama response types ──────────────────────────────────────────────────

#[derive(Deserialize)]
struct LlamaProtocol {
    slug: Option<String>,
    name: Option<String>,
    logo: Option<String>,
    url: Option<String>,
    category: Option<String>,
    chains: Option<Vec<String>>,
    chain: Option<String>,
    tvl: Option<f64>,
    change_1d: Option<f64>,
    change_7d: Option<f64>,
}

#[derive(Deserialize)]
struct LlamaOverview {
    protocols: Option<Vec<LlamaFeeProtocol>>,
}

#[derive(Deserialize)]
struct LlamaFeeProtocol {
    slug: Option<String>,
    name: Option<String>,
    total24h: Option<f64>,
    total7d: Option<f64>,
    total30d: Option<f64>,
}

// ── LI.FI response types ──────────────────────────────────────────────────────

#[derive(Deserialize)]
struct LiFiChainsResponse {
    chains: Option<Vec<LiFiChain>>,
}

#[derive(Deserialize)]
struct LiFiChain {
    key: Option<String>,
    name: Option<String>,
}

#[derive(Deserialize)]
struct LiFiToolsResponse {
    bridges: Option<Vec<LiFiTool>>,
    exchanges: Option<Vec<LiFiTool>>,
}

#[derive(Deserialize)]
struct LiFiTool {
    key: Option<String>,
    name: Option<String>,
    #[serde(rename = "supportedChains")]
    supported_chains: Option<Vec<String>>,
}

// ── Normalised row for upsert ─────────────────────────────────────────────────

struct ProtocolRow {
    slug: String,
    name: String,
    logo: String,
    url: String,
    category: String,
    chains: Vec<String>,
    tvl: f64,
    tvl_1d_pct: f64,
    tvl_7d_pct: f64,
    fees_24h: Option<f64>,
    fees_7d: Option<f64>,
    rev_24h: Option<f64>,
    rev_7d: Option<f64>,
    vol_24h: Option<f64>,
    vol_7d: Option<f64>,
}

impl ProtocolIngester {
    pub async fn new(graph: Arc<GraphClient>, defillama_url: &str, lifi_key: &str) -> Result<Self> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;
        Ok(Self {
            client,
            graph,
            defillama_url: defillama_url.to_string(),
            lifi_key: lifi_key.to_string(),
        })
    }

    /// Initialise Neo4j schema constraints (idempotent).
    pub async fn init_schema(&self) -> Result<()> {
        info!("Schema constraints assumed present (run cypher/ migrations if needed)");
        Ok(())
    }

    /// Fetch all DeFiLlama data, merge into Neo4j, return count of upserted rows.
    pub async fn ingest(&self) -> Result<usize> {
        info!("Starting DeFiLlama protocol ingestion");

        let base = self.defillama_url.clone();
        let url_protocols = format!("{}/protocols", base);
        let url_fees = format!("{}/overview/fees?excludeTotalDataChart=true&excludeTotalDataChartBreakdown=true", base);
        let url_rev = format!("{}/overview/fees?excludeTotalDataChart=true&excludeTotalDataChartBreakdown=true&dataType=dailyRevenue", base);
        let url_dexs = format!("{}/overview/dexs?excludeTotalDataChart=true&excludeTotalDataChartBreakdown=true", base);

        let (protocols_res, fees_res, rev_res, dexs_res) = tokio::join!(
            self.fetch_json::<Vec<LlamaProtocol>>(&url_protocols),
            self.fetch_json::<LlamaOverview>(&url_fees),
            self.fetch_json::<LlamaOverview>(&url_rev),
            self.fetch_json::<LlamaOverview>(&url_dexs),
        );

        let protocols = protocols_res.unwrap_or_default();

        let mut fees_map: HashMap<String, LlamaFeeProtocol> = HashMap::new();
        if let Ok(ov) = fees_res {
            for p in ov.protocols.unwrap_or_default() {
                let key = p.slug.clone().or_else(|| p.name.clone()).unwrap_or_default().to_lowercase();
                let _fee_total_30d = p.total30d.unwrap_or(0.0);
                if _fee_total_30d > 0.0 {
                    info!("Protocol {} 30d fee total = {}", key, _fee_total_30d);
                }
                fees_map.entry(key).or_insert(p);
            }
        }

        let mut rev_map: HashMap<String, LlamaFeeProtocol> = HashMap::new();
        if let Ok(ov) = rev_res {
            for p in ov.protocols.unwrap_or_default() {
                let key = p.slug.clone().or_else(|| p.name.clone()).unwrap_or_default().to_lowercase();
                rev_map.entry(key).or_insert(p);
            }
        }

        let mut vol_map: HashMap<String, LlamaFeeProtocol> = HashMap::new();
        if let Ok(ov) = dexs_res {
            for p in ov.protocols.unwrap_or_default() {
                let key = p.slug.clone().or_else(|| p.name.clone()).unwrap_or_default().to_lowercase();
                vol_map.entry(key).or_insert(p);
            }
        }

        // Sort by TVL, take top 300
        let mut sorted: Vec<&LlamaProtocol> = protocols.iter()
            .filter(|p| p.tvl.unwrap_or(0.0) > 0.0)
            .collect();
        sorted.sort_by(|a, b| b.tvl.unwrap_or(0.0).partial_cmp(&a.tvl.unwrap_or(0.0)).unwrap_or(std::cmp::Ordering::Equal));
        sorted.truncate(300);

        let rows: Vec<ProtocolRow> = sorted.iter().map(|p| {
            let slug = p.slug.clone().or_else(|| p.name.clone()).unwrap_or_default();
            let key = slug.to_lowercase();
            let name_key = p.name.clone().unwrap_or_default().to_lowercase();

            let fee = fees_map.get(&key).or_else(|| fees_map.get(&name_key));
            let rev = rev_map.get(&key).or_else(|| rev_map.get(&name_key));
            let vol = vol_map.get(&key).or_else(|| vol_map.get(&name_key));

            let chains = p.chains.clone().unwrap_or_else(|| {
                p.chain.clone().map(|c| vec![c]).unwrap_or_else(|| vec!["Unknown".to_string()])
            });

            ProtocolRow {
                slug,
                name: p.name.clone().unwrap_or_default(),
                logo: p.logo.clone().unwrap_or_default(),
                url: p.url.clone().unwrap_or_default(),
                category: p.category.clone().unwrap_or_else(|| "Other".to_string()),
                chains,
                tvl: p.tvl.unwrap_or(0.0),
                tvl_1d_pct: p.change_1d.unwrap_or(0.0),
                tvl_7d_pct: p.change_7d.unwrap_or(0.0),
                fees_24h: fee.and_then(|f| f.total24h),
                fees_7d: fee.and_then(|f| f.total7d),
                rev_24h: rev.and_then(|r| r.total24h),
                rev_7d: rev.and_then(|r| r.total7d),
                vol_24h: vol.and_then(|v| v.total24h),
                vol_7d: vol.and_then(|v| v.total7d),
            }
        }).collect();

        let count = rows.len();
        info!("Upserting {} protocols into Neo4j", count);

        for row in &rows {
            if let Err(e) = self.upsert_protocol(row).await {
                warn!("Failed to upsert {}: {}", row.slug, e);
            }
        }

        info!("Ingestion complete: {} protocols processed", count);
        Ok(count)
    }

    async fn upsert_protocol(&self, row: &ProtocolRow) -> Result<()> {
        self.graph.upsert_protocol(
            &row.slug,
            &row.name,
            &row.category,
            &row.logo,
            &row.url,
            row.tvl,
            row.tvl_1d_pct,
            row.tvl_7d_pct,
            &row.chains,
            row.fees_24h,
            row.fees_7d,
            row.rev_24h,
            row.rev_7d,
            row.vol_24h,
            row.vol_7d,
        ).await
    }

    /// Ingest LI.FI chain and bridge/aggregator data into Neo4j.
    pub async fn ingest_lifi_routes(&self) -> Result<()> {
        info!("Ingesting LI.FI routes");

        let chains_res = self.fetch_json::<serde_json::Value>("https://li.quest/v1/chains").await;
        let tools_res = self.fetch_json::<LiFiToolsResponse>("https://li.quest/v1/tools").await;

        if let Ok(chains_val) = chains_res {
            let chains: Vec<LiFiChain> = serde_json::from_value(
                chains_val.get("chains").cloned().unwrap_or(serde_json::Value::Array(vec![]))
            ).unwrap_or_default();

            for chain in &chains {
                if let Some(key) = &chain.key {
                    info!("LI.FI chain key {} for {}", key, chain.name.clone().unwrap_or_default());
                }
            }

            for chain in &chains {
                let name = chain.name.clone().unwrap_or_default();
                if name.is_empty() { continue; }
                if let Err(e) = self.graph.upsert_chain(&name).await {
                    warn!("Failed to upsert chain {}: {}", name, e);
                }
            }
            info!("Upserted {} LI.FI chains", chains.len());
        }

        if let Ok(tools) = tools_res {
            let bridges = tools.bridges.unwrap_or_default();
            for bridge in &bridges {
                let name = bridge.name.clone().unwrap_or_default();
                if name.is_empty() { continue; }
                if let Some(key) = &bridge.key {
                    info!("LI.FI bridge key {} for {}", key, name);
                }
                let supported: Vec<String> = bridge.supported_chains.clone().unwrap_or_default();
                if let Err(e) = self.graph.upsert_bridge(&name, &supported).await {
                    warn!("Failed to upsert bridge {}: {}", name, e);
                }
            }
            info!("Upserted {} LI.FI bridges", bridges.len());
        }

        Ok(())
    }

    async fn fetch_json<T: serde::de::DeserializeOwned>(&self, url: &str) -> Result<T> {
        let mut req = self.client
            .get(url)
            .header("Accept", "application/json");

        if url.contains("li.quest") && !self.lifi_key.is_empty() {
            req = req.header("x-api-key", self.lifi_key.clone());
        }

        let resp = req.send().await?;
        if !resp.status().is_success() {
            anyhow::bail!("{} → {}", url, resp.status());
        }
        Ok(resp.json::<T>().await?)
    }
}
