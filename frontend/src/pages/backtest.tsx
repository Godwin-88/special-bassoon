import React, { useState, useCallback, useRef } from 'react'
import { Card, CardHeader, CardTitle, CardContent } from '@/components/ui/card'
import {
  LineChart, Line, AreaChart, Area,
  XAxis, YAxis, CartesianGrid, Tooltip, ResponsiveContainer, Legend,
} from 'recharts'
import MarkdownRenderer from '@/components/MarkdownRenderer'

// ─── Types that mirror Rust SimRequest / SimResult ───────────────────────────

export type UniverseId = 'web3_defi' | 'web3_crypto' | 'hybrid' | 'trad_fi'
export type AssetType  =
  | 'defi_lp' | 'lending' | 'derivatives' | 'spot'
  | 'equity' | 'etf' | 'commodity' | 'stablecoin_yield'
export type StrategyId =
  | 'equal_weight' | 'momentum' | 'mean_reversion' | 'trend_following'
  | 'risk_parity' | 'kelly' | 'delta_neutral' | 'quant_value'
  | 'stat_arb' | 'ml_alpha' | 'canslim' | 'liquidity_provision_opt'

export interface SimRequest {
  universe: UniverseId
  asset_types: AssetType[]
  strategy: StrategyId
  params: { lookback?: number; short_window?: number; long_window?: number }
  days: number
  initial_capital: number
  risk_profile: 'conservative' | 'moderate' | 'aggressive'
  /** 500–3000. Only sent when applicable asset types are selected. */
  top_n?: number
}

export interface PortfolioPoint { date: string; value: number }

export interface SimMetrics {
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
  // Jump-adjusted risk metrics (Spadafora et al. arXiv:1803.07021)
  jump_var_95?: number
  jump_variance_fraction?: number
  jump_lambda?: number
}

export interface SimResult {
  metrics: SimMetrics
  assets: { id: string; label: string; asset_type: string }[]
  universe_size: number
}

// ─── UI Config ────────────────────────────────────────────────────────────────

export const UNIVERSES: { id: UniverseId; label: string; desc: string }[] = [
  { id: 'web3_defi',   label: 'Web3 DeFi',           desc: 'LP, lending, yield protocols' },
  { id: 'web3_crypto', label: 'Web3 Spot',            desc: 'ETH, BTC, SOL, major tokens' },
  { id: 'hybrid',      label: 'Hybrid',               desc: 'DeFi + equity ETFs + commodities' },
  { id: 'trad_fi',     label: 'Traditional Finance',  desc: 'Equities, bonds, commodities' },
]

export const ASSET_TYPES: { id: AssetType; label: string; rankable: boolean }[] = [
  { id: 'defi_lp',          label: 'DeFi LP',          rankable: false },
  { id: 'lending',          label: 'Lending',          rankable: false },
  { id: 'derivatives',      label: 'Derivatives / Futures / Options', rankable: true  },
  { id: 'spot',             label: 'Spot / Token',     rankable: true  },
  { id: 'stablecoin_yield', label: 'Stablecoin Yield', rankable: false },
  { id: 'equity',           label: 'Equity / Stocks',  rankable: true  },
  { id: 'etf',              label: 'ETF',              rankable: false },
  { id: 'commodity',        label: 'Commodity',        rankable: false },
]

/** Trading-day lookback options — dt = 1/252 in the Rust GBM engine. Max = 5 trading years. */
export const LOOKBACK_OPTIONS: { days: number; label: string }[] = [
  { days: 21,   label: '1 M'  },
  { days: 63,   label: '3 M'  },
  { days: 126,  label: '6 M'  },
  { days: 252,  label: '1 Y'  },
  { days: 504,  label: '2 Y'  },
  { days: 756,  label: '3 Y'  },
  { days: 1008, label: '4 Y'  },
  { days: 1260, label: '5 Y'  },
]

/** Top-N universe size options for rankable asset types (500–3000). */
export const TOP_N_OPTIONS = [500, 1000, 1500, 2000, 2500, 3000]

interface StrategyDef {
  id: StrategyId
  label: string
  desc: string
  color: string
  params?: { key: 'lookback' | 'short_window' | 'long_window'; label: string; min: number; max: number; step: number; default: number }[]
}

export const STRATEGIES: StrategyDef[] = [
  { id: 'equal_weight',            label: 'Equal Weight',                   color: '#6366f1', desc: '1/N across selected assets. Monthly rebalance.' },
  { id: 'momentum',                label: 'Momentum (Jegadeesh-Titman)',    color: '#10b981', desc: 'Overweight cross-sectional return winners.',
    params: [{ key: 'lookback', label: 'Lookback (days)', min: 5, max: 120, step: 5, default: 20 }] },
  { id: 'mean_reversion',          label: 'Mean Reversion (OU)',            color: '#f59e0b', desc: 'Long assets below moving average; Ornstein-Uhlenbeck.',
    params: [{ key: 'lookback', label: 'MA Window (days)', min: 5, max: 60, step: 5, default: 20 }] },
  { id: 'trend_following',         label: 'Trend Following (MA Crossover)', color: '#3b82f6', desc: 'Long when short MA > long MA; Antonacci Dual Momentum.',
    params: [
      { key: 'short_window', label: 'Short MA', min: 5, max: 30, step: 5, default: 10 },
      { key: 'long_window',  label: 'Long MA',  min: 20, max: 200, step: 10, default: 50 },
    ] },
  { id: 'risk_parity',             label: 'Risk Parity',                    color: '#8b5cf6', desc: 'Equalise risk contribution; weight ∝ 1/vol.',
    params: [{ key: 'lookback', label: 'Vol Window', min: 10, max: 60, step: 5, default: 20 }] },
  { id: 'kelly',                   label: 'Kelly Criterion',                color: '#ec4899', desc: 'Maximise log-utility; f* = µ/σ². 40% per-asset cap.',
    params: [{ key: 'lookback', label: 'Est. Window', min: 10, max: 60, step: 5, default: 20 }] },
  { id: 'delta_neutral',           label: 'Delta-Neutral Yield',            color: '#14b8a6', desc: 'Stablecoin + lending only; zero directional exposure.' },
  { id: 'quant_value',             label: 'Quant Value (P/TVL)',            color: '#f97316', desc: 'Long top-third by Sharpe proxy. DeFi: P/TVL, P/Revenue.',
    params: [{ key: 'lookback', label: 'Est. Window', min: 10, max: 60, step: 5, default: 20 }] },
  { id: 'stat_arb',                label: 'Statistical Arbitrage',          color: '#06b6d4', desc: 'Long underperformers, reversion bet. Chan spread z-score.',
    params: [{ key: 'lookback', label: 'Lookback', min: 5, max: 60, step: 5, default: 20 }] },
  { id: 'ml_alpha',                label: 'ML Alpha (Ensemble)',            color: '#a855f7', desc: '50/50 momentum + mean-reversion signal blend.' },
  { id: 'canslim',                 label: "CANSLIM Growth (O'Neil)",        color: '#d97706', desc: 'Momentum with ≥50d lookback; protocol revenue growth proxy.',
    params: [{ key: 'lookback', label: 'Base Lookback', min: 30, max: 120, step: 10, default: 50 }] },
  { id: 'liquidity_provision_opt', label: 'LP Optimisation (Uniswap V3)',   color: '#0ea5e9', desc: 'Weights LP/stablecoin assets by fee_APY/vol. Chan + Harvey.',
    params: [{ key: 'lookback', label: 'Vol Window', min: 10, max: 60, step: 5, default: 20 }] },
]

// ─── Helpers ─────────────────────────────────────────────────────────────────

export function normalizeNav(history: PortfolioPoint[]): PortfolioPoint[] {
  if (!history.length) return []
  const first = history[0].value
  if (!first) return []
  return history.map(p => ({ date: p.date, value: parseFloat(((p.value / first) * 100).toFixed(2)) }))
}

export function drawdownSeries(nav: PortfolioPoint[]): { date: string; dd: number }[] {
  let peak = -Infinity
  return nav.map(p => {
    if (p.value > peak) peak = p.value
    return { date: p.date, dd: peak > 0 ? parseFloat((((peak - p.value) / peak) * -100).toFixed(2)) : 0 }
  })
}

type MergedRow = Record<string, string | number>

export function mergeSeries<T extends { date: string }>(
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

export function pct(v: number, d = 1): string { return `${(v * 100).toFixed(d)}%` }

/** Returns true when at least one selected asset type (or default types for the universe) is rankable. */
export function isTopNApplicable(universe: UniverseId, assetTypes: AssetType[]): boolean {
  if (assetTypes.length > 0) {
    return assetTypes.some(t => ASSET_TYPES.find(a => a.id === t)?.rankable)
  }
  // Default: TradFi and Hybrid include equity/derivatives; Crypto includes spot/derivatives
  return universe !== 'web3_defi'
}

// ─── Metric tooltip definitions ──────────────────────────────────────────────

const METRIC_DEFS: Record<string, { title: string; formula: string; interpretation: string }> = {
  'Ann. Ret': {
    title: 'Annualised Return',
    formula: '(1 + R_total)^(252 / T) − 1',
    interpretation: 'Geometric compound return scaled to one calendar year. T = trading days in simulation.',
  },
  'Sharpe': {
    title: 'Sharpe Ratio (ex-ante, rf = 5%)',
    formula: '(μ_excess × 252) / (σ_daily × √252)',
    interpretation: 'Risk-adjusted excess return per unit of total volatility. > 1.0 = institutionally acceptable; > 2.0 = exceptional.',
  },
  'Sortino': {
    title: 'Sortino Ratio',
    formula: '(μ_excess × 252) / (σ_downside × √252)',
    interpretation: 'Like Sharpe but penalises only downside deviation. Sortino >> Sharpe signals asymmetric payoff profile (large wins, small losses).',
  },
  'Max DD': {
    title: 'Maximum Drawdown',
    formula: 'max_t [(peak_t − NAV_t) / peak_t]',
    interpretation: 'Largest peak-to-trough NAV decline on any sub-path. Path-dependent; must be paired with Calmar to assess recoverability.',
  },
  'Calmar': {
    title: 'Calmar Ratio',
    formula: 'Ann. Return / |Max Drawdown|',
    interpretation: '> 1.0 = strategy earns more than its worst drawdown per year. The minimum institutional allocation threshold.',
  },
  'Vol': {
    title: 'Annualised Volatility',
    formula: 'σ_daily × √252',
    interpretation: 'Standard deviation of daily portfolio returns scaled to annual frequency. Includes both upside and downside variance.',
  },
  'Win%': {
    title: 'Win Rate',
    formula: 'count(daily_ret > 0) / T',
    interpretation: 'Fraction of positive-return trading days. Sub-50% is viable if paired with positive skew (rare large wins).',
  },
  'ES 95%': {
    title: 'Expected Shortfall (CVaR) at 95%',
    formula: 'E[−R | R < VaR_95]',
    interpretation: 'Average loss on the worst 5% of trading days. Coherent risk measure; used by Basel III and institutional risk committees. More informative than VaR alone.',
  },
  'Jump VaR': {
    title: 'Jump-Adjusted VaR 95% (Spadafora et al. arXiv:1803.07021)',
    formula: 'VaR decomposed via Merton jump-diffusion: σ_diffusive + Poisson(λ) compound jumps',
    interpretation: 'Separates diffusive (hedgeable with delta) from jump (requires gamma/vega or tail protection) risk in the 1-day 95% loss estimate.',
  },
  'Jump %': {
    title: 'Jump Variance Fraction',
    formula: 'σ²_jump / (σ²_diffusive + σ²_jump)',
    interpretation: 'Fraction of total return variance driven by discontinuous jump events. > 40% = strategy risk profile dominated by regime shifts and liquidity gaps, not continuous drift.',
  },
  'Turnover': {
    title: 'Annualised Portfolio Turnover',
    formula: 'mean(Σ|Δw_i|) × 252  [expressed as portfolio turns/year]',
    interpretation: '1x = full portfolio replaced once per year. High turnover (> 10x) severely erodes live Sharpe; assume 10–30 bps all-in cost per turn for institutional execution.',
  },
}

function MetricTh({ col, className = '' }: { col: string; className?: string }) {
  const [show, setShow] = useState(false)
  const def = METRIC_DEFS[col]
  const ref = useRef<HTMLTableCellElement>(null)

  return (
    <th
      ref={ref}
      className={`py-2 pr-3 text-right relative select-none ${def ? 'cursor-help underline decoration-dotted decoration-gray-400' : ''} ${className}`}
      onMouseEnter={() => def && setShow(true)}
      onMouseLeave={() => setShow(false)}
    >
      {col}
      {show && def && (
        <div className="absolute z-50 right-0 top-full mt-1 w-72 rounded-lg border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 shadow-xl p-3 text-left pointer-events-none">
          <p className="font-semibold text-xs text-gray-900 dark:text-white mb-1">{def.title}</p>
          <p className="font-mono text-[10px] text-indigo-600 dark:text-indigo-400 mb-1.5 bg-indigo-50 dark:bg-indigo-950/50 rounded px-1.5 py-0.5">{def.formula}</p>
          <p className="text-[11px] text-gray-600 dark:text-gray-400 leading-relaxed">{def.interpretation}</p>
        </div>
      )}
    </th>
  )
}

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

export interface StrategyRun {
  strategyId: StrategyId
  label: string
  color: string
  result: SimResult | null
  error: string
}

// ─── Sub-components ───────────────────────────────────────────────────────────

/** Top-bar universe selector — horizontal radio pills. */
function UniverseBar({
  value, onChange,
}: { value: UniverseId; onChange: (v: UniverseId) => void }) {
  return (
    <div data-testid="universe-bar" className="flex flex-wrap gap-2">
      {UNIVERSES.map(u => (
        <label key={u.id} data-testid={`universe-${u.id}`}
          className={`cursor-pointer rounded-lg border px-4 py-2 text-sm font-medium transition-colors ${
            value === u.id
              ? 'bg-indigo-600 border-indigo-600 text-white shadow-sm'
              : 'border-gray-300 dark:border-gray-600 bg-white dark:bg-gray-800 text-gray-700 dark:text-gray-300 hover:border-indigo-400'
          }`}>
          <input type="radio" className="sr-only" name="universe" value={u.id}
            checked={value === u.id} onChange={() => onChange(u.id)} />
          <span className="block font-semibold">{u.label}</span>
          <span className="block text-xs opacity-70 mt-0.5">{u.desc}</span>
        </label>
      ))}
    </div>
  )
}

/** Top-bar asset type selector — horizontal checkbox pills. */
function AssetTypeBar({
  selected, onChange,
}: { selected: AssetType[]; onChange: (v: AssetType[]) => void }) {
  const toggle = (t: AssetType) =>
    onChange(selected.includes(t) ? selected.filter(x => x !== t) : [...selected, t])

  return (
    <div data-testid="asset-type-bar" className="flex flex-wrap gap-1.5 items-center">
      {ASSET_TYPES.map(t => (
        <button key={t.id} data-testid={`asset-type-${t.id}`}
          onClick={() => toggle(t.id)}
          className={`rounded-full px-3 py-1 text-xs font-medium border transition-colors ${
            selected.includes(t.id)
              ? 'bg-indigo-600 border-indigo-600 text-white'
              : 'border-gray-300 dark:border-gray-600 text-gray-600 dark:text-gray-400 hover:border-indigo-400'
          }`}>
          {t.label}
        </button>
      ))}
      {selected.length > 0 && (
        <button onClick={() => onChange([])}
          className="text-xs text-gray-400 underline ml-1">
          clear
        </button>
      )}
    </div>
  )
}

/** Top-bar Top-N selector — shown only when applicable asset types are selected. */
function TopNBar({
  universe, assetTypes, value, onChange,
}: {
  universe: UniverseId
  assetTypes: AssetType[]
  value: number | undefined
  onChange: (v: number | undefined) => void
}) {
  if (!isTopNApplicable(universe, assetTypes)) return null

  return (
    <div data-testid="top-n-bar" className="flex items-center gap-2 flex-wrap">
      <span className="text-xs font-semibold text-gray-500 dark:text-gray-400 uppercase tracking-wide whitespace-nowrap">
        Universe Size
      </span>
      {TOP_N_OPTIONS.map(n => (
        <button key={n} data-testid={`top-n-${n}`}
          onClick={() => onChange(value === n ? undefined : n)}
          className={`rounded-md px-2.5 py-1 text-xs font-medium transition-colors ${
            value === n
              ? 'bg-indigo-600 text-white'
              : 'bg-gray-100 dark:bg-gray-800 text-gray-700 dark:text-gray-300 hover:bg-gray-200 dark:hover:bg-gray-700'
          }`}>
          Top {n.toLocaleString()}
        </button>
      ))}
      {value !== undefined && (
        <button onClick={() => onChange(undefined)} className="text-xs text-gray-400 underline">
          curated only
        </button>
      )}
    </div>
  )
}

// ─── Page ─────────────────────────────────────────────────────────────────────

export default function BacktestPage() {
  const [universe, setUniverse]   = useState<UniverseId>('web3_defi')
  const [assetTypes, setAssetTypes] = useState<AssetType[]>([])
  const [topN, setTopN]           = useState<number | undefined>(undefined)
  const [strategyId, setStrategyId] = useState<StrategyId>('momentum')
  const [compareMode, setCompareMode] = useState(false)
  const [capital, setCapital]     = useState(100000)
  const [horizon, setHorizon]     = useState(252)
  const [riskProfile, setRiskProfile] = useState<'conservative' | 'moderate' | 'aggressive'>('moderate')
  const [paramValues, setParamValues] = useState<Partial<Record<StrategyId, Record<string, number>>>>({})
  const [runs, setRuns]           = useState<StrategyRun[]>([])
  const [loading, setLoading]     = useState(false)
  const [aiAnalysis, setAiAnalysis] = useState<string>('')
  const [aiLoading, setAiLoading]   = useState(false)
  const [aiError, setAiError]       = useState<string>('')

  const strategyDef = STRATEGIES.find(s => s.id === strategyId)!

  const getParam = (sid: StrategyId, key: string, def: number) =>
    paramValues[sid]?.[key] ?? def

  const runBacktest = useCallback(async () => {
    const toRun = compareMode ? STRATEGIES : [strategyDef]
    setLoading(true)
    setAiAnalysis('')
    setAiError('')
    setRuns(toRun.map(s => ({ strategyId: s.id, label: s.label, color: s.color, result: null, error: '' })))

    // For large top-N in compare mode, run sequentially to avoid saturating the Rust API
    const largeUniverse = topN !== undefined && topN >= 1000 && compareMode
    const TIMEOUT_MS = 90_000

    const runOne = async (s: typeof toRun[0]) => {
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
        ...(topN !== undefined && isTopNApplicable(universe, assetTypes) ? { top_n: topN } : {}),
      }
      const controller = new AbortController()
      const timer = setTimeout(() => controller.abort(), TIMEOUT_MS)
      try {
        const res = await fetch('/api/backtest/run', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify(body),
          signal: controller.signal,
        })
        const data = await res.json()
        if (!res.ok) return { id: s.id, result: null as SimResult | null, error: data.error ?? 'error' }
        return { id: s.id, result: data as SimResult, error: '' }
      } catch (e: any) {
        const msg = e.name === 'AbortError' ? 'Simulation timed out (> 90s) — try a smaller universe' : (e.message ?? 'failed')
        return { id: s.id, result: null as SimResult | null, error: msg }
      } finally {
        clearTimeout(timer)
      }
    }

    let results: Awaited<ReturnType<typeof runOne>>[]
    if (largeUniverse) {
      // Sequential to protect the Rust API from concurrent large-universe allocations
      results = []
      for (const s of toRun) {
        const r = await runOne(s)
        results.push(r)
        // Update incrementally so the user sees progress
        setRuns(prev => prev.map(run =>
          run.strategyId === s.id
            ? { ...run, result: r.result, error: r.error }
            : run
        ))
      }
    } else {
      results = await Promise.all(toRun.map(runOne))
    }

    setRuns(toRun.map(s => {
      const r = results.find(x => x.id === s.id)!
      return { strategyId: s.id, label: s.label, color: s.color, result: r.result, error: r.error }
    }))
    setLoading(false)
  }, [compareMode, strategyId, universe, assetTypes, topN, horizon, capital, riskProfile, paramValues])

  const runAiAnalysis = useCallback(async (completedRuns: (StrategyRun & { result: SimResult })[]) => {
    setAiLoading(true)
    setAiAnalysis('')
    setAiError('')
    try {
      const res = await fetch('/api/backtest/analyze', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          runs: completedRuns.map(r => ({ label: r.label, color: r.color, metrics: r.result.metrics })),
          config: { universe, asset_types: assetTypes, days: horizon, initial_capital: capital, risk_profile: riskProfile, top_n: topN },
        }),
      })
      const data = await res.json()
      if (!res.ok) { setAiError(data.error ?? 'Analysis failed'); return }
      setAiAnalysis(data.analysis ?? '')
    } catch (e: any) {
      setAiError(e.message ?? 'Network error')
    } finally {
      setAiLoading(false)
    }
  }, [universe, assetTypes, horizon, capital, riskProfile, topN])

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
    <div className="p-6 space-y-5 bg-gray-50 dark:bg-gray-950 min-h-screen">
      <div>
        <h1 className="text-3xl font-bold text-gray-900 dark:text-white">Strategy Backtester</h1>
        <p className="text-sm text-gray-500 dark:text-gray-400 mt-1">
          GBM simulation with correlated assets. All computation runs in Rust — strategies from Chan, Tulchinsky, O'Neil, Harvey.
        </p>
      </div>

      {/* ── Top bar: universe + asset types + top-N ── */}
      <div className="space-y-3 rounded-xl border border-gray-200 dark:border-gray-800 bg-white dark:bg-gray-900 shadow-sm p-4">
        <div>
          <p className="text-xs font-semibold text-gray-500 dark:text-gray-400 uppercase tracking-wide mb-2">
            Asset Universe
          </p>
          <UniverseBar value={universe} onChange={u => { setUniverse(u); setTopN(undefined) }} />
        </div>

        <div>
          <p className="text-xs font-semibold text-gray-500 dark:text-gray-400 uppercase tracking-wide mb-2">
            Asset Types
          </p>
          <AssetTypeBar selected={assetTypes} onChange={setAssetTypes} />
        </div>

        <TopNBar universe={universe} assetTypes={assetTypes} value={topN} onChange={setTopN} />
      </div>

      {/* ── Body: sidebar + charts ── */}
      <div className="flex gap-6 items-start">

        {/* ── Sidebar ── */}
        <aside className="w-68 shrink-0 space-y-4">

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
            <CardContent className="space-y-0.5 max-h-64 overflow-y-auto pr-1">
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
            <CardHeader>
              <CardTitle className="text-xs uppercase tracking-wide text-gray-500 dark:text-gray-400">
                Backtest Period
                <span className="ml-1 font-normal normal-case text-gray-400">(trading days)</span>
              </CardTitle>
            </CardHeader>
            <CardContent className="flex gap-1.5 flex-wrap">
              {LOOKBACK_OPTIONS.map(opt => (
                <button key={opt.days} data-testid={`horizon-${opt.days}`}
                  onClick={() => setHorizon(opt.days)}
                  className={`rounded-md px-2.5 py-1 text-xs font-medium transition-colors ${
                    horizon === opt.days
                      ? 'bg-gray-900 dark:bg-white text-white dark:text-gray-900'
                      : 'bg-gray-100 dark:bg-gray-800 text-gray-700 dark:text-gray-300 hover:bg-gray-200 dark:hover:bg-gray-700'
                  }`}>
                  {opt.label}
                </button>
              ))}
            </CardContent>
          </Card>

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
                    {(singleRun.result.universe_size ?? singleRun.result.assets.length).toLocaleString()} instruments · {universe}
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
                        <th className="py-2 pr-4 text-left">Strategy</th>
                        <MetricTh col="Ann. Ret" />
                        <MetricTh col="Sharpe" />
                        <MetricTh col="Sortino" />
                        <MetricTh col="Max DD" />
                        <MetricTh col="Calmar" />
                        <MetricTh col="Vol" />
                        <MetricTh col="Win%" />
                        <MetricTh col="ES 95%" />
                        <MetricTh col="Jump VaR" />
                        <MetricTh col="Jump %" />
                        <MetricTh col="Turnover" className="pr-0" />
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
                            <td className="py-2.5 pr-3 text-right font-mono text-orange-500" title="1-day 95% VaR adjusted for jump-diffusion decomposition (arXiv:1803.07021)">
                              {m.jump_var_95 != null ? pct(m.jump_var_95) : '—'}
                            </td>
                            <td className="py-2.5 pr-3 text-right font-mono text-purple-500" title="Fraction of total variance from jump events (Poisson λ-weighted)">
                              {m.jump_variance_fraction != null ? pct(m.jump_variance_fraction, 0) : '—'}
                            </td>
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

          {/* ── AI Analysis Panel ── */}
          {activeRuns.length > 0 && (
            <Card>
              <CardHeader>
                <div className="flex items-center justify-between">
                  <div>
                    <CardTitle>AI Tearsheet Analysis</CardTitle>
                    <p className="text-xs text-gray-500 dark:text-gray-400 mt-0.5">
                      Post-doctoral institutional interpretation · llama-3.3-70b
                    </p>
                  </div>
                  <button
                    onClick={() => runAiAnalysis(activeRuns)}
                    disabled={aiLoading}
                    className="rounded-md bg-indigo-600 hover:bg-indigo-700 disabled:opacity-50 text-white px-3 py-1.5 text-xs font-semibold transition-colors flex items-center gap-1.5"
                  >
                    {aiLoading ? (
                      <>
                        <svg className="animate-spin h-3 w-3" viewBox="0 0 24 24" fill="none">
                          <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
                          <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8v8H4z" />
                        </svg>
                        Analyzing…
                      </>
                    ) : aiAnalysis ? 'Re-analyze' : 'Analyze with AI'}
                  </button>
                </div>
              </CardHeader>
              {(aiAnalysis || aiLoading || aiError) && (
                <CardContent>
                  {aiLoading && !aiAnalysis && (
                    <div className="space-y-2 animate-pulse">
                      {[...Array(6)].map((_, i) => (
                        <div key={i} className={`h-3 rounded bg-gray-200 dark:bg-gray-700 ${i % 3 === 2 ? 'w-2/3' : 'w-full'}`} />
                      ))}
                    </div>
                  )}
                  {aiError && (
                    <p className="text-sm text-red-500">{aiError}</p>
                  )}
                  {aiAnalysis && (
                    <div className="border border-indigo-100 dark:border-indigo-900/40 rounded-lg bg-indigo-50/40 dark:bg-indigo-950/20 p-4">
                      <MarkdownRenderer content={aiAnalysis} />
                    </div>
                  )}
                </CardContent>
              )}
            </Card>
          )}

          {singleRun && (
            <Card>
              <CardHeader>
                <CardTitle>
                  Asset Universe
                  {(singleRun.result.universe_size ?? 0) > singleRun.result.assets.length && (
                    <span className="ml-2 text-xs font-normal text-gray-400 dark:text-gray-500">
                      showing {singleRun.result.assets.length} of {(singleRun.result.universe_size ?? singleRun.result.assets.length).toLocaleString()} instruments
                    </span>
                  )}
                </CardTitle>
              </CardHeader>
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
