import {
  normalizeNav, drawdownSeries, mergeSeries, pct, isTopNApplicable,
  LOOKBACK_OPTIONS, TOP_N_OPTIONS, ASSET_TYPES,
  type PortfolioPoint,
} from '../pages/backtest'

// ─── normalizeNav ─────────────────────────────────────────────────────────────

describe('normalizeNav', () => {
  it('returns empty array for empty input', () => {
    expect(normalizeNav([])).toEqual([])
  })

  it('returns empty array when first value is zero', () => {
    const pts: PortfolioPoint[] = [{ date: '2025-01-01', value: 0 }, { date: '2025-01-02', value: 100 }]
    expect(normalizeNav(pts)).toEqual([])
  })

  it('rebases first value to 100', () => {
    const pts: PortfolioPoint[] = [
      { date: '2025-01-01', value: 200 },
      { date: '2025-01-02', value: 300 },
    ]
    const result = normalizeNav(pts)
    expect(result[0].value).toBe(100)
  })

  it('correctly scales subsequent values', () => {
    const pts: PortfolioPoint[] = [
      { date: '2025-01-01', value: 100 },
      { date: '2025-01-02', value: 150 },
      { date: '2025-01-03', value: 50 },
    ]
    const result = normalizeNav(pts)
    expect(result[0].value).toBe(100)
    expect(result[1].value).toBe(150)
    expect(result[2].value).toBe(50)
  })

  it('preserves dates', () => {
    const pts: PortfolioPoint[] = [
      { date: '2025-01-01', value: 100 },
      { date: '2025-06-01', value: 110 },
    ]
    const result = normalizeNav(pts)
    expect(result[0].date).toBe('2025-01-01')
    expect(result[1].date).toBe('2025-06-01')
  })

  it('rounds to 2 decimal places', () => {
    const pts: PortfolioPoint[] = [
      { date: '2025-01-01', value: 3 },
      { date: '2025-01-02', value: 1 },
    ]
    const result = normalizeNav(pts)
    // 1/3 * 100 = 33.333... → 33.33
    expect(result[1].value).toBe(33.33)
  })
})

// ─── drawdownSeries ───────────────────────────────────────────────────────────

describe('drawdownSeries', () => {
  it('returns empty array for empty input', () => {
    expect(drawdownSeries([])).toEqual([])
  })

  it('drawdown is 0 when portfolio is at all-time high', () => {
    const nav: PortfolioPoint[] = [
      { date: '2025-01-01', value: 100 },
      { date: '2025-01-02', value: 110 },
      { date: '2025-01-03', value: 120 },
    ]
    const result = drawdownSeries(nav)
    result.forEach(r => expect(r.dd).toBe(0))
  })

  it('computes correct peak-to-trough drawdown', () => {
    const nav: PortfolioPoint[] = [
      { date: '2025-01-01', value: 100 },
      { date: '2025-01-02', value: 120 },
      { date: '2025-01-03', value: 60 },  // 50% DD from peak of 120
    ]
    const result = drawdownSeries(nav)
    expect(result[0].dd).toBe(0)
    expect(result[1].dd).toBe(0)
    expect(result[2].dd).toBeCloseTo(-50, 1)
  })

  it('drawdown is non-positive (losses are negative)', () => {
    const nav: PortfolioPoint[] = [
      { date: '2025-01-01', value: 100 },
      { date: '2025-01-02', value: 80 },
      { date: '2025-01-03', value: 90 },
    ]
    const result = drawdownSeries(nav)
    result.forEach(r => expect(r.dd).toBeLessThanOrEqual(0))
  })

  it('recovers: drawdown returns to 0 at new ATH', () => {
    const nav: PortfolioPoint[] = [
      { date: '2025-01-01', value: 100 },
      { date: '2025-01-02', value: 80 },
      { date: '2025-01-03', value: 110 }, // new ATH
    ]
    const result = drawdownSeries(nav)
    expect(result[2].dd).toBe(0)
  })
})

// ─── mergeSeries ─────────────────────────────────────────────────────────────

describe('mergeSeries', () => {
  it('returns empty for empty input', () => {
    expect(mergeSeries<{ date: string; value: number }>([], 'value')).toEqual([])
  })

  it('merges two series by date', () => {
    const s1: { date: string; value: number }[] = [
      { date: '2025-01-01', value: 100 },
      { date: '2025-01-02', value: 110 },
    ]
    const s2: { date: string; value: number }[] = [
      { date: '2025-01-01', value: 200 },
      { date: '2025-01-02', value: 210 },
    ]
    const result = mergeSeries(
      [{ key: 'A', points: s1 }, { key: 'B', points: s2 }],
      'value',
    )
    expect(result).toHaveLength(2)
    expect(result[0]).toMatchObject({ date: '2025-01-01', A: 100, B: 200 })
    expect(result[1]).toMatchObject({ date: '2025-01-02', A: 110, B: 210 })
  })

  it('handles staggered dates — missing values are absent from row', () => {
    const s1: { date: string; value: number }[] = [{ date: '2025-01-01', value: 100 }]
    const s2: { date: string; value: number }[] = [{ date: '2025-01-02', value: 200 }]
    const result = mergeSeries(
      [{ key: 'A', points: s1 }, { key: 'B', points: s2 }],
      'value',
    )
    expect(result).toHaveLength(2)
    expect(result[0]['B']).toBeUndefined()
    expect(result[1]['A']).toBeUndefined()
  })

  it('dates are sorted ascending', () => {
    const pts: { date: string; value: number }[] = [
      { date: '2025-03-01', value: 1 },
      { date: '2025-01-01', value: 2 },
      { date: '2025-02-01', value: 3 },
    ]
    const result = mergeSeries([{ key: 'X', points: pts }], 'value')
    expect(result[0].date).toBe('2025-01-01')
    expect(result[1].date).toBe('2025-02-01')
    expect(result[2].date).toBe('2025-03-01')
  })

  it('deduplicates dates from multiple series', () => {
    const pts: { date: string; value: number }[] = [
      { date: '2025-01-01', value: 1 },
      { date: '2025-01-01', value: 2 }, // duplicate
    ]
    const result = mergeSeries([{ key: 'A', points: pts }], 'value')
    expect(result).toHaveLength(1)
  })
})

// ─── pct ─────────────────────────────────────────────────────────────────────

describe('pct', () => {
  it('formats 0.5 as 50.0%', () => { expect(pct(0.5)).toBe('50.0%') })
  it('formats 0 as 0.0%', () => { expect(pct(0)).toBe('0.0%') })
  it('formats -0.1 as -10.0%', () => { expect(pct(-0.1)).toBe('-10.0%') })
  it('respects decimal precision argument', () => { expect(pct(0.12345, 2)).toBe('12.35%') })
  it('formats 1.0 as 100.0%', () => { expect(pct(1.0)).toBe('100.0%') })
})

// ─── isTopNApplicable ─────────────────────────────────────────────────────────

describe('isTopNApplicable', () => {
  it('returns true for trad_fi with no asset-type filter', () => {
    expect(isTopNApplicable('trad_fi', [])).toBe(true)
  })

  it('returns true for hybrid with no asset-type filter', () => {
    expect(isTopNApplicable('hybrid', [])).toBe(true)
  })

  it('returns true for web3_crypto with no asset-type filter', () => {
    expect(isTopNApplicable('web3_crypto', [])).toBe(true)
  })

  it('returns false for web3_defi with no asset-type filter', () => {
    expect(isTopNApplicable('web3_defi', [])).toBe(false)
  })

  it('returns true when equity is in selected types', () => {
    expect(isTopNApplicable('web3_defi', ['equity'])).toBe(true)
  })

  it('returns true when derivatives is in selected types', () => {
    expect(isTopNApplicable('web3_defi', ['derivatives'])).toBe(true)
  })

  it('returns true when spot is in selected types', () => {
    expect(isTopNApplicable('web3_defi', ['spot'])).toBe(true)
  })

  it('returns false when only non-rankable types are selected', () => {
    expect(isTopNApplicable('trad_fi', ['lending', 'stablecoin_yield'])).toBe(false)
  })

  it('returns true when mix of rankable and non-rankable types selected', () => {
    expect(isTopNApplicable('web3_defi', ['lending', 'equity'])).toBe(true)
  })
})

// ─── Config integrity ────────────────────────────────────────────────────────

describe('LOOKBACK_OPTIONS', () => {
  it('contains exactly 8 options', () => {
    expect(LOOKBACK_OPTIONS).toHaveLength(8)
  })

  it('minimum is 1 month = 21 trading days', () => {
    expect(LOOKBACK_OPTIONS[0].days).toBe(21)
  })

  it('maximum is 5 trading years = 1260 trading days', () => {
    expect(LOOKBACK_OPTIONS[LOOKBACK_OPTIONS.length - 1].days).toBe(1260)
  })

  it('options are strictly ascending', () => {
    for (let i = 1; i < LOOKBACK_OPTIONS.length; i++) {
      expect(LOOKBACK_OPTIONS[i].days).toBeGreaterThan(LOOKBACK_OPTIONS[i - 1].days)
    }
  })

  it('all options are positive trading day counts', () => {
    LOOKBACK_OPTIONS.forEach(o => expect(o.days).toBeGreaterThan(0))
  })
})

describe('TOP_N_OPTIONS', () => {
  it('starts at 500', () => { expect(TOP_N_OPTIONS[0]).toBe(500) })
  it('ends at 3000', () => { expect(TOP_N_OPTIONS[TOP_N_OPTIONS.length - 1]).toBe(3000) })
  it('all values in [500, 3000]', () => {
    TOP_N_OPTIONS.forEach(n => {
      expect(n).toBeGreaterThanOrEqual(500)
      expect(n).toBeLessThanOrEqual(3000)
    })
  })
  it('is strictly ascending', () => {
    for (let i = 1; i < TOP_N_OPTIONS.length; i++) {
      expect(TOP_N_OPTIONS[i]).toBeGreaterThan(TOP_N_OPTIONS[i - 1])
    }
  })
})

describe('ASSET_TYPES rankable flags', () => {
  it('equity is rankable', () => {
    expect(ASSET_TYPES.find(a => a.id === 'equity')?.rankable).toBe(true)
  })
  it('derivatives is rankable', () => {
    expect(ASSET_TYPES.find(a => a.id === 'derivatives')?.rankable).toBe(true)
  })
  it('spot is rankable', () => {
    expect(ASSET_TYPES.find(a => a.id === 'spot')?.rankable).toBe(true)
  })
  it('lending is not rankable', () => {
    expect(ASSET_TYPES.find(a => a.id === 'lending')?.rankable).toBe(false)
  })
  it('stablecoin_yield is not rankable', () => {
    expect(ASSET_TYPES.find(a => a.id === 'stablecoin_yield')?.rankable).toBe(false)
  })
  it('defi_lp is not rankable', () => {
    expect(ASSET_TYPES.find(a => a.id === 'defi_lp')?.rankable).toBe(false)
  })
})
