import React, { useEffect, useState, useMemo, useRef } from 'react'
import type { ProtocolRow } from './api/analytics/protocols'
import { cn } from '@/lib/utils'
import MarkdownRenderer from '@/components/MarkdownRenderer'

// ─── Types ───────────────────────────────────────────────────────────────────

type SortKey = 'rank' | 'tvl' | 'tvlChange1d' | 'tvlChange7d' |
  'fees24h' | 'fees7d' | 'fees30d' |
  'revenue24h' | 'revenue7d' | 'revenue30d' |
  'volume24h' | 'volume7d' | 'volume30d'

type Horizon = '24h' | '7d' | '30d'

// ─── Constants ───────────────────────────────────────────────────────────────

const PINNED_CHAINS = ['All', 'Ethereum', 'BSC', 'Solana', 'Arbitrum', 'Base', 'Polygon', 'Avalanche', 'Optimism', 'Tron']

const CATEGORIES = [
  'All Categories', 'Dexes', 'Lending', 'Liquid Staking', 'Bridge', 'Yield',
  'CDP', 'Derivatives', 'RWA', 'Restaking', 'Yield Aggregator', 'Launchpad',
  'Insurance', 'Payments', 'Other',
]

// ─── Helpers ─────────────────────────────────────────────────────────────────

function fmtUSD(n: number | null, compact = true): string {
  if (n == null || !isFinite(n)) return '—'
  const abs = Math.abs(n)
  if (compact) {
    if (abs >= 1e9) return `$${(n / 1e9).toFixed(2)}B`
    if (abs >= 1e6) return `$${(n / 1e6).toFixed(2)}M`
    if (abs >= 1e3) return `$${(n / 1e3).toFixed(1)}K`
    return `$${n.toFixed(0)}`
  }
  return n.toLocaleString('en-US', { style: 'currency', currency: 'USD', maximumFractionDigits: 0 })
}

function fmtPct(n: number | null): string {
  if (n == null || !isFinite(n)) return '—'
  return `${n > 0 ? '+' : ''}${n.toFixed(2)}%`
}

function pctColor(n: number | null): string {
  if (n == null) return 'text-gray-400 dark:text-gray-500'
  if (n > 0) return 'text-emerald-600 dark:text-emerald-400'
  if (n < 0) return 'text-red-500 dark:text-red-400'
  return 'text-gray-500 dark:text-gray-400'
}

function getMetric(row: ProtocolRow, key: SortKey): number | null {
  return (row as any)[key] ?? null
}

// ─── Sub-components ──────────────────────────────────────────────────────────

function SortIcon({ active, dir }: { active: boolean; dir: 'asc' | 'desc' }) {
  if (!active) return <span className="ml-1 text-gray-300 dark:text-gray-600">↕</span>
  return <span className="ml-1">{dir === 'asc' ? '↑' : '↓'}</span>
}

function ChainBadge({ chain }: { chain: string }) {
  return (
    <span className="inline-block rounded px-1.5 py-0.5 text-[10px] font-medium bg-gray-100 dark:bg-gray-700 text-gray-600 dark:text-gray-300 mr-1 mb-0.5">
      {chain}
    </span>
  )
}

function CategoryBadge({ cat }: { cat: string }) {
  const palette: Record<string, string> = {
    'Dexes': 'bg-blue-100 text-blue-700 dark:bg-blue-900/40 dark:text-blue-300',
    'Lending': 'bg-violet-100 text-violet-700 dark:bg-violet-900/40 dark:text-violet-300',
    'Liquid Staking': 'bg-amber-100 text-amber-700 dark:bg-amber-900/40 dark:text-amber-300',
    'Bridge': 'bg-pink-100 text-pink-700 dark:bg-pink-900/40 dark:text-pink-300',
    'Yield': 'bg-emerald-100 text-emerald-700 dark:bg-emerald-900/40 dark:text-emerald-300',
    'CDP': 'bg-orange-100 text-orange-700 dark:bg-orange-900/40 dark:text-orange-300',
    'Derivatives': 'bg-red-100 text-red-700 dark:bg-red-900/40 dark:text-red-300',
    'Restaking': 'bg-cyan-100 text-cyan-700 dark:bg-cyan-900/40 dark:text-cyan-300',
    'RWA': 'bg-teal-100 text-teal-700 dark:bg-teal-900/40 dark:text-teal-300',
  }
  const cls = palette[cat] ?? 'bg-gray-100 text-gray-600 dark:bg-gray-700 dark:text-gray-300'
  return (
    <span className={`inline-block rounded-full px-2 py-0.5 text-[10px] font-semibold ${cls}`}>
      {cat}
    </span>
  )
}

function Skeleton() {
  return (
    <div className="animate-pulse space-y-2">
      {Array.from({ length: 15 }).map((_, i) => (
        <div key={i} className="h-10 rounded bg-gray-100 dark:bg-gray-800" />
      ))}
    </div>
  )
}

// ─── AI Agent Panel ───────────────────────────────────────────────────────────

function AgentPanel({
  protocols,
  context,
  onClose,
}: {
  protocols: ProtocolRow[]
  context: string
  onClose: () => void
}) {
  const [question, setQuestion] = useState('')
  const [analysis, setAnalysis] = useState('')
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState('')
  const inputRef = useRef<HTMLInputElement>(null)

  async function analyze(q?: string) {
    setLoading(true)
    setError('')
    setAnalysis('')
    try {
      const res = await fetch('/api/analytics/agent', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ protocols, question: q || question, context }),
      })
      const data = await res.json()
      if (!res.ok) throw new Error(data.error)
      setAnalysis(data.analysis)
    } catch (e: any) {
      setError(e.message)
    } finally {
      setLoading(false)
    }
  }

  useEffect(() => { analyze() }, [])

  return (
    <div className="flex flex-col border-t border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-900 p-4">
      <div className="flex items-center justify-between mb-3">
        <div className="flex items-center gap-2">
          <span className="h-2 w-2 rounded-full bg-emerald-500 animate-pulse" />
          <span className="text-sm font-semibold text-gray-800 dark:text-gray-100">AI Agent — Protocol Analysis</span>
          <span className="text-xs text-gray-400 dark:text-gray-500">Llama 3.3 70B via Groq</span>
        </div>
        <button
          onClick={onClose}
          className="text-gray-400 hover:text-gray-600 dark:text-gray-500 dark:hover:text-gray-300 text-lg leading-none"
        >
          ×
        </button>
      </div>

      {loading && (
        <div className="space-y-2 animate-pulse">
          <div className="h-4 bg-gray-200 dark:bg-gray-700 rounded w-3/4" />
          <div className="h-4 bg-gray-200 dark:bg-gray-700 rounded w-full" />
          <div className="h-4 bg-gray-200 dark:bg-gray-700 rounded w-5/6" />
          <div className="h-4 bg-gray-200 dark:bg-gray-700 rounded w-2/3" />
        </div>
      )}

      {error && <p className="text-sm text-red-500 dark:text-red-400">{error}</p>}

      {analysis && (
        <div className="max-h-56 overflow-y-auto mb-3">
          <MarkdownRenderer
            content={analysis}
            className="text-gray-800 dark:text-gray-200"
          />
        </div>
      )}

      <div className="flex gap-2 mt-auto">
        <input
          ref={inputRef}
          value={question}
          onChange={e => setQuestion(e.target.value)}
          onKeyDown={e => { if (e.key === 'Enter' && question.trim()) analyze(question) }}
          placeholder="Ask about these protocols…"
          className="flex-1 rounded-md border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100 placeholder:text-gray-400 dark:placeholder:text-gray-500 px-3 py-1.5 text-sm focus:outline-none focus:ring-2 focus:ring-gray-900 dark:focus:ring-gray-400"
        />
        <button
          onClick={() => question.trim() && analyze(question)}
          disabled={loading}
          className="rounded-md bg-gray-900 dark:bg-white px-3 py-1.5 text-sm text-white dark:text-gray-900 hover:bg-gray-700 dark:hover:bg-gray-200 disabled:opacity-50"
        >
          Ask
        </button>
        <button
          onClick={() => { setQuestion(''); analyze() }}
          disabled={loading}
          className="rounded-md border border-gray-200 dark:border-gray-700 px-3 py-1.5 text-sm text-gray-600 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800 disabled:opacity-50"
        >
          Re-analyse
        </button>
      </div>
    </div>
  )
}

// ─── Main Page ───────────────────────────────────────────────────────────────

export default function Analytics() {
  const [allRows, setAllRows] = useState<ProtocolRow[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState('')

  const [chain, setChain] = useState('All')
  const [category, setCategory] = useState('All Categories')
  const [search, setSearch] = useState('')
  const [horizon, setHorizon] = useState<Horizon>('24h')

  const [sortKey, setSortKey] = useState<SortKey>('tvl')
  const [sortDir, setSortDir] = useState<'asc' | 'desc'>('desc')

  const [agentOpen, setAgentOpen] = useState(false)
  const [selectedRow, setSelectedRow] = useState<ProtocolRow | null>(null)

  const availableChains = useMemo(() => {
    const counts = new Map<string, number>()
    for (const r of allRows) {
      for (const c of r.chains) counts.set(c, (counts.get(c) ?? 0) + 1)
    }
    const sorted = [...counts.entries()].sort((a, b) => b[1] - a[1]).map(([c]) => c)
    const pinned = PINNED_CHAINS.filter(c => c === 'All' || sorted.includes(c))
    const rest = sorted.filter(c => !PINNED_CHAINS.includes(c)).slice(0, 8)
    return [...pinned, ...rest]
  }, [allRows])

  useEffect(() => {
    fetch('/api/analytics/protocols')
      .then(r => r.json())
      .then(data => { setAllRows(Array.isArray(data) ? data : []); setLoading(false) })
      .catch(e => { setError(e.message); setLoading(false) })
  }, [])

  const filtered = useMemo(() => {
    let rows = allRows
    if (chain !== 'All') rows = rows.filter(r => r.chains.includes(chain))
    if (category !== 'All Categories') rows = rows.filter(r => r.category === category)
    if (search.trim()) {
      const q = search.toLowerCase()
      rows = rows.filter(r =>
        r.name.toLowerCase().includes(q) ||
        r.category.toLowerCase().includes(q) ||
        r.chains.some(c => c.toLowerCase().includes(q))
      )
    }
    return [...rows].sort((a, b) => {
      const av = getMetric(a, sortKey) ?? -Infinity
      const bv = getMetric(b, sortKey) ?? -Infinity
      return sortDir === 'desc' ? bv - av : av - bv
    })
  }, [allRows, chain, category, search, sortKey, sortDir])

  function toggleSort(key: SortKey) {
    if (sortKey === key) setSortDir(d => d === 'desc' ? 'asc' : 'desc')
    else { setSortKey(key); setSortDir('desc') }
  }

  function Th({ label, sk }: { label: string; sk: SortKey }) {
    return (
      <th
        className="cursor-pointer select-none whitespace-nowrap px-3 py-2 text-right text-xs font-semibold text-gray-500 dark:text-gray-400 hover:text-gray-900 dark:hover:text-white"
        onClick={() => toggleSort(sk)}
      >
        {label}
        <SortIcon active={sortKey === sk} dir={sortDir} />
      </th>
    )
  }

  const feeKey: SortKey = `fees${horizon}` as SortKey
  const revKey: SortKey = `revenue${horizon}` as SortKey
  const volKey: SortKey = `volume${horizon}` as SortKey
  const tvlChangeKey: SortKey = horizon === '24h' ? 'tvlChange1d' : 'tvlChange7d'

  const agentProtocols = selectedRow ? [selectedRow] : filtered.slice(0, 20)
  const agentContext = selectedRow
    ? `Single protocol selected: ${selectedRow.name} (${selectedRow.category})`
    : `Showing top ${agentProtocols.length} of ${filtered.length} protocols. Filter: chain=${chain}, category=${category}, sorted by ${sortKey} ${sortDir}.`

  return (
    <div className="min-h-screen bg-gray-50 dark:bg-gray-950">
      {/* Header */}
      <div className="border-b border-gray-200 dark:border-gray-800 bg-white dark:bg-gray-900 px-6 py-4">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <div>
            <h1 className="text-xl font-bold text-gray-900 dark:text-white">Protocol Analytics</h1>
            <p className="text-xs text-gray-500 dark:text-gray-400 mt-0.5">
              {loading ? 'Loading…' : `${filtered.length} protocols · TVL, fees, revenue, volume`}
            </p>
          </div>

          <div className="flex flex-wrap items-center gap-2">
            <input
              value={search}
              onChange={e => setSearch(e.target.value)}
              placeholder="Search protocol…"
              className="rounded-md border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100 placeholder:text-gray-400 dark:placeholder:text-gray-500 px-3 py-1.5 text-sm focus:outline-none focus:ring-2 focus:ring-gray-900 dark:focus:ring-gray-400 w-44"
            />

            <select
              value={category}
              onChange={e => setCategory(e.target.value)}
              className="rounded-md border border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 text-gray-900 dark:text-gray-100 px-3 py-1.5 text-sm focus:outline-none focus:ring-2 focus:ring-gray-900 dark:focus:ring-gray-400"
            >
              {CATEGORIES.map(c => <option key={c}>{c}</option>)}
            </select>

            <div className="flex rounded-md border border-gray-200 dark:border-gray-700 overflow-hidden bg-white dark:bg-gray-800">
              {(['24h', '7d', '30d'] as Horizon[]).map(h => (
                <button
                  key={h}
                  onClick={() => setHorizon(h)}
                  className={cn(
                    'px-3 py-1.5 text-xs font-medium transition-colors',
                    horizon === h
                      ? 'bg-gray-900 dark:bg-white text-white dark:text-gray-900'
                      : 'text-gray-600 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-700'
                  )}
                >
                  {h}
                </button>
              ))}
            </div>

            <button
              onClick={() => { setSelectedRow(null); setAgentOpen(o => !o) }}
              className={cn(
                'flex items-center gap-1.5 rounded-md border px-3 py-1.5 text-sm font-medium transition-colors',
                agentOpen
                  ? 'border-emerald-400 bg-emerald-50 dark:bg-emerald-900/20 text-emerald-700 dark:text-emerald-400'
                  : 'border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800 text-gray-600 dark:text-gray-300 hover:bg-gray-50 dark:hover:bg-gray-700'
              )}
            >
              <span className={cn('h-1.5 w-1.5 rounded-full', agentOpen ? 'bg-emerald-500 animate-pulse' : 'bg-gray-400 dark:bg-gray-500')} />
              AI Analysis
            </button>
          </div>
        </div>

        {/* Chain tabs */}
        <div className="mt-3 flex flex-wrap gap-1">
          {availableChains.map(c => (
            <button
              key={c}
              onClick={() => setChain(c)}
              className={cn(
                'rounded-full px-3 py-1 text-xs font-medium transition-colors',
                chain === c
                  ? 'bg-gray-900 dark:bg-white text-white dark:text-gray-900'
                  : 'bg-gray-100 dark:bg-gray-800 text-gray-600 dark:text-gray-300 hover:bg-gray-200 dark:hover:bg-gray-700'
              )}
            >
              {c}
            </button>
          ))}
        </div>
      </div>

      {/* Table */}
      <div className="flex-1 overflow-hidden">
        {error && (
          <div className="m-6 rounded-lg border border-red-200 dark:border-red-900 bg-red-50 dark:bg-red-900/20 p-4 text-sm text-red-600 dark:text-red-400">
            Failed to load data: {error}
          </div>
        )}

        {loading ? (
          <div className="p-6"><Skeleton /></div>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead className="sticky top-0 bg-white dark:bg-gray-900 border-b border-gray-200 dark:border-gray-800 z-10">
                <tr>
                  <th className="cursor-pointer select-none px-3 py-2 text-left text-xs font-semibold text-gray-500 dark:text-gray-400 hover:text-gray-900 dark:hover:text-white" onClick={() => toggleSort('rank')}>
                    # <SortIcon active={sortKey === 'rank'} dir={sortDir} />
                  </th>
                  <th className="px-3 py-2 text-left text-xs font-semibold text-gray-500 dark:text-gray-400">Protocol</th>
                  <th className="px-3 py-2 text-left text-xs font-semibold text-gray-500 dark:text-gray-400">Category</th>
                  <th className="px-3 py-2 text-left text-xs font-semibold text-gray-500 dark:text-gray-400">Chains</th>
                  <Th label="TVL" sk="tvl" />
                  <Th label={`TVL Δ ${horizon === '30d' ? '7d' : horizon}`} sk={tvlChangeKey} />
                  <Th label={`Fees ${horizon}`} sk={feeKey} />
                  <Th label={`Revenue ${horizon}`} sk={revKey} />
                  <Th label={`Volume ${horizon}`} sk={volKey} />
                  <th className="px-3 py-2 text-xs font-semibold text-gray-500 dark:text-gray-400" />
                </tr>
              </thead>
              <tbody className="divide-y divide-gray-100 dark:divide-gray-800">
                {filtered.length === 0 ? (
                  <tr>
                    <td colSpan={10} className="py-16 text-center text-sm text-gray-400 dark:text-gray-500">
                      No protocols match the current filters.
                    </td>
                  </tr>
                ) : (
                  filtered.map(row => {
                    const isSelected = selectedRow?.slug === row.slug
                    return (
                      <tr
                        key={row.slug}
                        className={cn(
                          'group hover:bg-blue-50 dark:hover:bg-blue-900/10 transition-colors cursor-pointer',
                          isSelected && 'bg-blue-50 dark:bg-blue-900/20'
                        )}
                        onClick={() => {
                          setSelectedRow(prev => prev?.slug === row.slug ? null : row)
                          setAgentOpen(true)
                        }}
                      >
                        <td className="px-3 py-2.5 text-gray-400 dark:text-gray-500 text-xs">{row.rank}</td>
                        <td className="px-3 py-2.5">
                          <div className="flex items-center gap-2">
                            {row.logo ? (
                              // eslint-disable-next-line @next/next/no-img-element
                              <img src={row.logo} alt={row.name} className="h-5 w-5 rounded-full object-contain" onError={e => { (e.target as HTMLImageElement).style.display = 'none' }} />
                            ) : (
                              <div className="h-5 w-5 rounded-full bg-gray-200 dark:bg-gray-700 flex items-center justify-center text-[8px] text-gray-500 dark:text-gray-400">{row.name[0]}</div>
                            )}
                            <span className="font-medium text-gray-900 dark:text-gray-100">{row.name}</span>
                          </div>
                        </td>
                        <td className="px-3 py-2.5"><CategoryBadge cat={row.category} /></td>
                        <td className="px-3 py-2.5 max-w-[160px]">
                          <div className="flex flex-wrap">
                            {row.chains.slice(0, 4).map(c => <ChainBadge key={c} chain={c} />)}
                            {row.chains.length > 4 && (
                              <span className="text-[10px] text-gray-400 dark:text-gray-500">+{row.chains.length - 4}</span>
                            )}
                          </div>
                        </td>
                        <td className="px-3 py-2.5 text-right font-medium text-gray-900 dark:text-gray-100">{fmtUSD(row.tvl)}</td>
                        <td className={cn('px-3 py-2.5 text-right text-xs', pctColor(getMetric(row, tvlChangeKey)))}>
                          {fmtPct(getMetric(row, tvlChangeKey))}
                        </td>
                        <td className="px-3 py-2.5 text-right text-gray-700 dark:text-gray-300">{fmtUSD(getMetric(row, feeKey))}</td>
                        <td className="px-3 py-2.5 text-right text-gray-700 dark:text-gray-300">{fmtUSD(getMetric(row, revKey))}</td>
                        <td className="px-3 py-2.5 text-right text-gray-700 dark:text-gray-300">{fmtUSD(getMetric(row, volKey))}</td>
                        <td className="px-3 py-2.5 text-right">
                          <span className={cn(
                            'text-[10px] px-1.5 py-0.5 rounded opacity-0 group-hover:opacity-100 transition-opacity',
                            isSelected ? 'opacity-100 bg-blue-600 text-white' : 'bg-gray-100 dark:bg-gray-800 text-gray-500 dark:text-gray-400'
                          )}>
                            {isSelected ? 'selected' : 'analyse'}
                          </span>
                        </td>
                      </tr>
                    )
                  })
                )}
              </tbody>
            </table>
          </div>
        )}
      </div>

      {/* AI Agent Panel — sticky bottom */}
      {agentOpen && (
        <div className="sticky bottom-0 z-20 border-t border-gray-200 dark:border-gray-700 shadow-lg">
          <AgentPanel
            protocols={agentProtocols}
            context={agentContext}
            onClose={() => { setAgentOpen(false); setSelectedRow(null) }}
          />
        </div>
      )}
    </div>
  )
}
