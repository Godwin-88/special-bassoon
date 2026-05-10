import type { NextApiRequest, NextApiResponse } from 'next'

export interface ProtocolRow {
  rank: number
  slug: string
  name: string
  logo: string
  url: string
  category: string
  chains: string[]
  tvl: number
  tvlChange1d: number
  tvlChange7d: number
  fees24h: number | null
  fees7d: number | null
  revenue24h: number | null
  revenue7d: number | null
  volume24h: number | null
  volume7d: number | null
}

const RUST_API = process.env.RUST_API_URL ?? 'http://localhost:8080'

export default async function handler(_req: NextApiRequest, res: NextApiResponse) {
  try {
    const upstream = await fetch(`${RUST_API}/api/analytics/protocols`, {
      headers: { Accept: 'application/json' },
    })

    if (!upstream.ok) {
      const body = await upstream.text()
      return res.status(upstream.status).json({ error: body })
    }

    const data: any[] = await upstream.json()

    // Normalise field names from Rust snake_case → camelCase for the UI
    const rows: ProtocolRow[] = data.map((p, i) => ({
      rank: i + 1,
      slug: p.slug ?? '',
      name: p.name ?? '',
      logo: p.logo ?? '',
      url: p.url ?? '',
      category: p.category ?? 'Other',
      chains: Array.isArray(p.chains) ? p.chains : [],
      tvl: p.tvl ?? 0,
      tvlChange1d: p.tvl_change_1d ?? 0,
      tvlChange7d: p.tvl_change_7d ?? 0,
      fees24h: p.fees_24h ?? null,
      fees7d: p.fees_7d ?? null,
      revenue24h: p.revenue_24h ?? null,
      revenue7d: p.revenue_7d ?? null,
      volume24h: p.volume_24h ?? null,
      volume7d: p.volume_7d ?? null,
    }))

    const filtered = rows.filter(r => r.category?.toLowerCase() !== 'cex')

    res.setHeader('Cache-Control', 's-maxage=60, stale-while-revalidate=30')
    res.status(200).json(filtered)
  } catch (err: any) {
    res.status(502).json({ error: err.message ?? 'upstream error' })
  }
}
