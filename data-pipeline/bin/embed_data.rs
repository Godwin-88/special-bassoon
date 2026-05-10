/// Compute and persist graph embeddings from Neo4j.
///
/// Run with:
///   cargo run --bin embed_data
///
/// Optional: set INGEST=1 to pull live DeFiLlama data into Neo4j first.
///   INGEST=1 cargo run --bin embed_data

use data_pipeline::ProtocolIngester;
use neo4j_graph::{GraphClient, GraphEmbeddings};
use anyhow::Result;
use std::{env, sync::Arc};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let uri      = env::var("NEO4J_URI").unwrap_or_else(|_| "bolt://localhost:7687".into());
    let user     = env::var("NEO4J_USER").unwrap_or_else(|_| "neo4j".into());
    let password = env::var("NEO4J_PASSWORD").unwrap_or_else(|_| "password".into());
    let defillama = env::var("DEFILLAMA_API_URL").unwrap_or_else(|_| "https://api.llama.fi".into());
    let lifi_key  = env::var("LIFI_API_KEY").unwrap_or_default();
    let out_path  = env::var("EMBEDDINGS_OUT").unwrap_or_else(|_| "embeddings.json".into());

    println!("Connecting to Neo4j at {}", uri);
    let client = Arc::new(GraphClient::connect(&uri, &user, &password).await?);

    // Optional live ingestion: pull DeFiLlama → Neo4j before computing embeddings
    if env::var("INGEST").as_deref() == Ok("1") {
        println!("Ingesting live protocol data from DeFiLlama...");
        let ingester = ProtocolIngester::new(Arc::clone(&client), &defillama, &lifi_key).await?;
        let count = ingester.ingest().await?;
        println!("Ingested {} protocols into Neo4j", count);

        println!("Ingesting LI.FI routes...");
        if let Err(e) = ingester.ingest_lifi_routes().await {
            eprintln!("LI.FI ingest warning: {}", e);
        }
    }

    println!("Computing graph embeddings...");
    let mut embeddings = GraphEmbeddings::new();
    embeddings.compute_embeddings(&client).await?;

    let n_proto = embeddings.protocol_embeddings.len();
    let n_risk  = embeddings.risk_embeddings.len();
    println!("Computed {} protocol embeddings, {} risk embeddings", n_proto, n_risk);

    // Serialise to JSON for disk persistence
    let payload = serde_json::json!({
        "protocol_embeddings": embeddings.protocol_embeddings
            .iter()
            .map(|(k, v)| (k.clone(), v.to_vec()))
            .collect::<std::collections::HashMap<String, Vec<f64>>>(),
        "risk_embeddings": embeddings.risk_embeddings
            .iter()
            .map(|(k, v)| (k.clone(), v.to_vec()))
            .collect::<std::collections::HashMap<String, Vec<f64>>>(),
    });

    std::fs::write(&out_path, serde_json::to_string_pretty(&payload)?)?;
    println!("Embeddings written to {}", out_path);

    // Sample output
    if let Some((name, vec)) = embeddings.protocol_embeddings.iter().next() {
        let preview: Vec<f64> = vec.iter().take(8).copied().collect();
        println!("Sample — '{}': {:?} ...(64 dims)", name, preview);
    }

    Ok(())
}
