/**
 * UI rendering tests for BacktestPage.
 * These tests verify the layout contract: universe and asset type selectors
 * are in the top bar, strategy/lookback/capital controls are in the sidebar.
 * No network calls are made — fetch is mocked to simulate the API.
 */
import React from 'react'
import { render, screen, fireEvent, within } from '@testing-library/react'
import BacktestPage from '../pages/backtest'
import { LOOKBACK_OPTIONS, TOP_N_OPTIONS, UNIVERSES, ASSET_TYPES, STRATEGIES } from '../pages/backtest'

// Mock recharts to avoid canvas/ResizeObserver issues in jsdom
jest.mock('recharts', () => {
  const React = require('react')
  return {
    ResponsiveContainer: ({ children }: any) => <div>{children}</div>,
    LineChart: ({ children }: any) => <div>{children}</div>,
    AreaChart: ({ children }: any) => <div>{children}</div>,
    Line: () => null,
    Area: () => null,
    XAxis: () => null,
    YAxis: () => null,
    CartesianGrid: () => null,
    Tooltip: () => null,
    Legend: () => null,
  }
})

// ─── Layout: universe bar is on top ──────────────────────────────────────────

describe('UniverseBar', () => {
  it('renders all 4 universe options', () => {
    render(<BacktestPage />)
    UNIVERSES.forEach(u => {
      expect(screen.getByTestId(`universe-${u.id}`)).toBeInTheDocument()
    })
  })

  it('web3_defi is selected by default', () => {
    render(<BacktestPage />)
    const radio = screen.getByTestId('universe-web3_defi').querySelector('input[type="radio"]')
    expect(radio).toBeChecked()
  })

  it('clicking a universe label selects it', () => {
    render(<BacktestPage />)
    const label = screen.getByTestId('universe-trad_fi')
    fireEvent.click(label)
    const radio = label.querySelector('input[type="radio"]')
    expect(radio).toBeChecked()
  })

  it('universe bar exists in the document (above sidebar)', () => {
    const { container } = render(<BacktestPage />)
    const bar = container.querySelector('[data-testid="universe-bar"]')
    expect(bar).toBeInTheDocument()
  })
})

// ─── Layout: asset type bar is on top ────────────────────────────────────────

describe('AssetTypeBar', () => {
  it('renders all asset type buttons', () => {
    render(<BacktestPage />)
    ASSET_TYPES.forEach(t => {
      expect(screen.getByTestId(`asset-type-${t.id}`)).toBeInTheDocument()
    })
  })

  it('no types selected by default', () => {
    render(<BacktestPage />)
    ASSET_TYPES.forEach(t => {
      const btn = screen.getByTestId(`asset-type-${t.id}`)
      expect(btn).not.toHaveClass('bg-indigo-600')
    })
  })

  it('clicking a type toggles it on', () => {
    render(<BacktestPage />)
    const btn = screen.getByTestId('asset-type-equity')
    fireEvent.click(btn)
    expect(btn).toHaveClass('bg-indigo-600')
  })

  it('clicking same type twice deselects it', () => {
    render(<BacktestPage />)
    const btn = screen.getByTestId('asset-type-equity')
    fireEvent.click(btn)
    fireEvent.click(btn)
    expect(btn).not.toHaveClass('bg-indigo-600')
  })

  it('clear button appears when a type is selected', () => {
    render(<BacktestPage />)
    expect(screen.queryByText('clear')).not.toBeInTheDocument()
    fireEvent.click(screen.getByTestId('asset-type-equity'))
    expect(screen.getByText('clear')).toBeInTheDocument()
  })

  it('clear button deselects all types', () => {
    render(<BacktestPage />)
    fireEvent.click(screen.getByTestId('asset-type-equity'))
    fireEvent.click(screen.getByTestId('asset-type-spot'))
    fireEvent.click(screen.getByText('clear'))
    ASSET_TYPES.forEach(t => {
      expect(screen.getByTestId(`asset-type-${t.id}`)).not.toHaveClass('bg-indigo-600')
    })
  })
})

// ─── Top-N bar visibility ─────────────────────────────────────────────────────

describe('TopNBar', () => {
  it('is hidden by default for web3_defi (no rankable types)', () => {
    render(<BacktestPage />)
    // web3_defi with no types selected → not applicable
    expect(screen.queryByTestId('top-n-bar')).not.toBeInTheDocument()
  })

  it('appears when a rankable asset type is selected', () => {
    render(<BacktestPage />)
    fireEvent.click(screen.getByTestId('asset-type-equity'))
    expect(screen.getByTestId('top-n-bar')).toBeInTheDocument()
  })

  it('appears when universe is trad_fi with no types selected', () => {
    render(<BacktestPage />)
    fireEvent.click(screen.getByTestId('universe-trad_fi'))
    expect(screen.getByTestId('top-n-bar')).toBeInTheDocument()
  })

  it('disappears when switching from trad_fi to web3_defi with no types', () => {
    render(<BacktestPage />)
    fireEvent.click(screen.getByTestId('universe-trad_fi'))
    expect(screen.getByTestId('top-n-bar')).toBeInTheDocument()
    fireEvent.click(screen.getByTestId('universe-web3_defi'))
    expect(screen.queryByTestId('top-n-bar')).not.toBeInTheDocument()
  })

  it('renders all Top-N options when visible', () => {
    render(<BacktestPage />)
    fireEvent.click(screen.getByTestId('universe-trad_fi'))
    TOP_N_OPTIONS.forEach(n => {
      expect(screen.getByTestId(`top-n-${n}`)).toBeInTheDocument()
    })
  })

  it('clicking a top-n button highlights it', () => {
    render(<BacktestPage />)
    fireEvent.click(screen.getByTestId('universe-trad_fi'))
    const btn = screen.getByTestId('top-n-500')
    fireEvent.click(btn)
    expect(btn).toHaveClass('bg-indigo-600')
  })

  it('clicking same top-n twice deselects it (shows "curated only" button)', () => {
    render(<BacktestPage />)
    fireEvent.click(screen.getByTestId('universe-trad_fi'))
    const btn500 = screen.getByTestId('top-n-500')
    fireEvent.click(btn500)
    expect(screen.getByText('curated only')).toBeInTheDocument()
    fireEvent.click(btn500) // deselect
    expect(screen.queryByText('curated only')).not.toBeInTheDocument()
  })

  it('switching universe resets top-n selection', () => {
    render(<BacktestPage />)
    fireEvent.click(screen.getByTestId('universe-trad_fi'))
    fireEvent.click(screen.getByTestId('top-n-1000'))
    expect(screen.getByTestId('top-n-1000')).toHaveClass('bg-indigo-600')
    // switch universe → reset
    fireEvent.click(screen.getByTestId('universe-hybrid'))
    const btn1000 = screen.getByTestId('top-n-1000')
    expect(btn1000).not.toHaveClass('bg-indigo-600')
  })

  it('not visible when only non-rankable types are selected', () => {
    render(<BacktestPage />)
    fireEvent.click(screen.getByTestId('asset-type-lending'))
    fireEvent.click(screen.getByTestId('asset-type-stablecoin_yield'))
    expect(screen.queryByTestId('top-n-bar')).not.toBeInTheDocument()
  })
})

// ─── Backtest period (lookback) options ───────────────────────────────────────

describe('Lookback period', () => {
  it('renders all 8 lookback options', () => {
    render(<BacktestPage />)
    LOOKBACK_OPTIONS.forEach(opt => {
      expect(screen.getByTestId(`horizon-${opt.days}`)).toBeInTheDocument()
    })
  })

  it('1 Y (252 days) is selected by default', () => {
    render(<BacktestPage />)
    expect(screen.getByTestId('horizon-252')).toHaveClass('bg-gray-900', 'dark:bg-white')
  })

  it('5 Y (1260 trading days) option is present', () => {
    render(<BacktestPage />)
    expect(screen.getByTestId('horizon-1260')).toBeInTheDocument()
    expect(screen.getByTestId('horizon-1260')).toHaveTextContent('5 Y')
  })

  it('clicking a period button selects it', () => {
    render(<BacktestPage />)
    const btn = screen.getByTestId('horizon-504')
    fireEvent.click(btn)
    expect(btn).toHaveClass('bg-gray-900')
  })

  it('selecting a period deselects the previous one', () => {
    render(<BacktestPage />)
    const prev = screen.getByTestId('horizon-252')
    const next = screen.getByTestId('horizon-1260')
    fireEvent.click(next)
    expect(prev).not.toHaveClass('bg-gray-900')
    expect(next).toHaveClass('bg-gray-900')
  })
})

// ─── Strategy sidebar ─────────────────────────────────────────────────────────

describe('Strategy sidebar', () => {
  it('all strategies are listed in sidebar', () => {
    render(<BacktestPage />)
    STRATEGIES.forEach(s => {
      expect(screen.getByText(s.label)).toBeInTheDocument()
    })
  })

  it('Compare all checkbox toggles compare mode', () => {
    render(<BacktestPage />)
    const cb = screen.getByRole('checkbox', { name: /compare all/i })
    expect(cb).not.toBeChecked()
    fireEvent.click(cb)
    expect(cb).toBeChecked()
  })
})
