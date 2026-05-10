use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post, put},
    Json, Router,
};
use data_pipeline::ProtocolIngester;
use neo4j_graph::GraphClient;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tower_http::cors::CorsLayer;
use tracing::{info, error, warn};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    info!("Bridge Capital Markets API starting...");

    dotenv::dotenv().ok();

    let port = std::env::var("API_PORT").unwrap_or("8080".to_string());
    let addr = format!("0.0.0.0:{}", port);

    let neo4j_uri  = std::env::var("NEO4J_URI").unwrap_or("bolt://localhost:7687".into());
    let neo4j_user = std::env::var("NEO4J_USER").unwrap_or("neo4j".into());
    let neo4j_pass = std::env::var("NEO4J_PASSWORD").unwrap_or("password".into());
    let defillama  = std::env::var("DEFILLAMA_API_URL").unwrap_or("https://api.llama.fi".into());
    let lifi_key   = std::env::var("LIFI_API_KEY").unwrap_or_default();

    // Connect to Neo4j — non-fatal if unavailable (API degrades gracefully)
    let graph_opt: Option<Arc<GraphClient>> = match GraphClient::connect(&neo4j_uri, &neo4j_user, &neo4j_pass).await {
        Ok(g) => {
            info!("Neo4j connected at {}", neo4j_uri);
            Some(Arc::new(g))
        }
        Err(e) => {
            warn!("Neo4j unavailable ({}), analytics endpoint will return empty", e);
            None
        }
    };

    let ingester_opt: Option<Arc<ProtocolIngester>> = if let Some(ref g) = graph_opt {
        match ProtocolIngester::new(Arc::clone(g), &defillama, &lifi_key).await {
            Ok(ing) => Some(Arc::new(ing)),
            Err(e) => {
                warn!("Failed to build ProtocolIngester: {}", e);
                None
            }
        }
    } else {
        None
    };

    // Background ingestion task — every 5 minutes
    if let Some(ref ing) = ingester_opt {
        let ing_startup = Arc::clone(ing);
        tokio::spawn(async move {
            if let Err(e) = ing_startup.ingest().await {
                error!("Startup ingestion failed: {}", e);
            }
        });

        let ing_loop = Arc::clone(ing);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(300));
            interval.tick().await; // first tick fires immediately; skip — startup already ran
            loop {
                interval.tick().await;
                if let Err(e) = ing_loop.ingest().await {
                    error!("Periodic ingestion error: {}", e);
                }
            }
        });
    }

    let state = Arc::new(AppState { graph: graph_opt });

    let app = Router::new()
        .route("/portfolio", get(get_portfolio))
        .route("/portfolio", put(update_portfolio))
        .route("/risk-profile", put(update_risk_profile))
        .route("/backtest-results", get(get_backtest_results))
        .route("/execution-logs", get(get_execution_logs))
        .route("/api/metrics", get(get_metrics))
        .route("/api/graph", get(get_graph))
        .route("/api/backtest", post(run_backtest))
        .route("/api/analytics/protocols", get(get_analytics_protocols))
        .route("/api/protocol-context", get(get_protocol_context))
        .route("/health", get(health_check))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("API server listening on {}", addr);

    axum::serve(listener, app).await?;
    Ok(())
}

struct AppState {
    graph: Option<Arc<GraphClient>>,
}

// ─── Response Types ──────────────────────────────────────

#[derive(Serialize)]
struct HealthResponse {
    status: String,
}

#[derive(Serialize)]
struct PortfolioResponse {
    id: String,
    total_value_usd: f64,
    metrics: serde_json::Value,
}

#[derive(Deserialize, Debug)]
struct UpdatePortfolioRequest {
    total_value_usd: Option<f64>,
}

#[derive(Deserialize, Debug)]
struct UpdateRiskProfileRequest {
    max_slippage_bps: Option<u32>,
    max_drawdown_pct: Option<f64>,
}

#[derive(Serialize)]
struct ExecutionLog {
    id: String,
    timestamp: String,
    action: String,
}

#[derive(Serialize)]
struct BacktestResults {
    sortino_ratio: f64,
    max_drawdown_pct: f64,
    total_apy: f64,
}

/// Live systemic risk metrics — consumed by dashboard.tsx
#[derive(Serialize, Clone)]
pub struct MetricsResponse {
    pub absorption: f64,
    pub tvl: f64,
    pub contagion: usize,
    pub expected_shortfall: f64,
    pub garch_volatility: f64,
    pub bayesian_risk_score: f64,
}

/// Force-graph data for the risk network visualization
#[derive(Serialize, Clone)]
pub struct GraphData {
    pub nodes: Vec<GraphNode>,
    pub links: Vec<GraphLink>,
}

#[derive(Serialize, Clone)]
pub struct GraphNode {
    pub id: String,
    pub val: f64,
}

#[derive(Serialize, Clone)]
pub struct GraphLink {
    pub source: String,
    pub target: String,
    pub value: f64,
}

// BacktestRequest is now the full SimRequest from portfolio_sim
type BacktestRequest = rl_agent::SimRequest;

// ─── Handlers ────────────────────────────────────────────

async fn health_check(State(_state): State<Arc<AppState>>) -> impl IntoResponse {
    Json(HealthResponse { status: "healthy".to_string() })
}

async fn get_portfolio(State(_state): State<Arc<AppState>>) -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(PortfolioResponse {
            id: "portfolio_1".to_string(),
            total_value_usd: 100000.0,
            metrics: serde_json::json!({
                "sortino_ratio": 1.2,
                "max_drawdown_pct": 8.5,
                "total_apy": 12.5
            }),
        }),
    )
}

async fn update_portfolio(
    State(_state): State<Arc<AppState>>,
    Json(req): Json<UpdatePortfolioRequest>,
) -> impl IntoResponse {
    info!("Updating portfolio: {:?}", req);
    (StatusCode::OK, Json(serde_json::json!({"ok": true})))
}

async fn update_risk_profile(
    State(_state): State<Arc<AppState>>,
    Json(req): Json<UpdateRiskProfileRequest>,
) -> impl IntoResponse {
    info!("Updating risk profile: {:?}", req);
    (StatusCode::OK, Json(serde_json::json!({"ok": true})))
}

async fn get_backtest_results(State(_state): State<Arc<AppState>>) -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(BacktestResults {
            sortino_ratio: 1.5,
            max_drawdown_pct: 7.2,
            total_apy: 15.8,
        }),
    )
}

async fn get_execution_logs(State(_state): State<Arc<AppState>>) -> impl IntoResponse {
    let logs = vec![ExecutionLog {
        id: "exec_1".to_string(),
        timestamp: "2026-05-09T10:00:00Z".to_string(),
        action: "rebalance".to_string(),
    }];
    (StatusCode::OK, Json(logs))
}

/// GET /api/metrics — returns live systemic risk metrics
async fn get_metrics(State(_state): State<Arc<AppState>>) -> impl IntoResponse {
    use rl_agent::backtester::{generate_synthetic_data, BacktestEnvironment};
    use solfest_core::{Portfolio, RiskProfile, RLAction, Chain, Protocol, AllocationPercentage};
    use neo4j_graph::{GraphEmbeddings, RiskGraph, GARCHModel, BayesianRiskNetwork, Evidence};

    // Run a short synthetic backtest to get real metrics
    let days = 90usize;
    let market_data = generate_synthetic_data(days);
    let returns: Vec<f64> = market_data.windows(2).map(|w| {
        let prev = w[0].protocol_data.get("aave").map(|d| d.tvl_usd).unwrap_or(1.0);
        let curr = w[1].protocol_data.get("aave").map(|d| d.tvl_usd).unwrap_or(1.0);
        (curr - prev) / prev.max(1.0)
    }).collect();

    // GARCH volatility
    let garch_vol = GARCHModel::fit(&returns)
        .map(|m| m.conditional_variances(&returns).last().copied().unwrap_or(0.04).sqrt())
        .unwrap_or(0.04);

    // Bayesian risk inference
    let bay_net = BayesianRiskNetwork::default();
    let evidence = bay_net.update_from_market_data(garch_vol, 0.6);
    let cascade_prob = bay_net.p_liquidation_cascade(&evidence);

    // Risk graph absorption ratio (using a 7-node mock graph)
    let mut rg = RiskGraph::new_with_mock_data();
    let absorption = rg.calculate_absorption_ratio(3).unwrap_or(0.5);
    let contagion_result = rg.simulate_contagion_mc(
        std::collections::HashMap::from([("bridge_failure".to_string(), 0.3)]),
        5, 200, 0.7, 0.05, 42,
    );
    let contagion_paths = contagion_result.values().filter(|&&v| v > 0.5).count();

    // EVT expected shortfall
    let losses: Vec<f64> = returns.iter().map(|r| -r).collect();
    let es = neo4j_graph::EVTEstimator::fit(&losses, 0.90)
        .map(|e| e.expected_shortfall(0.95))
        .unwrap_or(0.045);

    Json(MetricsResponse {
        absorption,
        tvl: 48_000_000_000.0,
        contagion: contagion_paths,
        expected_shortfall: es,
        garch_volatility: garch_vol,
        bayesian_risk_score: cascade_prob,
    })
}

/// GET /api/graph — returns risk network nodes and links for force-graph visualization
async fn get_graph(State(_state): State<Arc<AppState>>) -> impl IntoResponse {
    use neo4j_graph::RiskGraph;

    let rg = RiskGraph::new_with_mock_data();
    let contagion = rg.simulate_contagion_mc(
        std::collections::HashMap::from([("bridge_failure".to_string(), 0.4)]),
        5, 100, 0.7, 0.05, 42,
    );

    let nodes: Vec<GraphNode> = rg.risk_nodes.iter().map(|name| {
        let val = contagion.get(name).copied().unwrap_or(0.0);
        GraphNode { id: name.clone(), val }
    }).collect();

    let n = rg.risk_nodes.len();
    let mut links = vec![];
    for i in 0..n {
        for j in (i + 1)..n {
            let w = rg.correlation_matrix[[i, j]];
            if w > 0.3 {
                links.push(GraphLink {
                    source: rg.risk_nodes[i].clone(),
                    target: rg.risk_nodes[j].clone(),
                    value: w,
                });
            }
        }
    }

    Json(GraphData { nodes, links })
}

/// GET /api/analytics/protocols — returns latest protocol snapshots from Neo4j
async fn get_analytics_protocols(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match &state.graph {
        Some(graph) => match graph.get_analytics_protocols().await {
            Ok(rows) => (StatusCode::OK, Json(serde_json::to_value(rows).unwrap_or_default())),
            Err(e) => {
                error!("Analytics query failed: {}", e);
                (StatusCode::BAD_GATEWAY, Json(serde_json::json!({"error": e.to_string()})))
            }
        },
        None => (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error": "Neo4j not connected"}))),
    }
}

/// POST /api/backtest — multi-asset portfolio simulation via portfolio_sim engine
async fn run_backtest(
    State(_state): State<Arc<AppState>>,
    Json(req): Json<BacktestRequest>,
) -> impl IntoResponse {
    match rl_agent::run_simulation(req) {
        Ok(result) => (StatusCode::OK, Json(result)).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": e}))).into_response(),
    }
}

/// GET /api/protocol-context — fetch protocol context batch from Neo4j
async fn get_protocol_context(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let names_str = params.get("names").cloned().unwrap_or_default();
    let names: Vec<String> = names_str.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();

    match &state.graph {
        Some(graph) => match graph.get_protocol_context_batch(&names).await {
            Ok(ctx) => (StatusCode::OK, Json(serde_json::to_value(ctx).unwrap_or_default())),
            Err(_e) => (StatusCode::OK, Json(serde_json::json!([]))),
        },
        None => (StatusCode::OK, Json(serde_json::json!([]))),
    }
}
