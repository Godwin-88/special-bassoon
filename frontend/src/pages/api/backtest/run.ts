import type { NextApiRequest, NextApiResponse } from 'next'

// Thin proxy — all simulation logic lives in the Rust API (rl-agent/src/portfolio_sim.rs)
const RUST_API = process.env.RUST_API_URL ?? 'http://localhost:8080'

export default async function handler(req: NextApiRequest, res: NextApiResponse) {
  if (req.method !== 'POST') return res.status(405).end()

  try {
    const upstream = await fetch(`${RUST_API}/api/backtest`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(req.body),
    })
    const data = await upstream.json()
    if (!upstream.ok) return res.status(upstream.status).json(data)
    res.status(200).json(data)
  } catch (err: any) {
    res.status(502).json({ error: err.message ?? 'upstream error' })
  }
}
