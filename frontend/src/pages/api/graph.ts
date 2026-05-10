import type { NextApiRequest, NextApiResponse } from 'next'

export default async function handler(req: NextApiRequest, res: NextApiResponse) {
  try {
    const response = await fetch('http://localhost:8080/api/graph')
    const data = await response.json()
    res.status(200).json(data)
  } catch {
    res.status(503).json({ error: 'API unavailable' })
  }
}
