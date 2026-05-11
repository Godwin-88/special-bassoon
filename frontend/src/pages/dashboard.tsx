import React, { useEffect, useState, useRef } from 'react'
import { Card, CardHeader, CardTitle, CardContent } from '@/components/ui/card'
import {
  LineChart, Line, XAxis, YAxis, CartesianGrid, Tooltip, ResponsiveContainer
} from 'recharts'

interface Metrics {
  absorption: number
  tvl: number
  contagion: number
  expected_shortfall: number
  garch_volatility: number
  bayesian_risk_score: number
}

interface GraphNode { id: string; val: number }
interface GraphLink { source: string; target: string; value: number }
interface GraphData { nodes: GraphNode[]; links: GraphLink[] }

interface BacktestMetrics {
  sortino_ratio?: number
  sharpe_ratio?: number
  sharpe?: number
  sortino?: number
  max_drawdown_pct?: number
  max_drawdown?: number
  total_apy?: number
  annualised_return?: number
  win_rate?: number
  cumulative_slippage_pct?: number
  expected_shortfall?: number
  es95?: number
  portfolio_history?: { date: string; value: number }[]
  jump_var_95?: number
  jump_variance_fraction?: number
  jump_lambda?: number
}

interface BacktestResult {
  metrics?: BacktestMetrics
  assets?: { id: string; label: string; asset_type: string }[]
  universe_size?: number
  // legacy flat fields (old static handler)
  sortino_ratio?: number
  sharpe_ratio?: number
  max_drawdown_pct?: number
  total_apy?: number
  win_rate?: number
  cumulative_slippage_pct?: number
  expected_shortfall?: number
  portfolio_history?: { date: string; value: number }[]
}

function riskColor(level: number): string {
  if (level > 0.6) return 'text-red-600 dark:text-red-400'
  if (level > 0.4) return 'text-yellow-600 dark:text-yellow-400'
  return 'text-green-600 dark:text-green-400'
}

function riskBadge(level: number): string {
  if (level > 0.6) return 'HIGH'
  if (level > 0.4) return 'MODERATE'
  return 'LOW'
}

export default function Dashboard() {
  const [metrics, setMetrics] = useState<Metrics | null>(null)
  const [graphData, setGraphData] = useState<GraphData>({ nodes: [], links: [] })
  const [backtest, setBacktest] = useState<BacktestResult | null>(null)
  const [loading, setLoading] = useState(true)
  const wsRef = useRef<WebSocket | null>(null)

  useEffect(() => {
    // Initial data fetch
    Promise.all([
      fetch('/api/metrics').then(r => r.json()).catch(() => null),
      fetch('/api/graph').then(r => r.json()).catch(() => null),
      fetch('/api/backtest', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ days: 365 }),
      }).then(r => r.json()).catch(() => null),
    ]).then(([m, g, b]) => {
      if (m) setMetrics(m)
      if (g) setGraphData(g)
      if (b) setBacktest(b)
      setLoading(false)
    })

    // Refresh metrics every 30s
    const interval = setInterval(() => {
      fetch('/api/metrics').then(r => r.json()).then(setMetrics).catch(() => {})
    }, 30_000)

    return () => clearInterval(interval)
  }, [])

  const portfolioChartData = (backtest?.metrics?.portfolio_history ?? backtest?.portfolio_history ?? [])
    .filter((point: { date: string; value: number }) => point.value != null && isFinite(point.value))
    .map((point: { date: string; value: number }) => ({
      time: new Date(point.date).toLocaleDateString(),
      value: parseFloat(point.value.toFixed(2)),
    }))

  return (
    <div className="p-6 space-y-6 bg-gray-50 dark:bg-gray-950 min-h-screen">
      <div className="flex items-center justify-between">
        <h1 className="text-3xl font-bold text-gray-900 dark:text-white">Systemic Risk Allocator OS</h1>
        {metrics && (
          <span className={`text-sm font-semibold px-3 py-1 rounded-full bg-gray-100 dark:bg-gray-800 ${riskColor(metrics.absorption)}`}>
            Systemic Risk: {riskBadge(metrics.absorption)}
          </span>
        )}
      </div>

      {/* Top row: key metrics */}
      <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
        <MetricCard
          title="Absorption Ratio"
          value={metrics ? metrics.absorption.toFixed(3) : '—'}
          subtitle="Eigenvalue fragility index"
          level={metrics?.absorption}
        />
        <MetricCard
          title="Total TVL"
          value={metrics ? `$${(metrics.tvl / 1e9).toFixed(1)}B` : '—'}
          subtitle="DeFi market size"
        />
        <MetricCard
          title="Contagion Paths"
          value={metrics ? metrics.contagion.toString() : '—'}
          subtitle="Nodes above 50% shock threshold"
          level={metrics ? Math.min(metrics.contagion / 7, 1) : undefined}
        />
      </div>

      {/* Second row: risk detail cards */}
      <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
        <MetricCard
          title="EVT Expected Shortfall"
          value={metrics ? `${(metrics.expected_shortfall * 100).toFixed(2)}%` : '—'}
          subtitle="95% CVaR (Peaks-over-Threshold)"
          level={metrics?.expected_shortfall}
        />
        <MetricCard
          title="GARCH Volatility"
          value={metrics ? `${(metrics.garch_volatility * 100).toFixed(2)}%` : '—'}
          subtitle="Conditional vol (annualized)"
          level={metrics ? metrics.garch_volatility * 5 : undefined}
        />
        <MetricCard
          title="Bayesian Cascade Risk"
          value={metrics ? `${(metrics.bayesian_risk_score * 100).toFixed(1)}%` : '—'}
          subtitle="P(LiquidationCascade | market state)"
          level={metrics?.bayesian_risk_score}
        />
      </div>

      {/* Charts row */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* Force graph */}
        <Card>
          <CardHeader>
            <CardTitle>Risk Contagion Network</CardTitle>
          </CardHeader>
          <CardContent className="h-[400px] flex items-center justify-center">
            <RiskGraph data={graphData} loading={loading} />
          </CardContent>
        </Card>

        {/* Portfolio history chart */}
        <Card>
          <CardHeader>
            <CardTitle>Backtest Portfolio Value</CardTitle>
          </CardHeader>
          <CardContent className="h-[400px]">
            {backtest && (
              <div className="mb-3 grid grid-cols-2 gap-2 text-xs text-gray-600 dark:text-gray-400">
                <span>Sortino: <strong className={riskColor(Math.max(0, 1 - ((backtest.metrics?.sortino ?? backtest.sortino_ratio ?? 0)) / 3))}>{(backtest.metrics?.sortino ?? backtest.sortino_ratio ?? 0).toFixed(3)}</strong></span>
                <span>Sharpe: <strong>{(backtest.metrics?.sharpe ?? backtest.sharpe_ratio ?? 0).toFixed(3)}</strong></span>
                <span>Max DD: <strong className={riskColor(Math.abs(backtest.metrics?.max_drawdown ?? backtest.max_drawdown_pct ?? 0) / 100)}>{(Math.abs(backtest.metrics?.max_drawdown ?? backtest.max_drawdown_pct ?? 0) * (backtest.metrics ? 100 : 1)).toFixed(1)}%</strong></span>
                <span>APY: <strong>{((backtest.metrics?.annualised_return ?? backtest.total_apy ?? 0) * 100).toFixed(1)}%</strong></span>
              </div>
            )}
            <ResponsiveContainer width="100%" height={330}>
              <LineChart data={portfolioChartData}>
                <CartesianGrid strokeDasharray="3 3" />
                <XAxis dataKey="time" tick={{ fontSize: 10 }} interval="preserveStartEnd" />
                <YAxis tick={{ fontSize: 10 }} />
                <Tooltip />
                <Line
                  type="monotone"
                  dataKey="value"
                  stroke="#2563eb"
                  dot={false}
                  strokeWidth={2}
                />
              </LineChart>
            </ResponsiveContainer>
          </CardContent>
        </Card>
      </div>

      {loading && (
        <div className="text-center text-gray-500 dark:text-gray-400 text-sm">
          Connecting to risk engine...
        </div>
      )}
    </div>
  )
}

function RiskGraph({ data, loading }: { data: GraphData; loading: boolean }) {
  const containerRef = useRef<HTMLDivElement>(null)
  const graphRef = useRef<unknown>(null)

  useEffect(() => {
    if (!containerRef.current || data.nodes.length === 0) return
    let cancelled = false

    import('force-graph').then((mod) => {
      if (cancelled || !containerRef.current) return
      const ForceGraph = (mod as any).default ?? mod

      const el = containerRef.current
      el.innerHTML = ''

      const g = ForceGraph()(el)
        .width(el.clientWidth || 480)
        .height(360)
        .graphData(data)
        .nodeLabel('id')
        .nodeVal((n: any) => Math.max((n.val ?? 0) * 20, 3))
        .nodeColor((n: any) => {
          const v = n.val ?? 0
          if (v > 0.6) return '#dc2626'
          if (v > 0.3) return '#d97706'
          return '#16a34a'
        })
        .linkWidth((l: any) => Math.max((l.value ?? 0) * 3, 0.5))
        .linkColor(() => '#94a3b8')

      graphRef.current = g
    })

    return () => {
      cancelled = true
      if (graphRef.current && (graphRef.current as any)._destructor) {
        ;(graphRef.current as any)._destructor()
      }
      graphRef.current = null
    }
  }, [data])

  if (data.nodes.length === 0) {
    return (
      <div className="text-gray-400 dark:text-gray-500 text-sm">
        {loading ? 'Loading risk graph...' : 'No graph data available'}
      </div>
    )
  }

  return <div ref={containerRef} className="w-full h-[360px]" />
}

function MetricCard({
  title, value, subtitle, level,
}: {
  title: string
  value: string
  subtitle?: string
  level?: number
}) {
  const color = level !== undefined ? riskColor(level) : 'text-gray-900 dark:text-white'
  return (
    <Card>
      <CardHeader>
        <CardTitle className="text-sm text-gray-600 dark:text-gray-400">{title}</CardTitle>
      </CardHeader>
      <CardContent>
        <div className={`text-2xl font-bold ${color}`}>{value}</div>
        {subtitle && <div className="text-xs text-gray-500 dark:text-gray-400 mt-1">{subtitle}</div>}
      </CardContent>
    </Card>
  )
}
