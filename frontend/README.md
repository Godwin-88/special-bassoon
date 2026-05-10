# Solfest Dashboard — Frontend

Next.js 16 dashboard for the Autonomous Cross-Chain Yield & Systemic Risk Allocator.

## Stack

- **Next.js 16.2.6** (Pages Router, Turbopack)
- **React 19** + TypeScript
- **Tailwind CSS 4**
- **Recharts 3** — portfolio value line chart
- **force-graph** — 2D risk contagion network (pure canvas, no A-Frame)
- **clsx + tailwind-merge** — conditional className utility

## Pages

| Route | Description |
|-------|-------------|
| `/` | Redirects to `/dashboard` |
| `/dashboard` | Live systemic risk metrics, contagion network, backtest portfolio chart |

## API Proxy Routes

The dashboard fetches same-origin `/api/*` routes that proxy to the Rust API on `:8080`:

| Proxy | Upstream | Description |
|-------|----------|-------------|
| `GET /api/metrics` | `:8080/api/metrics` | Absorption ratio, TVL, GARCH vol, EVT ES, Bayesian cascade prob |
| `GET /api/graph` | `:8080/api/graph` | Risk network nodes + edges |
| `POST /api/backtest` | `:8080/api/backtest` | Synthetic backtest — body: `{"days": 365}` |

## Dashboard Components

- **6 metric cards** — absorption ratio (color-coded HIGH/MODERATE/LOW), TVL, contagion paths, EVT expected shortfall, GARCH volatility, Bayesian liquidation cascade probability
- **Risk Contagion Network** — `force-graph` canvas visualization; node color encodes contagion level (red >60%, amber >30%, green otherwise)
- **Portfolio Value Chart** — Recharts `LineChart` of backtest history with Sortino/Sharpe/drawdown summary

## Development

```bash
npm install
npm run dev       # http://localhost:3000
npm run build     # production build
npm run lint
```

The Rust API must be running on port 8080 for live data:

```bash
# In repo root
cargo run -p api
```

If the API is unavailable, proxy routes return `503` and the dashboard shows `—` placeholders.

## Notes

- `react-force-graph` is installed but **not used** — it bundles A-Frame (VR/AR) which crashes in SSR. The dashboard uses the underlying `force-graph` package (2D only) loaded via `useEffect` import.
- Data refreshes every 30 seconds via `setInterval`; no WebSocket required for current polling cadence.
