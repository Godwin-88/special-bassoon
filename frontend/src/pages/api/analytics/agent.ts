import type { NextApiRequest, NextApiResponse } from 'next'

const GROQ_URL = 'https://api.groq.com/openai/v1/chat/completions'
const GROQ_MODEL = 'llama-3.3-70b-versatile'
const RUST_API = process.env.RUST_API_URL ?? 'http://localhost:8080'

const CEX_NAMES = new Set([
  'binance', 'bybit', 'okx', 'coinbase', 'kraken', 'htx', 'bitget', 'kucoin',
  'gemini', 'bitfinex', 'mexc', 'gate', 'bitmex', 'deribit', 'bitstamp',
  'crypto.com', 'crypto-com', 'robinhood', 'poloniex', 'hashkey', 'bitkub',
  'osl hk', 'swissborg', 'bitmart', 'whitebit',
])

function isCEX(p: any): boolean {
  if (p.category?.toLowerCase() === 'cex') return true
  const nameLower = (p.name ?? '').toLowerCase()
  for (const cex of CEX_NAMES) {
    if (nameLower.includes(cex)) return true
  }
  return false
}

function safe(n: number | null | undefined): number | null {
  return (n != null && isFinite(n) && n > 0) ? n : null
}

function annYield(daily: number | null, tvl: number): number | null {
  if (!safe(daily) || !safe(tvl)) return null
  return (daily! * 365 / tvl) * 100
}

function fmtPct(n: number | null): string {
  if (n == null) return '—'
  return n.toFixed(3) + '%'
}

function fmtUSD(n: number | null): string {
  if (n == null) return '—'
  if (n >= 1e9) return `$${(n/1e9).toFixed(2)}B`
  if (n >= 1e6) return `$${(n/1e6).toFixed(2)}M`
  if (n >= 1e3) return `$${(n/1e3).toFixed(1)}K`
  return `$${n.toFixed(0)}`
}

export default async function handler(req: NextApiRequest, res: NextApiResponse) {
  if (req.method !== 'POST') return res.status(405).end()

  const { protocols: rawProtocols, question, context } = req.body as {
    protocols: any[]
    question?: string
    context?: string
  }

  if (!rawProtocols?.length) return res.status(400).json({ error: 'No protocols provided' })

  const apiKey = process.env.GROQ_API_KEY?.trim()
  if (!apiKey) return res.status(503).json({ error: 'GROQ_API_KEY not configured' })

  // 1. Filter CEX — they are not DeFi protocols
  const protocols = rawProtocols.filter(p => !isCEX(p))

  // 2. Compute institutional metrics
  const enriched = protocols.slice(0, 20).map(p => {
    const tvl = safe(p.tvl) ?? 0
    return {
      ...p,
      fee_yield_ann: annYield(safe(p.fees24h), tvl),
      rev_yield_ann: annYield(safe(p.revenue24h), tvl),
      fee_capture_rate: (safe(p.fees24h) && safe(p.revenue24h))
        ? (p.revenue24h / p.fees24h) * 100 : null,
      vol_tvl_ratio: (safe(p.volume24h) && safe(tvl))
        ? (p.volume24h / tvl) * 100 : null,
    }
  })

  // 3. Fetch Neo4j protocol context
  let graphContext = ''
  try {
    const names = enriched.map(p => p.name).join(',')
    const ctxRes = await fetch(`${RUST_API}/api/protocol-context?names=${encodeURIComponent(names)}`, {
      headers: { Accept: 'application/json' },
    })
    if (ctxRes.ok) {
      const ctxData: any[] = await ctxRes.json()
      if (ctxData.length > 0) {
        graphContext = '\n\n## Knowledge Graph Context\n' + ctxData.map(c => {
          const parts = [`**${c.name}** (${c.category})`]
          if (c.description) parts.push(`Mechanism: ${c.description.slice(0, 200)}`)
          if (c.interest_rate_model) parts.push(`Rate model: ${c.interest_rate_model}`)
          if (c.mechanism) parts.push(`Invariant: ${c.mechanism.slice(0, 100)}`)
          if (c.fee_tiers) parts.push(`Fee tiers: ${Array.isArray(c.fee_tiers) ? c.fee_tiers.join(', ') : c.fee_tiers}`)
          if (c.concepts?.length) parts.push(`Implements: ${c.concepts.join(', ')}`)
          return parts.join(' | ')
        }).join('\n')
      }
    }
  } catch (_) { /* graph context is enrichment, not critical */ }

  // 4. Build institutional metrics table (annualised)
  const tableRows = enriched.map(p =>
    `| ${p.name} | ${p.category} | ${fmtUSD(p.tvl)} | ${fmtPct(p.fee_yield_ann)} | ${fmtPct(p.rev_yield_ann)} | ${p.fee_capture_rate ? p.fee_capture_rate.toFixed(1)+'%' : '—'} | ${p.vol_tvl_ratio ? p.vol_tvl_ratio.toFixed(2)+'%' : '—'} | ${p.tvlChange7d != null ? (p.tvlChange7d > 0 ? '+' : '') + p.tvlChange7d.toFixed(2)+'%' : '—'} |`
  ).join('\n')

  const table = `| Protocol | Category | TVL | Fee Yield Ann | Rev Yield Ann | Fee Capture | Vol/TVL | TVL 7d Δ |
|----------|----------|-----|---------------|---------------|-------------|---------|----------|
${tableRows}`

  const systemPrompt = `You are a quantitative DeFi research analyst at an institutional asset manager. Your output is read by portfolio managers and risk committees. Apply rigorous financial methodology.

ANALYTICAL FRAMEWORK — follow these rules without exception:

1. OPPORTUNITY RANKING: Rank by annualised fee yield (Fee Yield Ann column), not TVL. TVL is a size metric, not a return metric. Weight by conviction: high (>5% ann yield, >6 months live, audited), medium (2-5% ann yield or newer), low (<2% or missing data).

2. METRICS INTERPRETATION: All fee/revenue yields in the table are already annualised (daily × 365 / TVL). State them as such. Never present raw 24h numbers as the primary metric.

3. LENDING PROTOCOLS (Aave, Compound, MakerDAO): Fees are a direct function of utilisation rate U, governed by a kink model: R = R0 + U×R1 if U<Uopt, else R0 + R1 + ((U-Uopt)/(1-Uopt))×R2. High fees relative to TVL reflect HIGH UTILISATION — this is EXPECTED and HEALTHY, not anomalous. Never flag normal lending protocol fee generation as unusual.

4. AMM CAPITAL EFFICIENCY (Uniswap V3): The stated TVL includes out-of-range liquidity earning zero fees. Vol/TVL ratio is the correct efficiency proxy — it captures active capital deployment. A high vol/tvl with lower TVL can outrank a large pool with low vol/tvl.

5. RISK SIGNALS: 1-day TVL changes are dominated by ETH/BTC price movements and single large LP exits — they are NOISE, not signals. Only flag TVL as a risk signal if the 7d change exceeds -5% AND vol/tvl ratio is simultaneously declining (liquidity withdrawal with reduced activity). Do not flag normal daily fluctuations.

6. VETO RULE — TOKENOMICS: For Curve Finance, base fee yield understates true LP APY which depends on veCRV gauge weight allocation. Flag explicitly: "actual Curve LP yield = base fee yield + CRV emissions * gauge_weight_allocation — requires checking current gauge distribution." Same for Balancer (veBAL).

7. FEE CAPTURE RATE: This is the protocol's share of gross fees. Low capture (< 20%) = most fees go to LPs (LP-friendly, lower protocol revenue risk). High capture (> 60%) = protocol extractive, watch for LP migration pressure. N/A = fee switch off or data unavailable.

8. SMART CONTRACT RISK DISCOUNT: Apply an implicit risk premium for protocols < 12 months old, lacking major audits, or with no track record through adverse market conditions. State this explicitly in your conviction rating.

9. DATA COMPLETENESS: When fees/revenue/volume are missing (shown as —), do NOT impute or estimate. State "data unavailable — cannot assess yield" and exclude from rankings.

OUTPUT STRUCTURE (follow exactly):
1. **Risk-Adjusted Yield Leaders** — top 3 ranked by Fee Yield Ann, with conviction level and key qualifier
2. **Capital Efficiency Analysis** — Vol/TVL leaders and what the ratio implies for active LP returns
3. **Risk Signal Inventory** — structural risks only (sustained 7d declines, fee capture pressure, tokenomics dependencies); exclude single-day noise
4. **Data Quality Notes** — protocols with missing metrics that cannot be assessed
5. **Bottom Line** — one paragraph, portfolio construction implication

Keep the response under 500 words. Use precise terminology.`

  const userPrompt = question
    ? `${context ? `Filter context: ${context}\n\n` : ''}${table}${graphContext}\n\nAnalyst question: ${question}`
    : `${context ? `Filter context: ${context}\n\n` : ''}${table}${graphContext}\n\nProvide the institutional analysis following the output structure.`

  try {
    const response = await fetch(GROQ_URL, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Authorization': `Bearer ${apiKey}`,
      },
      body: JSON.stringify({
        model: GROQ_MODEL,
        messages: [
          { role: 'system', content: systemPrompt },
          { role: 'user', content: userPrompt },
        ],
        max_tokens: 800,
        temperature: 0.2,
      }),
    })

    if (!response.ok) {
      const err = await response.text()
      return res.status(response.status).json({ error: err })
    }

    const data = await response.json()
    const content = data.choices?.[0]?.message?.content ?? ''
    res.status(200).json({ analysis: content, model: GROQ_MODEL })
  } catch (err: any) {
    res.status(502).json({ error: err.message })
  }
}
