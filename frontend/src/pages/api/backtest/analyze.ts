import type { NextApiRequest, NextApiResponse } from 'next'

const GROQ_URL = 'https://api.groq.com/openai/v1/chat/completions'
const GROQ_MODEL = 'llama-3.3-70b-versatile'

export default async function handler(req: NextApiRequest, res: NextApiResponse) {
  if (req.method !== 'POST') return res.status(405).end()

  const apiKey = process.env.GROQ_API_KEY?.trim()
  if (!apiKey) return res.status(503).json({ error: 'GROQ_API_KEY not configured' })

  const { runs, config } = req.body as {
    runs: {
      label: string
      color: string
      metrics: {
        annualised_return: number
        sharpe: number
        sortino: number
        max_drawdown: number
        calmar: number
        win_rate: number
        volatility: number
        es95: number
        turnover: number
        jump_var_95?: number
        jump_variance_fraction?: number
        jump_lambda?: number
      }
    }[]
    config: {
      universe: string
      asset_types: string[]
      days: number
      initial_capital: number
      risk_profile: string
      top_n?: number
    }
  }

  if (!runs?.length) return res.status(400).json({ error: 'No simulation results provided' })

  function pct(v: number, d = 2) { return `${(v * 100).toFixed(d)}%` }
  function fmt(v: number, d = 3) { return v.toFixed(d) }

  const tableRows = runs.map(r => {
    const m = r.metrics
    return `| ${r.label} | ${pct(m.annualised_return)} | ${fmt(m.sharpe)} | ${fmt(m.sortino)} | ${pct(m.max_drawdown)} | ${fmt(m.calmar)} | ${pct(m.volatility)} | ${pct(m.win_rate)} | ${pct(m.es95)} | ${m.jump_var_95 != null ? pct(m.jump_var_95) : '—'} | ${m.jump_variance_fraction != null ? pct(m.jump_variance_fraction, 0) : '—'} | ${m.turnover.toFixed(1)}x |`
  }).join('\n')

  const table = `| Strategy | Ann. Return | Sharpe | Sortino | Max DD | Calmar | Volatility | Win Rate | ES 95% | Jump VaR 95% | Jump Var% | Turnover |
|----------|------------|--------|---------|--------|--------|-----------|----------|--------|-------------|----------|----------|
${tableRows}`

  const configLine = [
    `Universe: ${config.universe}`,
    config.asset_types.length ? `Asset types: ${config.asset_types.join(', ')}` : null,
    `Horizon: ${config.days} trading days (~${(config.days / 252).toFixed(1)} years)`,
    `Capital: $${config.initial_capital.toLocaleString()}`,
    `Risk profile: ${config.risk_profile}`,
    config.top_n ? `Universe size: Top ${config.top_n}` : null,
  ].filter(Boolean).join(' | ')

  const systemPrompt = `You are a post-doctoral quantitative portfolio analyst and risk engineer at a tier-1 institutional asset manager (think Two Sigma, AQR, or Citadel). Your audience is a sophisticated professional — a financial engineer who built these simulations and wants rigorous interpretation, not definitions.

ANALYTICAL FRAMEWORK — apply without exception:

1. RETURN QUALITY: Annualised return alone is meaningless without risk adjustment. Lead with Sharpe and Sortino. A Sharpe > 1.0 is institutionally acceptable; > 2.0 is exceptional in live trading (suspect in simulation without transaction costs). A Sortino >> Sharpe implies the strategy profits asymmetrically — left-tail losses are small relative to upside variance.

2. DRAWDOWN ANALYSIS: Max drawdown is a path-dependent metric. Pair it with Calmar (Ann.Return / MaxDD). Calmar > 1.0 means the strategy earns more than its worst drawdown annually — the minimum threshold for a serious allocation. ES95 (Expected Shortfall at 95%) is the tail CVaR — it captures the average loss in the worst 5% of days; this is what risk committees care about, not VaR.

3. JUMP RISK DECOMPOSITION (Spadafora et al. arXiv:1803.07021): Jump VaR 95% is the 1-day 95% VaR decomposed into diffusive (GBM) and jump (Poisson compound) components. Jump Variance % is the fraction of total return variance attributable to jump events. A Jump Var% > 40% signals the strategy's risk profile is dominated by discontinuous jumps — regime changes, liquidity crises — rather than diffusive volatility. This distinction matters for hedging: diffusive risk is hedgeable with delta; jump risk requires gamma/vega or tail protection.

4. TURNOVER & IMPLEMENTATION: Annualised turnover expressed as portfolio turns. 1x = complete portfolio replaced once per year. High-turnover strategies (>10x) have severely eroded Sharpe after realistic transaction costs (assume 10-30bps all-in for institutional execution). Flag any strategy where transaction-cost-adjusted Sharpe likely turns negative.

5. WIN RATE: The fraction of trading days with positive returns. A win rate below 50% can still be profitable if the strategy has positive skew (rare large wins). Pair with Sortino: if win_rate < 50% but Sortino > Sharpe, the strategy is negatively skewed in frequency but positively skewed in magnitude.

6. CROSS-STRATEGY COMPARISON (if multiple strategies): Identify Pareto-dominant strategies (better Sharpe AND lower drawdown). Flag dominated strategies explicitly. Note correlation in returns structure — strategies with similar signal logic likely have correlated drawdowns, negating diversification.

7. SIMULATION CAVEATS: This is a GBM simulation with correlated assets. State clearly: (a) GBM assumes log-normally distributed returns — it systematically underestimates tail risk, (b) Jump VaR addresses this partially but the Poisson λ is estimated from simulated data, (c) transaction costs in the simulation use 5bps flat slippage — live execution will be 2-6x higher for large universes, (d) no market impact model.

OUTPUT STRUCTURE (follow exactly, use markdown headers):
1. **Risk-Adjusted Performance Summary** — rank strategies by Sharpe; identify leaders and laggards with one-line rationale
2. **Drawdown & Tail Risk Profile** — Calmar analysis, ES95 interpretation, jump risk fraction signal
3. **Strategy-Specific Signals** — for each strategy, one or two sentences on what the metric pattern implies about the signal's behaviour in the simulated market regime
4. **Implementation Friction Assessment** — turnover-adjusted Sharpe estimate; flag strategies likely destroyed by realistic transaction costs
5. **Portfolio Construction Implication** — which 1-2 strategies would survive institutional due diligence and why; note any combination that offers uncorrelated risk profiles

Keep response under 700 words. No definitions of basic metrics. Be direct, opinionated, and precise.`

  const userPrompt = `Simulation configuration: ${configLine}

${table}

Provide the institutional-grade tearsheet interpretation.`

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
        max_tokens: 1000,
        temperature: 0.15,
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
