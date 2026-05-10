//! Multi-asset portfolio simulation engine.
//!
//! All strategies from cypher/12_algo_trading_strategies.cypher are implemented here.
//! Called by the API's /api/backtest endpoint; the Next.js UI is a thin rendering shell only.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc, Duration};

// ─── Asset Universe ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum UniverseId {
    Web3Defi,
    Web3Crypto,
    Hybrid,
    TradFi,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AssetType {
    DefiLp,
    Lending,
    Derivatives,
    Spot,
    Equity,
    Etf,
    Commodity,
    StablecoinYield,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StrategyId {
    EqualWeight,
    Momentum,
    MeanReversion,
    TrendFollowing,
    RiskParity,
    Kelly,
    DeltaNeutral,
    QuantValue,
    StatArb,
    MlAlpha,
    Canslim,
    LiquidityProvisionOpt,
}

#[derive(Debug, Clone)]
struct Asset {
    id: &'static str,
    label: &'static str,
    asset_type: AssetType,
    universes: &'static [&'static str],
    mu: f64,      // annualised drift
    sigma: f64,   // annualised volatility
    cluster: usize,
}

static ASSETS: &[Asset] = &[
    // ── Web3 DeFi ──────────────────────────────────────────────
    Asset { id: "uni_v3_eth_usdc",  label: "Uniswap V3 ETH/USDC",     asset_type: AssetType::DefiLp,          universes: &["web3_defi","hybrid"], mu: 0.45, sigma: 0.80, cluster: 0 },
    Asset { id: "curve_3pool",      label: "Curve 3Pool",              asset_type: AssetType::StablecoinYield, universes: &["web3_defi","hybrid"], mu: 0.08, sigma: 0.04, cluster: 0 },
    Asset { id: "aave_usdc",        label: "Aave USDC Lending",        asset_type: AssetType::Lending,         universes: &["web3_defi","hybrid"], mu: 0.06, sigma: 0.03, cluster: 0 },
    Asset { id: "gmx_glp",          label: "GMX GLP",                  asset_type: AssetType::Derivatives,     universes: &["web3_defi","hybrid"], mu: 0.35, sigma: 0.60, cluster: 0 },
    Asset { id: "lido_steth",       label: "Lido stETH",               asset_type: AssetType::Lending,         universes: &["web3_defi","web3_crypto","hybrid"], mu: 0.05, sigma: 0.45, cluster: 1 },
    Asset { id: "compound_eth",     label: "Compound ETH",             asset_type: AssetType::Lending,         universes: &["web3_defi","hybrid"], mu: 0.04, sigma: 0.42, cluster: 1 },
    Asset { id: "balancer_80_20",   label: "Balancer 80/20 BAL/ETH",   asset_type: AssetType::DefiLp,          universes: &["web3_defi","hybrid"], mu: 0.30, sigma: 0.70, cluster: 0 },
    Asset { id: "pendle_pt_steth",  label: "Pendle PT-stETH",          asset_type: AssetType::Derivatives,     universes: &["web3_defi"],          mu: 0.09, sigma: 0.15, cluster: 0 },
    Asset { id: "convex_crv",       label: "Convex CRV Boost",         asset_type: AssetType::DefiLp,          universes: &["web3_defi"],          mu: 0.22, sigma: 0.65, cluster: 0 },
    Asset { id: "maker_dsr",        label: "MakerDAO DSR",             asset_type: AssetType::StablecoinYield, universes: &["web3_defi","hybrid"], mu: 0.05, sigma: 0.01, cluster: 0 },
    // ── Web3 Spot ──────────────────────────────────────────────
    Asset { id: "eth",  label: "ETH",  asset_type: AssetType::Spot, universes: &["web3_crypto","hybrid"], mu: 0.50, sigma: 0.75, cluster: 1 },
    Asset { id: "btc",  label: "BTC",  asset_type: AssetType::Spot, universes: &["web3_crypto","hybrid"], mu: 0.55, sigma: 0.65, cluster: 2 },
    Asset { id: "sol",  label: "SOL",  asset_type: AssetType::Spot, universes: &["web3_crypto"],          mu: 0.65, sigma: 0.95, cluster: 1 },
    Asset { id: "bnb",  label: "BNB",  asset_type: AssetType::Spot, universes: &["web3_crypto"],          mu: 0.40, sigma: 0.80, cluster: 1 },
    Asset { id: "arb",  label: "ARB",  asset_type: AssetType::Spot, universes: &["web3_crypto"],          mu: 0.35, sigma: 1.05, cluster: 1 },
    // ── Traditional Finance ────────────────────────────────────
    Asset { id: "spy",      label: "SPY (S&P 500)",      asset_type: AssetType::Etf,       universes: &["trad_fi","hybrid"], mu: 0.12, sigma: 0.18, cluster: 3 },
    Asset { id: "qqq",      label: "QQQ (Nasdaq)",       asset_type: AssetType::Etf,       universes: &["trad_fi","hybrid"], mu: 0.15, sigma: 0.22, cluster: 3 },
    Asset { id: "tlt",      label: "TLT (20Y Treasury)", asset_type: AssetType::Etf,       universes: &["trad_fi","hybrid"], mu: 0.03, sigma: 0.14, cluster: 4 },
    Asset { id: "gld",      label: "GLD (Gold)",         asset_type: AssetType::Commodity, universes: &["trad_fi","hybrid"], mu: 0.08, sigma: 0.16, cluster: 4 },
    Asset { id: "iwm",      label: "IWM (Russell 2000)", asset_type: AssetType::Etf,       universes: &["trad_fi"],          mu: 0.10, sigma: 0.24, cluster: 3 },
    Asset { id: "hyg",      label: "HYG (High Yield)",   asset_type: AssetType::Etf,       universes: &["trad_fi","hybrid"], mu: 0.05, sigma: 0.10, cluster: 4 },
    Asset { id: "uso",      label: "USO (Crude Oil)",    asset_type: AssetType::Commodity, universes: &["trad_fi"],          mu: 0.07, sigma: 0.35, cluster: 5 },
    Asset { id: "vix_short",label: "VIX Short Vol",      asset_type: AssetType::Derivatives,universes: &["trad_fi"],         mu: 0.20, sigma: 0.50, cluster: 5 },
];

// ─── Correlation model ────────────────────────────────────────────────────────

fn cross_corr(a: usize, b: usize) -> f64 {
    if a == b { return 0.75; }
    let (lo, hi) = (a.min(b), a.max(b));
    match (lo, hi) {
        (0, 1) => 0.40, (0, 2) => 0.15, (0, 3) => 0.10, (0, 4) => -0.05, (0, 5) => 0.05,
        (1, 2) => 0.65, (1, 3) => 0.25, (1, 4) => -0.10, (1, 5) => 0.10,
        (2, 3) => 0.20, (2, 4) => -0.05, (2, 5) => 0.08,
        (3, 4) => -0.15, (3, 5) => 0.05,
        (4, 5) => 0.00,
        _ => 0.05,
    }
}

// Lower Cholesky decomposition of the correlation matrix
fn cholesky(assets: &[&Asset]) -> Vec<Vec<f64>> {
    let n = assets.len();
    let mut corr: Vec<Vec<f64>> = (0..n).map(|i| {
        (0..n).map(|j| {
            if i == j { 1.0 } else { cross_corr(assets[i].cluster, assets[j].cluster) }
        }).collect()
    }).collect();

    let mut l: Vec<Vec<f64>> = vec![vec![0.0; n]; n];
    for i in 0..n {
        for j in 0..=i {
            let mut s = corr[i][j];
            for k in 0..j { s -= l[i][k] * l[j][k]; }
            l[i][j] = if i == j { s.max(1e-12).sqrt() } else { s / (l[j][j] + 1e-12) };
        }
    }
    l
}

// ─── PRNG + Box-Muller ────────────────────────────────────────────────────────

struct Rng(u32);
impl Rng {
    fn new(seed: u32) -> Self { Self(seed) }
    fn next_f64(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0 as f64 / u32::MAX as f64
    }
    fn normal(&mut self) -> f64 {
        let u1 = self.next_f64().max(1e-12);
        let u2 = self.next_f64();
        (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
    }
}

fn generate_returns(assets: &[&Asset], days: usize, seed: u32) -> Vec<Vec<f64>> {
    let n = assets.len();
    let l = cholesky(assets);
    let mut rng = Rng::new(seed);
    let dt: f64 = 1.0 / 252.0;
    let sqrt_dt = dt.sqrt();

    (0..days).map(|_| {
        let z: Vec<f64> = (0..n).map(|_| rng.normal()).collect();
        let corr_z: Vec<f64> = (0..n).map(|i| {
            (0..=i).map(|j| l[i][j] * z[j]).sum::<f64>()
        }).collect();
        (0..n).map(|i| {
            let mu = assets[i].mu;
            let sigma = assets[i].sigma;
            (mu - 0.5 * sigma * sigma) * dt + sigma * sqrt_dt * corr_z[i]
        }).collect()
    }).collect()
}

// ─── Strategy implementations ─────────────────────────────────────────────────

fn equal_weights(n: usize) -> Vec<f64> { vec![1.0 / n as f64; n] }

fn normalise_pos(w: Vec<f64>) -> Vec<f64> {
    let total: f64 = w.iter().sum();
    if total < 1e-10 { return equal_weights(w.len()); }
    w.into_iter().map(|v| v / total).collect()
}

fn momentum_weights(returns: &[Vec<f64>], t: usize, lookback: usize) -> Vec<f64> {
    let n = returns[0].len();
    if t < lookback { return equal_weights(n); }
    let mom: Vec<f64> = (0..n).map(|i| {
        returns[(t - lookback)..t].iter().map(|day| day[i]).sum::<f64>()
    }).collect();
    normalise_pos(mom.into_iter().map(|v| v.max(0.0)).collect())
}

fn mean_reversion_weights(prices: &[Vec<f64>], t: usize, window: usize) -> Vec<f64> {
    let n = prices[0].len();
    if t < window { return equal_weights(n); }
    let ma: Vec<f64> = (0..n).map(|i| {
        prices[(t - window)..t].iter().map(|p| p[i]).sum::<f64>() / window as f64
    }).collect();
    let dists: Vec<f64> = (0..n).map(|i| (ma[i] - prices[t][i]).max(0.0)).collect();
    normalise_pos(dists)
}

fn trend_following_weights(prices: &[Vec<f64>], t: usize, short_w: usize, long_w: usize) -> Vec<f64> {
    let n = prices[0].len();
    if t < long_w { return equal_weights(n); }
    let short_ma: Vec<f64> = (0..n).map(|i| {
        prices[(t - short_w)..t].iter().map(|p| p[i]).sum::<f64>() / short_w as f64
    }).collect();
    let long_ma: Vec<f64> = (0..n).map(|i| {
        prices[(t - long_w)..t].iter().map(|p| p[i]).sum::<f64>() / long_w as f64
    }).collect();
    let signals: Vec<f64> = (0..n).map(|i| if short_ma[i] > long_ma[i] { 1.0 } else { 0.0 }).collect();
    let count: f64 = signals.iter().sum();
    if count < 1e-10 { equal_weights(n) } else { signals.into_iter().map(|s| s / count).collect() }
}

fn risk_parity_weights(returns: &[Vec<f64>], t: usize, window: usize) -> Vec<f64> {
    let n = returns[0].len();
    if t < window { return equal_weights(n); }
    let slice = &returns[(t - window)..t];
    let means: Vec<f64> = (0..n).map(|i| slice.iter().map(|d| d[i]).sum::<f64>() / window as f64).collect();
    let vols: Vec<f64> = (0..n).map(|i| {
        let var = slice.iter().map(|d| (d[i] - means[i]).powi(2)).sum::<f64>() / window as f64;
        var.sqrt().max(1e-6)
    }).collect();
    normalise_pos(vols.into_iter().map(|v| 1.0 / v).collect())
}

fn kelly_weights(returns: &[Vec<f64>], t: usize, window: usize) -> Vec<f64> {
    let n = returns[0].len();
    if t < window { return equal_weights(n); }
    let slice = &returns[(t - window)..t];
    let means: Vec<f64> = (0..n).map(|i| slice.iter().map(|d| d[i]).sum::<f64>() / window as f64).collect();
    let vars: Vec<f64> = (0..n).map(|i| {
        (slice.iter().map(|d| (d[i] - means[i]).powi(2)).sum::<f64>() / window as f64).max(1e-12)
    }).collect();
    // Kelly fraction f* = µ/σ², capped at 40% per asset
    let kelly: Vec<f64> = (0..n).map(|i| (means[i] / vars[i]).max(0.0).min(0.40)).collect();
    normalise_pos(kelly)
}

fn delta_neutral_weights(assets: &[&Asset]) -> Vec<f64> {
    let w: Vec<f64> = assets.iter().map(|a| {
        match a.asset_type {
            AssetType::StablecoinYield | AssetType::Lending => 1.0,
            _ => 0.0,
        }
    }).collect();
    normalise_pos(w)
}

fn quant_value_weights(returns: &[Vec<f64>], t: usize, window: usize, n: usize) -> Vec<f64> {
    if t < window { return equal_weights(n); }
    let slice = &returns[(t - window)..t];
    let means: Vec<f64> = (0..n).map(|i| slice.iter().map(|d| d[i]).sum::<f64>() / window as f64).collect();
    let vols: Vec<f64> = (0..n).map(|i| {
        let var = slice.iter().map(|d| (d[i] - means[i]).powi(2)).sum::<f64>() / window as f64;
        var.sqrt().max(1e-6)
    }).collect();
    // Rank by risk-adjusted return (Sharpe proxy ≈ P/TVL)
    let mut scores: Vec<(usize, f64)> = (0..n).map(|i| (i, means[i] / vols[i])).collect();
    scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let top_k = (n / 3).max(1);
    let mut w = vec![0.0f64; n];
    for &(idx, _) in &scores[..top_k] { w[idx] = 1.0 / top_k as f64; }
    w
}

fn stat_arb_weights(returns: &[Vec<f64>], t: usize, window: usize) -> Vec<f64> {
    let n = returns[0].len();
    if t < window || n < 2 { return equal_weights(n); }
    // Cumulative return over window
    let cum: Vec<f64> = (0..n).map(|i| {
        returns[(t - window)..t].iter().fold(1.0f64, |acc, d| acc * (1.0 + d[i]))
    }).collect();
    let mut sorted = cum.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median = sorted[n / 2];
    // Long recent underperformers (mean-reversion bet); see Chan "Quantitative Trading"
    let w: Vec<f64> = cum.iter().map(|&r| if r < median { 1.0 } else { 0.0 }).collect();
    normalise_pos(w)
}

fn ml_alpha_weights(returns: &[Vec<f64>], prices: &[Vec<f64>], t: usize) -> Vec<f64> {
    let n = returns[0].len();
    let window = 20usize.min(t);
    if window < 5 { return equal_weights(n); }
    // Ensemble: 50% momentum + 50% mean-reversion — approximates stacked signal
    let mw = momentum_weights(returns, t, window);
    let mrw = mean_reversion_weights(prices, t, window);
    (0..n).map(|i| 0.5 * mw[i] + 0.5 * mrw[i]).collect()
}

fn liquidity_provision_weights(assets: &[&Asset], returns: &[Vec<f64>], t: usize, window: usize) -> Vec<f64> {
    let n = assets.len();
    if t < window { return equal_weights(n); }
    // Optimal LP: maximize fee_APY / realized_vol (net APY ≈ fee_APY × (1 - IL_rate))
    // For DeFi LP assets use low-vol weighting; for non-LP use risk-parity
    let slice = &returns[(t - window)..t];
    let means: Vec<f64> = (0..n).map(|i| slice.iter().map(|d| d[i]).sum::<f64>() / window as f64).collect();
    let vols: Vec<f64> = (0..n).map(|i| {
        let var = slice.iter().map(|d| (d[i] - means[i]).powi(2)).sum::<f64>() / window as f64;
        var.sqrt().max(1e-6)
    }).collect();
    let w: Vec<f64> = (0..n).map(|i| {
        let lp_bonus = match assets[i].asset_type {
            AssetType::DefiLp | AssetType::StablecoinYield => 1.5,
            _ => 1.0,
        };
        lp_bonus / vols[i]
    }).collect();
    normalise_pos(w)
}

// ─── Metrics computation ──────────────────────────────────────────────────────

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct PortfolioPoint {
    pub date: String,
    pub value: f64,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct AssetInfo {
    pub id: String,
    pub label: String,
    pub asset_type: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct SimMetrics {
    pub annualised_return: f64,
    pub sharpe: f64,
    pub sortino: f64,
    pub max_drawdown: f64,
    pub calmar: f64,
    pub win_rate: f64,
    pub volatility: f64,
    pub es95: f64,
    pub turnover: f64,
    pub portfolio_history: Vec<PortfolioPoint>,
}

fn compute_metrics(
    nav: &[f64],
    daily_returns: &[f64],
    all_weights: &[Vec<f64>],
    start_date: DateTime<Utc>,
) -> SimMetrics {
    let n = nav.len();
    if n < 2 {
        return SimMetrics {
            annualised_return: 0.0, sharpe: 0.0, sortino: 0.0, max_drawdown: 0.0,
            calmar: 0.0, win_rate: 0.0, volatility: 0.0, es95: 0.0, turnover: 0.0,
            portfolio_history: vec![],
        };
    }

    let ann = 252.0_f64;
    let rf_daily = 0.05 / ann;

    let total_return = nav[n - 1] / nav[0] - 1.0;
    let ann_return = (1.0 + total_return).powf(ann / (n - 1) as f64) - 1.0;
    let ann_return = if ann_return.is_finite() { ann_return } else { 0.0 };

    let excess: Vec<f64> = daily_returns.iter().map(|r| r - rf_daily).collect();
    let mean_ex: f64 = excess.iter().sum::<f64>() / excess.len() as f64;
    let var_ex: f64 = excess.iter().map(|r| (r - mean_ex).powi(2)).sum::<f64>() / excess.len() as f64;
    let vol = (var_ex * ann).sqrt();
    let vol = if vol.is_finite() { vol } else { 0.0 };
    let sharpe = if vol > 1e-10 { (mean_ex * ann) / vol } else { 0.0 };

    let down: Vec<f64> = excess.iter().filter(|&&r| r < 0.0).cloned().collect();
    let down_var = if down.is_empty() { var_ex } else { down.iter().map(|r| r.powi(2)).sum::<f64>() / down.len() as f64 };
    let sortino = if down_var > 1e-10 { (mean_ex * ann) / (down_var * ann).sqrt() } else { 0.0 };
    let sortino = if sortino.is_finite() { sortino } else { 0.0 };

    let mut peak = nav[0];
    let mut max_dd = 0.0f64;
    for &v in nav {
        if v > peak { peak = v; }
        max_dd = max_dd.max((peak - v) / peak);
    }

    let calmar = if max_dd > 1e-6 { ann_return / max_dd } else { 0.0 };
    let win_rate = daily_returns.iter().filter(|&&r| r > 0.0).count() as f64 / daily_returns.len() as f64;

    let mut sorted_ret: Vec<f64> = daily_returns.to_vec();
    sorted_ret.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let cutoff = ((sorted_ret.len() as f64 * 0.05) as usize).max(1);
    let es95 = -sorted_ret[..cutoff].iter().sum::<f64>() / cutoff as f64;

    let turnover: f64 = if all_weights.len() > 1 {
        let l1_sum: f64 = all_weights.windows(2).map(|w| {
            w[0].iter().zip(w[1].iter()).map(|(a, b)| (b - a).abs()).sum::<f64>()
        }).sum();
        (l1_sum / (all_weights.len() - 1) as f64) * ann
    } else { 0.0 };

    let sample = (n / 120).max(1);
    let portfolio_history: Vec<PortfolioPoint> = nav.iter().enumerate()
        .filter(|(i, _)| i % sample == 0 || *i == n - 1)
        .map(|(i, &v)| {
            let dt = start_date + Duration::days((i as f64 * (n as f64 / n as f64)) as i64);
            PortfolioPoint {
                date: dt.format("%Y-%m-%d").to_string(),
                value: (v * 100.0).round() / 100.0,
            }
        })
        .collect();

    SimMetrics { annualised_return: ann_return, sharpe, sortino, max_drawdown: max_dd, calmar, win_rate, volatility: vol, es95, turnover, portfolio_history }
}

// ─── Public API ───────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub struct StrategyParams {
    pub lookback: Option<usize>,
    pub short_window: Option<usize>,
    pub long_window: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SimRequest {
    pub universe: UniverseId,
    pub asset_types: Option<Vec<AssetType>>,
    pub strategy: StrategyId,
    pub params: Option<StrategyParams>,
    pub days: Option<usize>,
    pub initial_capital: Option<f64>,
    pub risk_profile: Option<String>,
    pub seed: Option<u32>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct SimResult {
    pub metrics: SimMetrics,
    pub assets: Vec<AssetInfo>,
}


pub fn run_simulation(req: SimRequest) -> Result<SimResult, String> {
    let universe_key = match req.universe {
        UniverseId::Web3Defi   => "web3_defi",
        UniverseId::Web3Crypto => "web3_crypto",
        UniverseId::Hybrid     => "hybrid",
        UniverseId::TradFi     => "trad_fi",
    };

    let type_filter = req.asset_types.unwrap_or_default();
    let selected: Vec<&Asset> = ASSETS.iter()
        .filter(|a| a.universes.contains(&universe_key))
        .filter(|a| type_filter.is_empty() || type_filter.contains(&a.asset_type))
        .collect();

    if selected.is_empty() {
        return Err("No assets matched the selected universe and asset-type filters".into());
    }

    let days = req.days.unwrap_or(365).min(1460);
    let initial_capital = req.initial_capital.unwrap_or(100_000.0);
    let seed = req.seed.unwrap_or(42);
    let params = req.params.unwrap_or(StrategyParams { lookback: None, short_window: None, long_window: None });

    // Leverage cap by risk profile
    let leverage_cap: f64 = match req.risk_profile.as_deref().unwrap_or("moderate") {
        "conservative" => 0.50,
        "aggressive"   => 1.00,
        _              => 0.80,
    };

    let lookback   = params.lookback.unwrap_or(20);
    let short_w    = params.short_window.unwrap_or(10);
    let long_w     = params.long_window.unwrap_or(50);
    let n = selected.len();

    // GBM returns: days × n
    let raw_returns = generate_returns(&selected, days, seed);

    // Build price index for strategies that need it
    let mut prices: Vec<Vec<f64>> = vec![vec![100.0f64; n]];
    for day in &raw_returns {
        let prev = prices.last().unwrap();
        prices.push((0..n).map(|i| prev[i] * (1.0 + day[i])).collect());
    }

    // Rebalance frequency by strategy type
    let rebal_freq: usize = match req.strategy {
        StrategyId::EqualWeight | StrategyId::DeltaNeutral | StrategyId::LiquidityProvisionOpt => 21,
        StrategyId::StatArb | StrategyId::MeanReversion => 3,
        _ => 5,
    };

    let mut nav = vec![initial_capital];
    let mut port_returns: Vec<f64> = Vec::with_capacity(days);
    let mut all_weights: Vec<Vec<f64>> = Vec::with_capacity(days);
    let mut current_weights = equal_weights(n);

    for t in 0..days {
        if t % rebal_freq == 0 {
            let raw_w: Vec<f64> = match req.strategy {
                StrategyId::EqualWeight          => equal_weights(n),
                StrategyId::Momentum             => momentum_weights(&raw_returns, t, lookback),
                StrategyId::Canslim              => momentum_weights(&raw_returns, t, lookback.max(50)),
                StrategyId::MeanReversion        => mean_reversion_weights(&prices, t, lookback),
                StrategyId::TrendFollowing       => trend_following_weights(&prices, t, short_w, long_w),
                StrategyId::RiskParity           => risk_parity_weights(&raw_returns, t, lookback),
                StrategyId::Kelly                => kelly_weights(&raw_returns, t, lookback),
                StrategyId::DeltaNeutral         => delta_neutral_weights(&selected),
                StrategyId::QuantValue           => quant_value_weights(&raw_returns, t, lookback, n),
                StrategyId::StatArb              => stat_arb_weights(&raw_returns, t, lookback),
                StrategyId::MlAlpha              => ml_alpha_weights(&raw_returns, &prices, t),
                StrategyId::LiquidityProvisionOpt => liquidity_provision_weights(&selected, &raw_returns, t, lookback),
            };
            // Apply leverage cap
            current_weights = raw_w.into_iter().map(|w| w * leverage_cap).collect();
        }

        all_weights.push(current_weights.clone());

        let port_return: f64 = current_weights.iter()
            .zip(raw_returns[t].iter())
            .map(|(w, r)| w * r)
            .sum();

        // Proportional slippage (0.05% per rebalance weight moved)
        let slippage = if t % rebal_freq == 0 {
            current_weights.iter().map(|w| w * 0.0005).sum::<f64>()
        } else { 0.0 };

        let net = port_return - slippage;
        port_returns.push(net);
        nav.push(nav.last().unwrap() * (1.0 + net));
    }

    let start_date = Utc::now() - Duration::days(days as i64);
    let metrics = compute_metrics(&nav, &port_returns, &all_weights, start_date);

    let assets = selected.iter().map(|a| AssetInfo {
        id: a.id.to_string(),
        label: a.label.to_string(),
        asset_type: format!("{:?}", a.asset_type).to_lowercase(),
    }).collect();

    Ok(SimResult { metrics, assets })
}
