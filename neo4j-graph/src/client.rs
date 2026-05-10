use anyhow::Result;
use neo4rs::{Graph, Row, ConfigBuilder};
use std::sync::Arc;
use tracing::{info, debug};

/// Neo4j graph client for knowledge base queries
pub struct GraphClient {
    graph: Arc<Graph>,
}

impl GraphClient {
    /// Initialize connection to Neo4j at bolt://localhost:7687
    pub async fn connect(uri: &str, user: &str, password: &str) -> Result<Self> {
        info!("Connecting to Neo4j at {}", uri);
        
        let config = ConfigBuilder::default()
            .uri(uri)
            .user(user)
            .password(password)
            .build()?;
            
        let graph = Graph::connect(config).await?;
        
        // Verify connection
        let hello = graph.run(neo4rs::query("RETURN 1 as status")).await?;
        debug!("Neo4j connection established: {:?}", hello);
        
        Ok(Self {
            graph: Arc::new(graph),
        })
    }

    /// Get all DeFi protocols from graph
    pub async fn get_all_protocols(&self) -> Result<Vec<ProtocolNode>> {
        let query = neo4rs::query(
            "MATCH (p:DeFiProtocol)
             RETURN p.name as name, p.category as category, p.chain as chain,
                    p.description as description, p.tvl_peak_usd as tvl_usd"
        );
        
        let mut result = self.graph.execute(query).await?;
        let mut protocols = vec![];
        
        while let Ok(Some(row)) = result.next().await {
            if let Ok(protocol) = ProtocolNode::from_row(&row) {
                protocols.push(protocol);
            }
        }
        
        Ok(protocols)
    }

    /// Get protocol risks from graph
    pub async fn get_protocol_risks(&self, protocol_name: &str) -> Result<Vec<RiskNode>> {
        let query = neo4rs::query(
            "MATCH (p:DeFiProtocol {name: $name})-[:IMPLEMENTS|CAUSES|RELATED_TO]->
                   (r:TransactConcept)
             WHERE r.category CONTAINS 'risk'
             RETURN r.name as name, r.definition as definition, r.category as category"
        ).param("name", protocol_name);
        
        let mut result = self.graph.execute(query).await?;
        let mut risks = vec![];
        
        while let Ok(Some(row)) = result.next().await {
            if let Ok(risk) = RiskNode::from_row(&row) {
                risks.push(risk);
            }
        }
        
        Ok(risks)
    }

    /// Get strategy prerequisites from graph
    pub async fn get_strategy_prerequisites(&self, strategy_name: &str) -> Result<Vec<ConceptNode>> {
        let query = neo4rs::query(
            "MATCH (s:TradingStrategy {name: $name})-[:APPLIED_IN|USES]->
                   (c:TransactConcept)
             RETURN c.name as name, c.definition as definition,
                    c.category as category, c.difficulty as difficulty"
        ).param("name", strategy_name);
        
        let mut result = self.graph.execute(query).await?;
        let mut concepts = vec![];
        
        while let Ok(Some(row)) = result.next().await {
            if let Ok(concept) = ConceptNode::from_row(&row) {
                concepts.push(concept);
            }
        }
        
        Ok(concepts)
    }

    /// Get DeFi equivalent of traditional finance concept
    pub async fn get_defi_equivalent(&self, concept_name: &str) -> Result<Option<ConceptNode>> {
        let query = neo4rs::query(
            "MATCH (c1:TransactConcept {name: $name})-[:HAS_DEFI_EQUIVALENT]->(c2:TransactConcept)
             RETURN c2.name as name, c2.definition as definition, c2.category as category"
        ).param("name", concept_name);
        
        let mut result = self.graph.execute(query).await?;
        
        if let Ok(Some(row)) = result.next().await {
            return Ok(Some(ConceptNode::from_row(&row)?));
        }
        
        Ok(None)
    }

    /// Get risk correlation matrix (which risks are correlated)
    pub async fn get_risk_correlations(&self) -> Result<Vec<RiskCorrelation>> {
        let query = neo4rs::query(
            "MATCH (r1:TransactConcept)-[:CAUSES|MITIGATES|CORRELATES_WITH]->
                   (r2:TransactConcept)
             WHERE r1.category CONTAINS 'risk' AND r2.category CONTAINS 'risk'
             RETURN r1.name as from_risk, r2.name as to_risk"
        );
        
        let mut result = self.graph.execute(query).await?;
        let mut correlations = vec![];
        
        while let Ok(Some(row)) = result.next().await {
            if let Ok(corr) = RiskCorrelation::from_row(&row) {
                correlations.push(corr);
            }
        }
        
        Ok(correlations)
    }

    /// Get mapping of protocols to the concepts they implement
    pub async fn get_protocol_concept_mapping(&self) -> Result<Vec<(String, String)>> {
        let query = neo4rs::query(
            "MATCH (p:DeFiProtocol)-[:IMPLEMENTS]->(c:TransactConcept)
             RETURN p.name as protocol, c.name as concept"
        );
        
        let mut result = self.graph.execute(query).await?;
        let mut mapping = vec![];
        
        while let Ok(Some(row)) = result.next().await {
            mapping.push((row.get("protocol")?, row.get("concept")?));
        }
        
        Ok(mapping)
    }

    /// Get mapping of concepts to their categories
    pub async fn get_concept_categories(&self) -> Result<Vec<(String, String)>> {
        let query = neo4rs::query(
            "MATCH (c:TransactConcept)
             RETURN c.name as name, c.category as category"
        );

        let mut result = self.graph.execute(query).await?;
        let mut mapping = vec![];

        while let Ok(Some(row)) = result.next().await {
            mapping.push((row.get("name")?, row.get("category")?));
        }

        Ok(mapping)
    }

    /// Upsert a DeFiProtocol node, its Chain relationships, and a ProtocolSnapshot.
    #[allow(clippy::too_many_arguments)]
    pub async fn upsert_protocol(
        &self,
        slug: &str,
        name: &str,
        category: &str,
        logo: &str,
        url: &str,
        tvl: f64,
        tvl_1d_pct: f64,
        tvl_7d_pct: f64,
        chains: &[String],
        fees_24h: Option<f64>,
        fees_7d: Option<f64>,
        rev_24h: Option<f64>,
        rev_7d: Option<f64>,
        vol_24h: Option<f64>,
        vol_7d: Option<f64>,
    ) -> Result<()> {
        // Upsert the protocol node
        let q = neo4rs::query(
            "MERGE (p:DeFiProtocol {slug: $slug})
             SET p.name       = $name,
                 p.category   = $category,
                 p.logo       = $logo,
                 p.url        = $url,
                 p.tvl        = $tvl,
                 p.tvl_1d_pct = $tvl_1d_pct,
                 p.tvl_7d_pct = $tvl_7d_pct,
                 p.updated_at = datetime()"
        )
        .param("slug", slug)
        .param("name", name)
        .param("category", category)
        .param("logo", logo)
        .param("url", url)
        .param("tvl", tvl)
        .param("tvl_1d_pct", tvl_1d_pct)
        .param("tvl_7d_pct", tvl_7d_pct);

        self.graph.run(q).await?;

        // Upsert Chain nodes and DEPLOYED_ON edges
        for chain in chains {
            let q2 = neo4rs::query(
                "MERGE (c:Chain {name: $chain})
                 WITH c
                 MATCH (p:DeFiProtocol {slug: $slug})
                 MERGE (p)-[:DEPLOYED_ON]->(c)"
            )
            .param("chain", chain.as_str())
            .param("slug", slug);
            self.graph.run(q2).await?;
        }

        // Create ProtocolSnapshot
        let snap = neo4rs::query(
            "MATCH (p:DeFiProtocol {slug: $slug})
             CREATE (s:ProtocolSnapshot {
               tvl:         $tvl,
               fees_24h:    $fees_24h,
               fees_7d:     $fees_7d,
               rev_24h:     $rev_24h,
               rev_7d:      $rev_7d,
               vol_24h:     $vol_24h,
               vol_7d:      $vol_7d,
               captured_at: datetime()
             })
             MERGE (p)-[:HAS_SNAPSHOT]->(s)"
        )
        .param("slug", slug)
        .param("tvl", tvl)
        .param("fees_24h", fees_24h.unwrap_or(0.0))
        .param("fees_7d", fees_7d.unwrap_or(0.0))
        .param("rev_24h", rev_24h.unwrap_or(0.0))
        .param("rev_7d", rev_7d.unwrap_or(0.0))
        .param("vol_24h", vol_24h.unwrap_or(0.0))
        .param("vol_7d", vol_7d.unwrap_or(0.0));

        self.graph.run(snap).await?;

        Ok(())
    }

    /// Upsert a Chain node (used by LI.FI ingester).
    pub async fn upsert_chain(&self, name: &str) -> Result<()> {
        let q = neo4rs::query("MERGE (c:Chain {name: $name})")
            .param("name", name);
        self.graph.run(q).await?;
        Ok(())
    }

    /// Upsert a Bridge node and BRIDGES_TO edges between supported chains.
    pub async fn upsert_bridge(&self, name: &str, supported_chains: &[String]) -> Result<()> {
        let q = neo4rs::query("MERGE (b:Bridge {name: $name})")
            .param("name", name);
        self.graph.run(q).await?;

        for chain in supported_chains {
            let q2 = neo4rs::query(
                "MERGE (c:Chain {name: $chain})
                 WITH c
                 MATCH (b:Bridge {name: $bridge})
                 MERGE (b)-[:SUPPORTS_CHAIN]->(c)"
            )
            .param("chain", chain.as_str())
            .param("bridge", name);
            self.graph.run(q2).await?;
        }
        Ok(())
    }

    /// Query the latest snapshot per protocol for the analytics endpoint.
    pub async fn get_analytics_protocols(&self) -> Result<Vec<AnalyticsProtocolRow>> {
        let query = neo4rs::query(
            "MATCH (p:DeFiProtocol)
             OPTIONAL MATCH (p)-[:HAS_SNAPSHOT]->(s:ProtocolSnapshot)
             WITH p, s ORDER BY s.captured_at DESC
             WITH p, collect(s)[0] AS latest
             OPTIONAL MATCH (p)-[:DEPLOYED_ON]->(c:Chain)
             RETURN p.slug        AS slug,
                    p.name        AS name,
                    p.logo        AS logo,
                    p.url         AS url,
                    p.category    AS category,
                    p.tvl         AS tvl,
                    p.tvl_1d_pct  AS tvl_change_1d,
                    p.tvl_7d_pct  AS tvl_change_7d,
                    collect(c.name) AS chains,
                    latest.fees_24h AS fees_24h,
                    latest.fees_7d  AS fees_7d,
                    latest.rev_24h  AS rev_24h,
                    latest.rev_7d   AS rev_7d,
                    latest.vol_24h  AS vol_24h,
                    latest.vol_7d   AS vol_7d
             ORDER BY p.tvl DESC
             LIMIT 300"
        );

        let mut result = self.graph.execute(query).await?;
        let mut rows = vec![];

        while let Ok(Some(row)) = result.next().await {
            rows.push(AnalyticsProtocolRow {
                slug: row.get("slug").unwrap_or_default(),
                name: row.get("name").unwrap_or_default(),
                logo: row.get("logo").unwrap_or_default(),
                url: row.get("url").unwrap_or_default(),
                category: row.get("category").unwrap_or_else(|_| "Other".to_string()),
                tvl: row.get("tvl").unwrap_or(0.0),
                tvl_change_1d: row.get("tvl_change_1d").unwrap_or(0.0),
                tvl_change_7d: row.get("tvl_change_7d").unwrap_or(0.0),
                chains: row.get("chains").unwrap_or_default(),
                fees_24h: row.get("fees_24h").ok(),
                fees_7d: row.get("fees_7d").ok(),
                revenue_24h: row.get("rev_24h").ok(),
                revenue_7d: row.get("rev_7d").ok(),
                volume_24h: row.get("vol_24h").ok(),
                volume_7d: row.get("vol_7d").ok(),
            });
        }

        Ok(rows)
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AnalyticsProtocolRow {
    pub slug: String,
    pub name: String,
    pub logo: String,
    pub url: String,
    pub category: String,
    pub tvl: f64,
    pub tvl_change_1d: f64,
    pub tvl_change_7d: f64,
    pub chains: Vec<String>,
    pub fees_24h: Option<f64>,
    pub fees_7d: Option<f64>,
    pub revenue_24h: Option<f64>,
    pub revenue_7d: Option<f64>,
    pub volume_24h: Option<f64>,
    pub volume_7d: Option<f64>,
}

// ── Node Types ────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProtocolNode {
    pub name: String,
    pub category: String,
    pub chain: String,
    pub description: Option<String>,
    pub tvl_usd: Option<String>,
}

impl ProtocolNode {
    pub fn from_row(row: &Row) -> Result<Self> {
        Ok(Self {
            name: row.get("name")?,
            category: row.get("category")?,
            chain: row.get("chain")?,
            description: row.get("description").ok(),
            tvl_usd: row.get("tvl_usd").ok(),
        })
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RiskNode {
    pub name: String,
    pub definition: Option<String>,
    pub category: String,
}

impl RiskNode {
    pub fn from_row(row: &Row) -> Result<Self> {
        Ok(Self {
            name: row.get("name")?,
            definition: row.get("definition").ok(),
            category: row.get("category")?,
        })
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ConceptNode {
    pub name: String,
    pub definition: Option<String>,
    pub category: String,
    pub difficulty: Option<String>,
}

impl ConceptNode {
    pub fn from_row(row: &Row) -> Result<Self> {
        Ok(Self {
            name: row.get("name")?,
            definition: row.get("definition").ok(),
            category: row.get("category")?,
            difficulty: row.get("difficulty").ok(),
        })
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RiskCorrelation {
    pub from_risk: String,
    pub to_risk: String,
}

impl RiskCorrelation {
    pub fn from_row(row: &Row) -> Result<Self> {
        Ok(Self {
            from_risk: row.get("from_risk")?,
            to_risk: row.get("to_risk")?,
        })
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SourceNode {
    pub id: String,
    pub title: String,
    pub author: Option<String>,
    pub year: Option<i32>,
}

impl SourceNode {
    pub fn from_row(row: &Row) -> Result<Self> {
        Ok(Self {
            id: row.get("id")?,
            title: row.get("title")?,
            author: row.get("author").ok(),
            year: row.get("year").ok(),
        })
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProtocolContextEntry {
    pub name: String,
    pub category: String,
    pub description: Option<String>,
    pub mechanism: Option<String>,
    pub fee_tiers: Option<String>,
    pub interest_rate_model: Option<String>,
    pub launched: Option<i64>,
    pub concepts: Vec<String>,
}

impl GraphClient {
    pub async fn get_protocol_context_batch(&self, names: &[String]) -> Result<Vec<ProtocolContextEntry>> {
        if names.is_empty() { return Ok(vec![]); }
        let query = neo4rs::query(
            "MATCH (p:DeFiProtocol)
             WHERE p.name IN $names
             OPTIONAL MATCH (p)-[:IMPLEMENTS]->(c:TransactConcept)
             RETURN p.name AS name, p.category AS category,
                    p.description AS description,
                    p.invariant AS mechanism,
                    p.fee_tiers AS fee_tiers,
                    p.interest_rate_model AS irm,
                    p.launched AS launched,
                    collect(c.name) AS concepts"
        ).param("names", names.to_vec());

        let mut result = self.graph.execute(query).await?;
        let mut entries = vec![];
        while let Ok(Some(row)) = result.next().await {
            entries.push(ProtocolContextEntry {
                name: row.get("name").unwrap_or_default(),
                category: row.get("category").unwrap_or_default(),
                description: row.get("description").ok(),
                mechanism: row.get("mechanism").ok(),
                fee_tiers: row.get("fee_tiers").ok(),
                interest_rate_model: row.get("irm").ok(),
                launched: row.get("launched").ok(),
                concepts: row.get("concepts").unwrap_or_default(),
            });
        }
        Ok(entries)
    }
}
