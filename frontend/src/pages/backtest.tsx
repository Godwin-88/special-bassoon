import React, { useState, useCallback } from 'react'
import { Card, CardHeader, CardTitle, CardContent } from '@/components/ui/card'
import {
  LineChart, Line, AreaChart, Area,
  XAxis, YAxis, CartesianGrid, Tooltip, ResponsiveContainer, Legend,
} from 'recharts'

// ─── Types that mirror Rust SimRequest / SimResult ───────────────────────────

type UniverseId = 'web3_defi' | 'web3_crypto' | 'hybrid' | 'trad_fi'
type AssetType  = 'defi_lp' | 'lending' | 'derivatives' | 'spot' | 'equity' | 'etf' | 'commodity' | 'stablecoin_yield'
type StrategyId =
  | 'equal_weight' | 'momentum' | 'mean_reversion' | 'trend_following'
  | 'risk_parity' | 'kelly' | 'delta_neutral' | 'quant_value'
  | 'stat_arb' | 'ml_alpha' | 'canslim' | 'liquidity_provision_opt'

interface SimRequest {
  universe: UniverseId
  asset_types: AssetType[]
  strategy: StrategyId
  params: { lookback?: number; short_window?: number; long_window?: number }
  days: number
  initial_capital: number
  risk_profile: 'conservative' | 'moderate' | 'aggressive'
}

interface PortfolioPoint { date: string; value: number }

interface SimMetrics {
  annualised_return: number
  sharpe: number
  sortino: number
  max_drawdown: number
  calmar: number
  win_rate: number
  volatility: number
  es95: number
  turnover: number
  portfolio_history: PortfolioPoint[]
}

interface SimResult {
  metrics: SimMetrics
  assets: { id: string; label: string; asset_type: string }[]
}

// ─── UI Config ────────────────────────────────────────────────────────────────

const UNIVERSES: { id: UniverseId; label: string; desc: string }[] = [
  { id: 'web3_defi',   label: 'Web3 DeFi',           desc: 'LP, lending, yield protocols' },
  { id: 'web3_crypto', label: 'Web3 Spot',            desc: 'ETH, BTC, SOL, major tokens' },
  { id: 'hybrid',      label: 'Hybrid (DeFi+TradFi)', desc: 'DeFi + equity ETFs + commodities' },
  { id: 'trad_fi',     label: 'Traditional Finance',  desc: 'Equities, bonds, commodities' },
]

const ASSET_TYPES: { id: AssetType; label: string }[] = [
  { id: 'defi_lp',          label: 'DeFi LP' },
  { id: 'lending',          label: 'Lending' },
  { id: 'derivatives',      label: 'Derivatives' },
  { id: 'spot',             label: 'Spot / Token' },
  { id: 'stablecoin_yield', label: 'Stablecoin Yield' },
  { id: 'equity',           label: 'Equity' },
  { id: 'etf',              label: 'ETF' },
  { id: 'commodity',        label: 'Commodity' },
]

interface StrategyDef {
  id: StrategyId
  label: string
  desc: string
  color: string
  params?: { key: 'lookback' | 'short_window' | 'long_window'; label: string; min: number; max: number; step: number; default: number }[]
}

const STRATEGIES: StrategyDef[] = [
  { id: 'equal_weight',          label: 'Equal Weight',                  color: '#6366f1', desc: '1/N across selected assets. Monthly rebalance.' },
  { id: 'momentum',              label: 'Momentum (Jegadeesh-Titman)',   color: '#10b981', desc: 'Overweight cross-sectional return winners.',
    params: [{ key: 'lookback', label: 'Lookback (days)', min: 5, max: 120, step: 5, default: 20 }] },
  { id: 'mean_reversion',        label: 'Mean Reversion (OU)',           color: '#f59e0b', desc: 'Long assets below moving average; Ornstein-Uhlenbeck.',
    params: [{ key: 'lookback', label: 'MA Window (days)', min: 5, max: 60, step: 5, default: 20 }] },
  { id: 'trend_following',       label: 'Trend Following (MA Crossover)', color: '#3b82f6', desc: 'Long when short MA > long MA; Antonacci Dual Momentum.',
    params: [
      { key: 'short_window', label: 'Short MA', min: 5, max: 30, step: 5, default: 10 },
      { key: 'long_window',  label: 'Long MA',  min: 20, max: 200, step: 10, default: 50 },
    ] },
  { id: 'risk_parity',           label: 'Risk Parity',                   color: '#8b5cf6', desc: 'Equalise risk contribution; weight ∝ 1/vol.',
    params: [{ key: 'lookback', label: 'Vol Window', min: 10, max: 60, step: 5, default: 20 }] },
  { id: 'kelly',                 label: 'Kelly Criterion',               color: '#ec4899', desc: 'Maximise log-utility; f* = µ/σ². 40% per-asset cap.',
    params: [{ key: 'lookback', label: 'Est. Window', min: 10, max: 60, step: 5, default: 20 }] },
  { id: 'delta_neutral',         label: 'Delta-Neutral Yield',           color: '#14b8a6', desc: 'Stablecoin + lending only; zero directional exposure.' },
  { id: 'quant_value',           label: 'Quant Value (P/TVL)',           color: '#f97316', desc: 'Long top-third by Sharpe proxy. DeFi: P/TVL, P/Revenue.',
    params: [{ key: 'lookback', label: 'Est. Window', min: 10, max: 60, step: 5, default: 20 }] },
  { id: 'stat_arb',              label: 'Statistical Arbitrage',         color: '#06b6d4', desc: 'Long underperformers, reversion bet. Chan spread z-score.',
    params: [{ key: 'lookback', label: 'Lookback', min: 5, max: 60, step: 5, default: 20 }] },
  { id: 'ml_alpha',              label: 'ML Alpha (Ensemble)',           color: '#a855f7', desc: '50/50 momentum + mean-reversion signal blend.' },
  { id: 'canslim',               label: 'CANSLIM Growth (O\'Neil)',       color: '#d97706', desc: 'Momentum with ≥50d lookback; protocol revenue growth proxy.',
    params: [{ key: 'lookback', label: 'Base Lookback', min: 30, max: 120, step: 10, default: 50 }] },
  { id: 'liquidity_provision_opt', label: 'LP Optimisation (Uniswap V3)', color: '#0ea5e9', desc: 'Weights LP/stablecoin assets by fee_APY/vol. Chan + Harvey.',
    params: [{ key: 'lookback', label: 'Vol Window', min: 10, max: 60, step: 5, default: 20 }] },
]

// ─── Helpers ─────────────────────────────────────────────────────────────────

function normalizeNav(history: PortfolioPoint[]): PortfolioPoint[] {
  if (!history.length) return []
  const first = history[0].value
  if (!first) return []
  return history.map(p => ({ date: p.date, value: parseFloat(((p.value / first) * 100).toFixed(2)) }))
}

function drawdownSeries(nav: PortfolioPoint[]): { date: string; dd: number }[] {
  let peak = -Infinity
  return nav.map(p => {
    if (p.value > peak) peak = p.value
    return { date: p.date, dd: peak > 0 ? parseFloat((((peak - p.value) / peak) * -100).toFixed(2)) : 0 }
  })
}

type MergedRow = Record<string, string | number>

function mergeSeries<T extends { date: string }>(
  series: { key: string; points: T[] }[],
  valueKey: keyof T,
): MergedRow[] {
  const dates = [...new Set(series.flatMap(s => s.points.map(p => p.date)))].sort()
  return dates.map(date => {
    const row: MergedRow = { date }
    series.forEach(s => {
      const pt = s.points.find(p => p.date === date)
      if (pt != null) row[s.key] = pt[valueKey] as unknown as number
    })
    return row
  })
}

function pct(v: number, d = 1) { return `${(v * 100).toFixed(d)}%` }

function exportCSV(runs: StrategyRun[]) {
  const done = runs.filter(r => r.result?.metrics) as (StrategyRun & { result: SimResult })[]
  if (!done.length) return
  const series = done.map(r => ({ key: r.label, points: normalizeNav(r.result.metrics.portfolio_history) }))
  const merged = mergeSeries(series, 'value')
  const csv = [
    ['Date', ...done.map(r => r.label)].join(','),
    ...merged.map(row => [row.date, ...done.map(r => row[r.label] ?? '')].join(',')),
  ].join('\n')
  const url = URL.createObjectURL(new Blob([csv], { type: 'text/csv' }))
  const a = Object.assign(document.createElement('a'), { href: url, download: 'backtest.csv' })
  a.click()
  URL.revokeObjectURL(url)
}

// ─── Run state ────────────────────────────────────────────────────────────────

interface StrategyRun {
  strategyId: StrategyId
  label: string
  color: string
  result: SimResult | null
  error: string
}

// ─── Page ─────────────────────────────────────────────────────────────────────

export default function BacktestPage() {
  const [universe, setUniverse] = useState<UniverseId>('web3_defi')
  const [assetTypes, setAssetTypes] = useState<AssetType[]>([])
  const [strategyId, setStrategyId] = useState<StrategyId>('momentum')
  const [compareMode, setCompareMode] = useState(false)
  const [capital, setCapital] = useState(100000)
  const [horizon, setHorizon] = useState(365)
  const [riskProfile, setRiskProfile] = useState<'conservative' | 'moderate' | 'aggressive'>('moderate')
  const [paramValues, setParamValues] = useState<Partial<Record<StrategyId, Record<string, number>>>>({})
  const [runs, setRuns] = useState<StrategyRun[]>([])
  const [loading, setLoading] = useState(false)

  const strategyDef = STRATEGIES.find(s => s.id === strategyId)!

  const getParam = (sid: StrategyId, key: string, def: number) =>
    paramValues[sid]?.[key] ?? def

  const toggleType = (t: AssetType) =>
    setAssetTypes(prev => prev.includes(t) ? prev.filter(x => x !== t) : [...prev, t])

  const runBacktest = useCallback(async () => {
    const toRun = compareMode ? STRATEGIES : [strategyDef]
    setLoading(true)
    setRuns(toRun.map(s => ({ strategyId: s.id, label: s.label, color: s.color, result: null, error: '' })))

    const results = await Promise.all(toRun.map(async s => {
      const params: Record<string, number> = {}
      for (const p of s.params ?? []) params[p.key] = getParam(s.id, p.key, p.default)
      const body: SimRequest = {
        universe,
        asset_types: assetTypes,
        strategy: s.id,
        params,
        days: horizon,
        initial_capital: capital,
        risk_profile: riskProfile,
      }
      try {
        const res = await fetch('/api/backtest/run', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(body),
        })
        const data = await res.json()
        if (!res.ok) return { id: s.id, result: null as SimResult | null, error: data.error ?? 'error' }
        return { id: s.id, result: data as SimResult, error: '' }
      } catch (e: any) {
        return { id: s.id, result: null as SimResult | null, error: e.message ?? 'failed' }
      }
    }))

    setRuns(toRun.map(s => {
      const r = results.find(x => x.id === s.id)!
      return { strategyId: s.id, label: s.label, color: s.color, result: r.result, error: r.error }
    }))
    setLoading(false)
  }, [compareMode, strategyId, universe, assetTypes, horizon, capital, riskProfile, paramValues])

  const activeRuns = (compareMode
    ? runs.filter(r => r.result?.metrics)
    : runs.filter(r => r.strategyId === strategyId && r.result?.metrics)) as (StrategyRun & { result: SimResult })[]

  const singleRun = !compareMode ? activeRuns[0] ?? null : null

  const equityData = mergeSeries(
    activeRuns.map(r => ({ key: r.label, points: normalizeNav(r.result.metrics.portfolio_history) })),
    'value',
  )
  const ddData = mergeSeries(
    activeRuns.map(r => ({ key: r.label, points: drawdownSeries(normalizeNav(r.result.metrics.portfolio_history)) })),
    'dd',
  )
  const tickInterval = Math.max(1, Math.floor((equityData.length - 1) / 6))

  return (
    <div className="p-6 space-y-6 bg-gray-50 dark:bg-gray-950 min-h-screen">
      <div>
        <h1 className="text-3xl font-bold text-gray-900 dark:text-white">Strategy Backtester</h1>
        <p className="text-sm text-gray-500 dark:text-gray-400 mt-1">
          GBM simulation with correlated assets. All computation runs in Rust — strategies from Chan, Tulchinsky, O'Neil, Harvey.
        </p>
      </div>

      <div className="flex gap-6 items-start">
        {/* ── Sidebar ── */}
        <aside className="w-72 shrink-0 space-y-4">

          <Card>
            <CardHeader><CardTitle className="text-xs uppercase tracking-wide text-gray-500 dark:text-gray-400">Asset Universe</CardTitle></CardHeader>
            <CardContent className="space-y-2">
              {UNIVERSES.map(u => (
                <label key={u.id} className="flex items-start gap-2 cursor-pointer">
                  <input type="radio" name="universe" value={u.id} checked={universe === u.id}
                    onChange={() => setUniverse(u.id)} className="mt-0.5 accent-indigo-600" />
                  <span className="flex flex-col">
                    <span className="text-sm font-medium text-gray-800 dark:text-gray-200">{u.label}</span>
                    <span className="text-xs text-gray-500 dark:text-gray-500">{u.desc}</span>
                  </span>
                </label>
              ))}
            </CardContent>
          </Card>

          <Card>
            <CardHeader><CardTitle className="text-xs uppercase tracking-wide text-gray-500 dark:text-gray-400">Asset Types</CardTitle></CardHeader>
            <CardContent className="flex flex-wrap gap-1.5">
              {ASSET_TYPES.map(t => (
                <button key={t.id} onClick={() => toggleType(t.id)}
                  className={`rounded-full px-2.5 py-0.5 text-xs font-medium border transition-colors ${
                    assetTypes.includes(t.id)
                      ? 'bg-indigo-600 border-indigo-600 text-white'
                      : 'border-gray-300 dark:border-gray-600 text-gray-600 dark:text-gray-400 hover:border-indigo-400'
                  }`}
                >
                  {t.label}
                </button>
              ))}
              {assetTypes.length > 0 && (
                <button onClick={() => setAssetTypes([])} className="text-xs text-gray-400 underline ml-1">clear</button>
              )}
            </CardContent>
          </Card>

          <Card>
            <CardHeader>
              <div className="flex items-center justify-between">
                <CardTitle className="text-xs uppercase tracking-wide text-gray-500 dark:text-gray-400">Strategy</CardTitle>
                <label className="flex items-center gap-1.5 text-xs text-gray-500 dark:text-gray-400 cursor-pointer">
                  <input type="checkbox" checked={compareMode} onChange={e => setCompareMode(e.target.checked)} className="accent-indigo-600" />
                  Compare all
                </label>
              </div>
            </CardHeader>
            <CardContent className="space-y-0.5 max-h-60 overflow-y-auto pr-1">
              {STRATEGIES.map(s => (
                <label key={s.id} className={`flex items-start gap-2 cursor-pointer rounded-md px-2 py-1.5 transition-colors ${
                  strategyId === s.id && !compareMode ? 'bg-gray-100 dark:bg-gray-800' : 'hover:bg-gray-50 dark:hover:bg-gray-800/50'
                }`}>
                  <input type="radio" name="strategy" value={s.id} checked={strategyId === s.id}
                    onChange={() => setStrategyId(s.id)} disabled={compareMode} className="mt-0.5 accent-indigo-600" />
                  <span>
                    <span className="text-sm font-medium block" style={{ color: s.color }}>{s.label}</span>
                    <span className="text-xs text-gray-500 dark:text-gray-500 leading-snug">{s.desc}</span>
                  </span>
                </label>
              ))}
            </CardContent>
          </Card>

          {!compareMode && strategyDef.params && (
            <Card>
              <CardHeader><CardTitle className="text-xs uppercase tracking-wide text-gray-500 dark:text-gray-400">Parameters</CardTitle></CardHeader>
              <CardContent className="space-y-3">
                {strategyDef.params.map(p => {
                  const val = getParam(strategyId, p.key, p.default)
                  return (
                    <div key={p.key}>
                      <div className="flex justify-between text-xs text-gray-600 dark:text-gray-400 mb-1">
                        <span>{p.label}</span><span className="font-mono">{val}d</span>
                      </div>
                      <input type="range" min={p.min} max={p.max} step={p.step} value={val}
                        onChange={e => setParamValues(prev => ({
                          ...prev, [strategyId]: { ...(prev[strategyId] ?? {}), [p.key]: +e.target.value }
                        }))}
                        className="w-full accent-indigo-600"
                      />
                    </div>
                  )
                })}
              </CardContent>
            </Card>
          )}

          <Card>
            <CardHeader><CardTitle className="text-xs uppercase tracking-wide text-gray-500 dark:text-gray-400">Capital (USD)</CardTitle></CardHeader>
            <CardContent>
              <input type="number" min={10000} max={10000000} step={10000} value={capital}
                onChange={e => setCapital(+e.target.value)}
                className="w-full rounded-md border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800 px-3 py-2 text-sm text-gray-900 dark:text-white"
              />
            </CardContent>
          </Card>

          <Card>
            <CardHeader><CardTitle className="text-xs uppercase tracking-wide text-gray-500 dark:text-gray-400">Horizon</CardTitle></CardHeader>
            <CardContent className="flex gap-2 flex-wrap">
              {[30, 90, 180, 365, 730].map(d => (
                <button key={d} onClick={() => setHorizon(d)}
                  className={`rounded-md px-2.5 py-1 text-xs font-medium transition-colors ${
                    horizon === d
                      ? 'bg-gray-900 dark:bg-white text-white dark:text-gray-900'
                      : 'bg-gray-100 dark:bg-gray-800 text-gray-700 dark:text-gray-300 hover:bg-gray-200 dark:hover:bg-gray-700'
                  }`}
                >
                  {d >= 365 ? `${d / 365}y` : `${d}d`}
                </button>
              ))}
            </CardContent>
          </Card>

          <Card>
            <CardHeader><CardTitle className="text-xs uppercase tracking-wide text-gray-500 dark:text-gray-400">Risk Profile</CardTitle></CardHeader>
            <CardContent>
              <select value={riskProfile} onChange={e => setRiskProfile(e.target.value as typeof riskProfile)}
                className="w-full rounded-md border border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800 px-3 py-2 text-sm text-gray-900 dark:text-white"
              >
                <option value="conservative">Conservative (50% leverage cap)</option>
                <option value="moderate">Moderate (80% leverage cap)</option>
                <option value="aggressive">Aggressive (100%)</option>
              </select>
            </CardContent>
          </Card>

          <button onClick={runBacktest} disabled={loading}
            className="w-full rounded-md bg-indigo-600 hover:bg-indigo-700 disabled:opacity-50 text-white px-4 py-2.5 text-sm font-semibold transition-colors"
          >
            {loading ? 'Simulating in Rust...' : 'Run Backtest'}
          </button>

          {activeRuns.length > 0 && (
            <button onClick={() => exportCSV(activeRuns)}
              className="w-full rounded-md border border-gray-300 dark:border-gray-600 text-gray-700 dark:text-gray-300 px-4 py-2 text-sm font-medium hover:bg-gray-100 dark:hover:bg-gray-800 transition-colors"
            >
              Export CSV
            </button>
          )}
        </aside>

        {/* ── Main Content ── */}
        <div className="flex-1 min-w-0 space-y-6">

          <Card>
            <CardHeader>
              <div className="flex items-center justify-between">
                <CardTitle>Equity Curve — NAV (Base 100)</CardTitle>
                {singleRun && (
                  <span className="text-xs text-gray-500 dark:text-gray-400">
                    {singleRun.result.assets.length} instruments · {universe}
                  </span>
                )}
              </div>
            </CardHeader>
            <CardContent className="h-72">
              {equityData.length === 0 ? (
                <div className="flex items-center justify-center h-full text-gray-400 dark:text-gray-500 text-sm">
                  {loading ? 'Running Rust simulation...' : 'Configure and run a backtest'}
                </div>
              ) : (
                <ResponsiveContainer width="100%" height="100%">
                  <LineChart data={equityData}>
                    <CartesianGrid strokeDasharray="3 3" stroke="#e5e7eb" />
                    <XAxis dataKey="date" tick={{ fontSize: 10 }} interval={tickInterval} />
                    <YAxis tick={{ fontSize: 10 }} />
                    <Tooltip formatter={(value) => typeof value === 'number' ? value.toFixed(2) : ''} />
                    {(compareMode || activeRuns.length > 1) && <Legend />}
                    {activeRuns.map(r => (
                      <Line key={r.strategyId} type="monotone" dataKey={r.label} stroke={r.color} dot={false} strokeWidth={2} />
                    ))}
                  </LineChart>
                </ResponsiveContainer>
              )}
            </CardContent>
          </Card>

          {ddData.length > 0 && (
            <Card>
              <CardHeader><CardTitle>Drawdown (%)</CardTitle></CardHeader>
              <CardContent className="h-48">
                <ResponsiveContainer width="100%" height="100%">
                  <AreaChart data={ddData}>
                    <CartesianGrid strokeDasharray="3 3" stroke="#e5e7eb" />
                    <XAxis dataKey="date" tick={{ fontSize: 10 }} interval={tickInterval} />
                    <YAxis tick={{ fontSize: 10 }} />
                    <Tooltip formatter={(v) => typeof v === 'number' ? `${v.toFixed(2)}%` : ''} />
                    {activeRuns.map(r => (
                      <Area key={r.strategyId} type="monotone" dataKey={r.label}
                        stroke={r.color} fill={r.color} fillOpacity={0.12} dot={false} strokeWidth={1.5}
                      />
                    ))}
                  </AreaChart>
                </ResponsiveContainer>
              </CardContent>
            </Card>
          )}

          {activeRuns.length > 0 && (
            <Card>
              <CardHeader><CardTitle>Performance Tearsheet</CardTitle></CardHeader>
              <CardContent>
                <div className="overflow-x-auto">
                  <table className="w-full text-sm text-left">
                    <thead>
                      <tr className="border-b border-gray-200 dark:border-gray-700 text-xs text-gray-500 dark:text-gray-400 uppercase">
                        <th className="py-2 pr-4">Strategy</th>
                        <th className="py-2 pr-3 text-right">Ann. Ret</th>
                        <th className="py-2 pr-3 text-right">Sharpe</th>
                        <th className="py-2 pr-3 text-right">Sortino</th>
                        <th className="py-2 pr-3 text-right">Max DD</th>
                        <th className="py-2 pr-3 text-right">Calmar</th>
                        <th className="py-2 pr-3 text-right">Vol</th>
                        <th className="py-2 pr-3 text-right">Win%</th>
                        <th className="py-2 pr-3 text-right">ES 95%</th>
                        <th className="py-2 text-right">Turnover</th>
                      </tr>
                    </thead>
                    <tbody>
                      {activeRuns.map(r => {
                        const m = r.result.metrics
                        return (
                          <tr key={r.strategyId} className="border-b border-gray-100 dark:border-gray-800 hover:bg-gray-50 dark:hover:bg-gray-800/40">
                            <td className="py-2.5 pr-4 font-semibold text-sm" style={{ color: r.color }}>{r.label}</td>
                            <td className={`py-2.5 pr-3 text-right font-mono ${m.annualised_return >= 0 ? 'text-emerald-500' : 'text-red-500'}`}>
                              {pct(m.annualised_return)}
                            </td>
                            <td className="py-2.5 pr-3 text-right font-mono text-gray-700 dark:text-gray-300">{m.sharpe.toFixed(2)}</td>
                            <td className="py-2.5 pr-3 text-right font-mono text-gray-700 dark:text-gray-300">{m.sortino.toFixed(2)}</td>
                            <td className="py-2.5 pr-3 text-right font-mono text-red-500">{pct(m.max_drawdown)}</td>
                            <td className="py-2.5 pr-3 text-right font-mono text-gray-700 dark:text-gray-300">{m.calmar.toFixed(2)}</td>
                            <td className="py-2.5 pr-3 text-right font-mono text-gray-700 dark:text-gray-300">{pct(m.volatility)}</td>
                            <td className="py-2.5 pr-3 text-right font-mono text-gray-700 dark:text-gray-300">{pct(m.win_rate)}</td>
                            <td className="py-2.5 pr-3 text-right font-mono text-amber-500">{pct(m.es95)}</td>
                            <td className="py-2.5 text-right font-mono text-gray-700 dark:text-gray-300">{m.turnover.toFixed(1)}x</td>
                          </tr>
                        )
                      })}
                    </tbody>
                  </table>
                </div>
              </CardContent>
            </Card>
          )}

          {singleRun && (
            <Card>
              <CardHeader><CardTitle>Asset Universe ({singleRun.result.assets.length} instruments)</CardTitle></CardHeader>
              <CardContent>
                <div className="flex flex-wrap gap-2">
                  {singleRun.result.assets.map(a => (
                    <span key={a.id} className="text-xs rounded-md bg-gray-100 dark:bg-gray-800 text-gray-700 dark:text-gray-300 px-2.5 py-1">
                      {a.label}
                      <span className="ml-1 text-gray-400 dark:text-gray-500">· {a.asset_type.replace('_', ' ')}</span>
                    </span>
                  ))}
                </div>
              </CardContent>
            </Card>
          )}

          {runs.filter(r => r.error).map(r => (
            <div key={r.strategyId} className="rounded-md bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-700 px-4 py-3 text-sm text-red-700 dark:text-red-300">
              {r.label}: {r.error}
            </div>
          ))}
        </div>
      </div>
    </div>
  )
}
