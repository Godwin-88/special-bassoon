# Autonomous Cross-Chain Yield & Systemic Risk Allocator

**A Real-Time Financial Network Operating System**

*GraphRAG · Bayesian Risk Networks · EVT/GARCH · Reinforcement Learning · Solana*

---

## Status

**Phase 3 complete** — institutional-grade financial engineering. Workspace compiles clean, 19 tests passing, dashboard live.

| Layer | Component | Status |
|-------|-----------|--------|
| Risk Math | EVT/POT (GPD MLE), GARCH(1,1), Hotelling deflation, MC Contagion | ✅ |
| Inference | 5-node binary Dynamic Bayesian Network (variable elimination) | ✅ |
| RL Engine | PPO with GAE, PolicyMode::Random cold-start, 720-action space | ✅ |
| Backtester | Synthetic + historical data, full PortfolioMetrics suite | ✅ |
| API | Axum REST on :8080, CORS, /api/metrics /api/graph /api/backtest | ✅ |
| Dashboard | Next.js 16, live metrics cards, force-graph, Recharts portfolio chart | ✅ |
| Execution Layer | Transaction construction (Solana/EVM) | ⏳ Phase 4 |
| Live Data | QuickNode RPC, DeFiLlama streaming | ⏳ Phase 4 |

---

## Architecture

```
                   ┌────────────────────┐
                   │  Cross-Chain Data  │
                   │  Solana/EVM/CEX    │
                   └─────────┬──────────┘
                             │
                    Streaming Pipelines
                             │
          ┌──────────────────┴──────────────────┐
          │                                     │
          ▼                                     ▼
┌────────────────────┐              ┌────────────────────┐
│  Knowledge Graph   │              │ Time-Series Engine │
│  Neo4j + GraphRAG  │              │  GARCH · EVT · DBN │
└─────────┬──────────┘              └─────────┬──────────┘
          │                                     │
          ▼                                     ▼
┌─────────────────────────────────────────────────────┐
│  SYSTEMIC RISK ENGINE                               │
│  Absorption Ratio · MC Contagion · EVT · Bayesian  │
└─────────────────────────────────────────────────────┘
                             │
                             ▼
┌─────────────────────────────────────────────────────┐
│  RL POLICY ENGINE                                   │
│  PPO · GAE · 720 Actions · Reward = Y - λ·Risk     │
└─────────────────────────────────────────────────────┘
                             │
                             ▼
┌─────────────────────────────────────────────────────┐
│  EXECUTION LAYER                                    │
│  Solana · LI.FI · QuickNode · Smart Routing        │
└─────────────────────────────────────────────────────┘
                             │
                             ▼
┌─────────────────────────────────────────────────────┐
│  DASHBOARD                                          │
│  Next.js 16 · Force Graph · Recharts · Live API    │
└─────────────────────────────────────────────────────┘
```

---

## Academic Foundation

| Algorithm | Reference | Status |
|-----------|-----------|--------|
| Absorption Ratio | Kritzman et al. (2011) | ✅ Hotelling eigenvalue deflation |
| Contagion Simulation | Eisenberg & Noe (2001) | ✅ Stochastic MC with liquidation tipping |
| EVT / POT | McNeil, Frey & Embrechts (2005) Ch. 7 | ✅ GPD MLE via Nelder-Mead |
| GARCH(1,1) | Bollerslev (1986) | ✅ Gaussian MLE, stationarity enforced |
| Bayesian DBN | Pearl (1988); Koller & Friedman (2009) | ✅ Variable elimination, 5-node binary |
| PPO | Schulman et al. (2017) | ✅ Clip loss, value coeff, entropy reg |
| GAE | Schulman et al. (2016) | ✅ λ=0.95, terminal bootstrapping |
| Sharpe / Sortino / Calmar | Sharpe (1966); Sortino & Price (1994) | ✅ |

---

## Reward Function

```
R_t = Y_t − λ₁·ES_t − λ₂·C_t − λ₃·S_t − λ₄·L_t

  Y_t  = daily yield
  ES_t = Expected Shortfall / CVaR (EVT-derived)   λ₁ = 0.35
  C_t  = Contagion risk index                       λ₂ = 0.30
  S_t  = Slippage cost fraction                     λ₃ = 0.15
  L_t  = Liquidity instability index                λ₄ = 0.20

Scaled to basis points, clamped to [−1000, 1000].
```

---

## Project Structure

```
solfest/
├── solfest-core/           # Shared types: Portfolio, RLState, RLAction, Constraints
├── neo4j-graph/            # Risk graph, embeddings, EVT, GARCH, Bayesian DBN
│   ├── src/risk_graph.rs   # Absorption ratio, MC contagion (Hotelling deflation)
│   ├── src/evt.rs          # EVT/POT — GPD MLE, VaR, Expected Shortfall
│   ├── src/garch.rs        # GARCH(1,1) — MLE, conditional variances, forecast
│   └── src/bayesian_network.rs  # 5-node DBN, variable elimination inference
├── data-pipeline/          # On-chain data, social signals, state construction
├── rl-agent/               # PPO policy, GAE training, backtester
│   ├── src/backtester.rs   # BacktestEnvironment, PortfolioMetrics, reward fn
│   ├── src/policy.rs       # PPOPolicy (Random + Loaded modes), PPO loss
│   └── src/training.rs     # PPOTrainer, EqualWeightBaseline
├── execution-engine/       # Rebalancing + LI.FI routing (Phase 4)
├── api/                    # Axum REST API — port 8080
│   └── src/main.rs         # /api/metrics, /api/graph, /api/backtest, /health
├── frontend/               # Next.js 16 dashboard
│   └── src/pages/
│       ├── dashboard.tsx   # Live metrics, force-graph, portfolio chart
│       └── api/            # Proxy routes (metrics, graph, backtest)
├── cypher/                 # Neo4j schema and seed data (65 sources, 340+ nodes)
├── docker-compose.yml
└── Cargo.toml              # Workspace
```

---

## Quick Start

### Prerequisites

- Rust (stable)
- Node.js 20+
- Python 3.12 + PyTorch (optional — needed only for neural network inference)

### 1. Environment

```bash
cp .env.example .env
# Set: NEO4J_URI, NEO4J_USER, NEO4J_PASSWORD, LIFI_API_KEY
# Optional for tch: LIBTORCH=<path/to/torch> LIBTORCH_CXX11_ABI=0
```

### 2. Initialize Neo4j Knowledge Graph

```bash
docker-compose up -d neo4j

# Seed the financial ontology
for f in cypher/*.cypher; do
  cat "$f" | docker exec -i neo4j cypher-shell -u neo4j -p password
done
```

### 3. Run the API (port 8080)

```bash
cargo run -p api
```

### 4. Run the Dashboard

```bash
cd frontend
npm install
npm run dev
```

Dashboard: `http://localhost:3000` (redirects to `/dashboard`)

### 5. Run Tests

```bash
cargo test -p neo4j-graph    # 13 tests: EVT, GARCH, Bayesian, contagion
cargo test -p rl-agent       # 6 tests: backtester, PPO, training, baseline
```

### Neural Network Inference (optional)

`tch` (PyTorch bindings) is an optional feature. The system runs fully in `PolicyMode::Random` without it. To enable:

```bash
cargo build -p rl-agent --features torch
```

---

## API Reference

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/api/metrics` | Live systemic risk: absorption ratio, GARCH vol, EVT ES, Bayesian cascade probability |
| GET | `/api/graph` | Risk network nodes and edges for force-graph visualization |
| POST | `/api/backtest` | Run synthetic backtest — body: `{"days": 365, "initial_value_usd": 100000}` |
| GET | `/health` | Health check |

---

## Risk Interpretations

**Absorption Ratio**
- `> 0.60` → HIGH systemic risk — reduce positions, increase hedging
- `0.40–0.60` → MODERATE — standard management
- `< 0.40` → LOW — diversification effective

**Bayesian Cascade Probability** (P(LiquidationCascade = High | market state))
- CPTs calibrated from: Terra/LUNA collapse, Nomad Bridge exploit, Euler Finance hack, USDC depeg, Aave liquidity crisis

---

## Phase Roadmap

| Phase | Scope | Status |
|-------|-------|--------|
| 1 | Core types, workspace, Neo4j client, backtester scaffold | ✅ Complete |
| 2 | Portfolio metrics, PPO loss, GAE, absorption ratio scaffold | ✅ Complete |
| 3 | EVT/GARCH/Bayesian/MC-contagion, wired API, live dashboard | ✅ Complete |
| 4 | QuickNode live data, Solana/EVM tx construction, LI.FI routing | ⏳ Next |
| 5 | Neural forecasting (Transformer + GNN), RL training loop at scale | ⏳ Planned |
| 6 | Testnet validation, mainnet canary, monitoring, compliance | ⏳ Planned |

---

## Key Differentiators

Most DeFi AI systems are reactive, statistical, and shallow.

This system is **causal** (Bayesian networks), **graph-native** (Neo4j + GraphRAG), **probabilistic** (EVT + GARCH forecasting), and **autonomous** (RL execution).

Result: institutional-grade autonomous treasury management, not just a yield optimizer.
