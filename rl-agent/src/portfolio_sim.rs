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

// ─── Owned working asset (used throughout the simulation) ────────────────────

#[derive(Debug, Clone)]
pub struct WorkAsset {
    pub id: String,
    pub label: String,
    pub asset_type: AssetType,
    pub mu: f64,       // annualised drift
    pub sigma: f64,    // annualised volatility
    pub cluster: usize,
}

// ─── Static curated asset catalogue ──────────────────────────────────────────

struct StaticAsset {
    id: &'static str,
    label: &'static str,
    asset_type: AssetType,
    universes: &'static [&'static str],
    mu: f64,
    sigma: f64,
    cluster: usize,
}

static ASSETS: &[StaticAsset] = &[
    // ── Web3 DeFi ──────────────────────────────────────────────
    StaticAsset { id: "uni_v3_eth_usdc",  label: "Uniswap V3 ETH/USDC",     asset_type: AssetType::DefiLp,          universes: &["web3_defi","hybrid"], mu: 0.45, sigma: 0.80, cluster: 0 },
    StaticAsset { id: "curve_3pool",      label: "Curve 3Pool",              asset_type: AssetType::StablecoinYield, universes: &["web3_defi","hybrid"], mu: 0.08, sigma: 0.04, cluster: 0 },
    StaticAsset { id: "aave_usdc",        label: "Aave USDC Lending",        asset_type: AssetType::Lending,         universes: &["web3_defi","hybrid"], mu: 0.06, sigma: 0.03, cluster: 0 },
    StaticAsset { id: "gmx_glp",          label: "GMX GLP",                  asset_type: AssetType::Derivatives,     universes: &["web3_defi","hybrid"], mu: 0.35, sigma: 0.60, cluster: 0 },
    StaticAsset { id: "lido_steth",       label: "Lido stETH",               asset_type: AssetType::Lending,         universes: &["web3_defi","web3_crypto","hybrid"], mu: 0.05, sigma: 0.45, cluster: 1 },
    StaticAsset { id: "compound_eth",     label: "Compound ETH",             asset_type: AssetType::Lending,         universes: &["web3_defi","hybrid"], mu: 0.04, sigma: 0.42, cluster: 1 },
    StaticAsset { id: "balancer_80_20",   label: "Balancer 80/20 BAL/ETH",   asset_type: AssetType::DefiLp,          universes: &["web3_defi","hybrid"], mu: 0.30, sigma: 0.70, cluster: 0 },
    StaticAsset { id: "pendle_pt_steth",  label: "Pendle PT-stETH",          asset_type: AssetType::Derivatives,     universes: &["web3_defi"],          mu: 0.09, sigma: 0.15, cluster: 0 },
    StaticAsset { id: "convex_crv",       label: "Convex CRV Boost",         asset_type: AssetType::DefiLp,          universes: &["web3_defi"],          mu: 0.22, sigma: 0.65, cluster: 0 },
    StaticAsset { id: "maker_dsr",        label: "MakerDAO DSR",             asset_type: AssetType::StablecoinYield, universes: &["web3_defi","hybrid"], mu: 0.05, sigma: 0.01, cluster: 0 },
    // ── Web3 Spot ──────────────────────────────────────────────
    StaticAsset { id: "eth",  label: "ETH",  asset_type: AssetType::Spot, universes: &["web3_crypto","hybrid"], mu: 0.50, sigma: 0.75, cluster: 1 },
    StaticAsset { id: "btc",  label: "BTC",  asset_type: AssetType::Spot, universes: &["web3_crypto","hybrid"], mu: 0.55, sigma: 0.65, cluster: 2 },
    StaticAsset { id: "sol",  label: "SOL",  asset_type: AssetType::Spot, universes: &["web3_crypto"],          mu: 0.65, sigma: 0.95, cluster: 1 },
    StaticAsset { id: "bnb",  label: "BNB",  asset_type: AssetType::Spot, universes: &["web3_crypto"],          mu: 0.40, sigma: 0.80, cluster: 1 },
    StaticAsset { id: "arb",  label: "ARB",  asset_type: AssetType::Spot, universes: &["web3_crypto"],          mu: 0.35, sigma: 1.05, cluster: 1 },
    // ── Traditional Finance ────────────────────────────────────
    StaticAsset { id: "spy",      label: "SPY (S&P 500)",      asset_type: AssetType::Etf,        universes: &["trad_fi","hybrid"], mu: 0.12, sigma: 0.18, cluster: 3 },
    StaticAsset { id: "qqq",      label: "QQQ (Nasdaq)",       asset_type: AssetType::Etf,        universes: &["trad_fi","hybrid"], mu: 0.15, sigma: 0.22, cluster: 3 },
    StaticAsset { id: "tlt",      label: "TLT (20Y Treasury)", asset_type: AssetType::Etf,        universes: &["trad_fi","hybrid"], mu: 0.03, sigma: 0.14, cluster: 4 },
    StaticAsset { id: "gld",      label: "GLD (Gold)",         asset_type: AssetType::Commodity,  universes: &["trad_fi","hybrid"], mu: 0.08, sigma: 0.16, cluster: 4 },
    StaticAsset { id: "iwm",      label: "IWM (Russell 2000)", asset_type: AssetType::Etf,        universes: &["trad_fi"],          mu: 0.10, sigma: 0.24, cluster: 3 },
    StaticAsset { id: "hyg",      label: "HYG (High Yield)",   asset_type: AssetType::Etf,        universes: &["trad_fi","hybrid"], mu: 0.05, sigma: 0.10, cluster: 4 },
    StaticAsset { id: "uso",      label: "USO (Crude Oil)",    asset_type: AssetType::Commodity,  universes: &["trad_fi"],          mu: 0.07, sigma: 0.35, cluster: 5 },
    StaticAsset { id: "vix_short",label: "VIX Short Vol",      asset_type: AssetType::Derivatives, universes: &["trad_fi"],         mu: 0.20, sigma: 0.50, cluster: 5 },
];

// ─── Public catalogue access ──────────────────────────────────────────────────

#[derive(Debug, Serialize, Clone)]
pub struct CatalogueEntry {
    pub id: String,
    pub label: String,
    pub asset_type: String,
    pub universes: Vec<String>,
}

/// Returns the full curated asset catalogue, optionally filtered by universe key.
pub fn list_catalogue(universe: Option<&str>) -> Vec<CatalogueEntry> {
    ASSETS.iter()
        .filter(|a| universe.map_or(true, |u| a.universes.contains(&u)))
        .map(|a| CatalogueEntry {
            id: a.id.to_string(),
            label: a.label.to_string(),
            asset_type: serde_json::to_string(&a.asset_type)
                .unwrap_or_default()
                .trim_matches('"')
                .to_string(),
            universes: a.universes.iter().map(|u| u.to_string()).collect(),
        })
        .collect()
}

// ─── Top-N synthetic universe (power-law market-cap distribution) ─────────────
//
// Asset types eligible for ranked Top-N simulation:
// Equity (stocks), Derivatives (futures/options), Spot (tokens) — all have
// liquid rank-ordered universes where top_n is a meaningful filter.
//
// Mathematical model (Gabaix 2009 power law, Fama-French size premium):
//   mu(r)   = µ_base + µ_size × ln(N/r) / ln(N)      [r = rank, 1 = largest]
//   sigma(r) = σ_base × (1 + 0.4 × ln(r) / ln(N))   [small-caps more volatile]
//   cluster  = r * NUM_CLUSTERS / N                   [geo-economic clustering]

fn is_top_n_eligible(at: &AssetType) -> bool {
    matches!(at, AssetType::Equity | AssetType::Derivatives | AssetType::Spot)
}

fn base_params_for_type(at: &AssetType) -> (f64, f64) {
    match at {
        AssetType::Equity      => (0.12, 0.20),
        AssetType::Derivatives => (0.20, 0.50),
        AssetType::Spot        => (0.40, 0.65),
        _                      => (0.10, 0.20),
    }
}

pub fn build_synthetic_universe(asset_type: &AssetType, top_n: usize) -> Vec<WorkAsset> {
    let (mu_base, sigma_base) = base_params_for_type(asset_type);
    let mu_size_premium = 0.05_f64;  // Fama-French SMB proxy
    let n_clusters = 8usize;
    let ln_n = (top_n as f64).ln().max(1.0);

    (1..=top_n).map(|r| {
        let rank_frac = r as f64 / top_n as f64;
        let ln_r = (r as f64).ln();
        // Fama-French SMB: smaller rank number = larger cap = lower expected return
        let mu    = mu_base + mu_size_premium * rank_frac;
        let sigma = sigma_base * (1.0 + 0.4 * ln_r / ln_n);
        let cluster = ((r - 1) * n_clusters / top_n).min(n_clusters - 1);

        let type_tag = match asset_type {
            AssetType::Equity      => "eq",
            AssetType::Derivatives => "fut",
            AssetType::Spot        => "tok",
            _                      => "syn",
        };

        WorkAsset {
            id: format!("{}_{:04}", type_tag, r),
            label: format!("#{} {} (rank {})", r, type_tag.to_uppercase(), r),
            asset_type: asset_type.clone(),
            mu,
            sigma,
            cluster,
        }
    }).collect()
}

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

/// Lower Cholesky decomposition of the correlation matrix defined by cluster IDs.
pub fn cholesky(clusters: &[usize]) -> Vec<Vec<f64>> {
    let n = clusters.len();
    let mut l: Vec<Vec<f64>> = vec![vec![0.0; n]; n];
    for i in 0..n {
        for j in 0..=i {
            let corr_ij = if i == j { 1.0 } else { cross_corr(clusters[i], clusters[j]) };
            let mut s = corr_ij;
            for k in 0..j { s -= l[i][k] * l[j][k]; }
            l[i][j] = if i == j { s.max(1e-12).sqrt() } else { s / (l[j][j] + 1e-12) };
        }
    }
    l
}

// ─── PRNG + Box-Muller ────────────────────────────────────────────────────────

pub struct Rng(u32);
impl Rng {
    pub fn new(seed: u32) -> Self { Self(seed.max(1)) }
    pub fn next_f64(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0 as f64 / u32::MAX as f64
    }
    pub fn normal(&mut self) -> f64 {
        let u1 = self.next_f64().max(1e-12);
        let u2 = self.next_f64();
        (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
    }
}

/// GBM correlated return matrix: shape [days][n_assets].
///
/// For n ≤ 120 assets uses a full n×n Cholesky (exact).
/// For n > 120 uses a cluster factor model (O(n) per day vs O(n³) Cholesky setup):
///   r_i = sqrt(h²) · (cluster-level factor correlated via 8×8 Cholesky)
///         + sqrt(1−h²) · idiosyncratic noise
/// h² = 0.60 (systematic communality), preserving cross-cluster and within-cluster
/// correlation structure at a tiny fraction of the compute cost.
pub fn generate_returns(assets: &[WorkAsset], days: usize, seed: u32) -> Vec<Vec<f64>> {
    let n = assets.len();
    let dt: f64 = 1.0 / 252.0;
    let sqrt_dt = dt.sqrt();

    if n <= 120 {
        // Full Cholesky — exact for small/medium universes
        let clusters: Vec<usize> = assets.iter().map(|a| a.cluster).collect();
        let l = cholesky(&clusters);
        let mut rng = Rng::new(seed);
        return (0..days).map(|_| {
            let z: Vec<f64> = (0..n).map(|_| rng.normal()).collect();
            let corr_z: Vec<f64> = (0..n).map(|i| {
                (0..=i).map(|j| l[i][j] * z[j]).sum::<f64>()
            }).collect();
            (0..n).map(|i| {
                let mu = assets[i].mu;
                let sigma = assets[i].sigma;
                (mu - 0.5 * sigma * sigma) * dt + sigma * sqrt_dt * corr_z[i]
            }).collect()
        }).collect();
    }

    // Factor model for large universes: 8 cluster factors + idiosyncratic noise
    const H2: f64 = 0.60;  // systematic communality (fraction of variance from cluster factors)
    let h = H2.sqrt();
    let idio = (1.0 - H2).sqrt();

    // Unique clusters present in this universe (max 8)
    let mut unique_clusters: Vec<usize> = assets.iter().map(|a| a.cluster).collect::<std::collections::HashSet<_>>().into_iter().collect();
    unique_clusters.sort_unstable();
    let nc = unique_clusters.len();

    // Map cluster id → index in factor vector
    let cluster_idx: std::collections::HashMap<usize, usize> = unique_clusters.iter().enumerate().map(|(i, &c)| (c, i)).collect();

    // 8×8 (or nc×nc) Cholesky for the factor correlation matrix
    let factor_l = cholesky(&unique_clusters);

    let mut rng = Rng::new(seed);

    (0..days).map(|_| {
        // Draw nc factor innovations and correlate them
        let zf: Vec<f64> = (0..nc).map(|_| rng.normal()).collect();
        let f: Vec<f64> = (0..nc).map(|i| {
            (0..=i).map(|j| factor_l[i][j] * zf[j]).sum::<f64>()
        }).collect();

        // Draw per-asset idiosyncratic innovations
        let ze: Vec<f64> = (0..n).map(|_| rng.normal()).collect();

        (0..n).map(|i| {
            let mu = assets[i].mu;
            let sigma = assets[i].sigma;
            let ci = cluster_idx[&assets[i].cluster];
            let corr_z = h * f[ci] + idio * ze[i];
            (mu - 0.5 * sigma * sigma) * dt + sigma * sqrt_dt * corr_z
        }).collect()
    }).collect()
}

// ─── Strategy implementations ─────────────────────────────────────────────────

pub fn equal_weights(n: usize) -> Vec<f64> { vec![1.0 / n as f64; n] }

pub fn normalise_pos(w: Vec<f64>) -> Vec<f64> {
    let total: f64 = w.iter().sum();
    if total < 1e-10 { return equal_weights(w.len()); }
    w.into_iter().map(|v| v / total).collect()
}

pub fn momentum_weights(returns: &[Vec<f64>], t: usize, lookback: usize) -> Vec<f64> {
    let n = returns[0].len();
    if t < lookback { return equal_weights(n); }
    let mom: Vec<f64> = (0..n).map(|i| {
        returns[(t - lookback)..t].iter().map(|day| day[i]).sum::<f64>()
    }).collect();
    normalise_pos(mom.into_iter().map(|v| v.max(0.0)).collect())
}

pub fn mean_reversion_weights(prices: &[Vec<f64>], t: usize, window: usize) -> Vec<f64> {
    let n = prices[0].len();
    if t < window { return equal_weights(n); }
    let ma: Vec<f64> = (0..n).map(|i| {
        prices[(t - window)..t].iter().map(|p| p[i]).sum::<f64>() / window as f64
    }).collect();
    let dists: Vec<f64> = (0..n).map(|i| (ma[i] - prices[t][i]).max(0.0)).collect();
    normalise_pos(dists)
}

pub fn trend_following_weights(prices: &[Vec<f64>], t: usize, short_w: usize, long_w: usize) -> Vec<f64> {
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

pub fn risk_parity_weights(returns: &[Vec<f64>], t: usize, window: usize) -> Vec<f64> {
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

pub fn kelly_weights(returns: &[Vec<f64>], t: usize, window: usize) -> Vec<f64> {
    let n = returns[0].len();
    if t < window { return equal_weights(n); }
    let slice = &returns[(t - window)..t];
    let means: Vec<f64> = (0..n).map(|i| slice.iter().map(|d| d[i]).sum::<f64>() / window as f64).collect();
    let vars: Vec<f64> = (0..n).map(|i| {
        (slice.iter().map(|d| (d[i] - means[i]).powi(2)).sum::<f64>() / window as f64).max(1e-12)
    }).collect();
    // Kelly fraction f* = µ/σ², capped at 40% per asset (Kelly 1956)
    let kelly: Vec<f64> = (0..n).map(|i| (means[i] / vars[i]).max(0.0).min(0.40)).collect();
    normalise_pos(kelly)
}

pub fn delta_neutral_weights(asset_types: &[AssetType]) -> Vec<f64> {
    let w: Vec<f64> = asset_types.iter().map(|at| {
        match at {
            AssetType::StablecoinYield | AssetType::Lending => 1.0,
            _ => 0.0,
        }
    }).collect();
    normalise_pos(w)
}

pub fn quant_value_weights(returns: &[Vec<f64>], t: usize, window: usize, n: usize) -> Vec<f64> {
    if t < window { return equal_weights(n); }
    let slice = &returns[(t - window)..t];
    let means: Vec<f64> = (0..n).map(|i| slice.iter().map(|d| d[i]).sum::<f64>() / window as f64).collect();
    let vols: Vec<f64> = (0..n).map(|i| {
        let var = slice.iter().map(|d| (d[i] - means[i]).powi(2)).sum::<f64>() / window as f64;
        var.sqrt().max(1e-6)
    }).collect();
    // Rank by risk-adjusted return (Sharpe proxy ≈ P/TVL for DeFi, P/E for equity)
    let mut scores: Vec<(usize, f64)> = (0..n).map(|i| (i, means[i] / vols[i])).collect();
    scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let top_k = (n / 3).max(1);
    let mut w = vec![0.0f64; n];
    for &(idx, _) in &scores[..top_k] { w[idx] = 1.0 / top_k as f64; }
    w
}

pub fn stat_arb_weights(returns: &[Vec<f64>], t: usize, window: usize) -> Vec<f64> {
    let n = returns[0].len();
    if t < window || n < 2 { return equal_weights(n); }
    // Cumulative return over window
    let cum: Vec<f64> = (0..n).map(|i| {
        returns[(t - window)..t].iter().fold(1.0f64, |acc, d| acc * (1.0 + d[i]))
    }).collect();
    let mut sorted = cum.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median = sorted[n / 2];
    // Long recent underperformers (mean-reversion bet); Chan "Quantitative Trading" §4
    let w: Vec<f64> = cum.iter().map(|&r| if r < median { 1.0 } else { 0.0 }).collect();
    normalise_pos(w)
}

pub fn ml_alpha_weights(returns: &[Vec<f64>], prices: &[Vec<f64>], t: usize) -> Vec<f64> {
    let n = returns[0].len();
    let window = 20usize.min(t);
    if window < 5 { return equal_weights(n); }
    // Ensemble: 50% momentum + 50% mean-reversion — stacked signal proxy (Tulchinsky 2019)
    let mw  = momentum_weights(returns, t, window);
    let mrw = mean_reversion_weights(prices, t, window);
    (0..n).map(|i| 0.5 * mw[i] + 0.5 * mrw[i]).collect()
}

pub fn liquidity_provision_weights(asset_types: &[AssetType], returns: &[Vec<f64>], t: usize, window: usize) -> Vec<f64> {
    let n = asset_types.len();
    if t < window { return equal_weights(n); }
    // Optimal LP: maximise fee_APY / realized_vol (net APY ≈ fee_APY × (1 - IL_rate))
    // Chan + Harvey: weight ∝ lp_bonus / vol
    let slice = &returns[(t - window)..t];
    let means: Vec<f64> = (0..n).map(|i| slice.iter().map(|d| d[i]).sum::<f64>() / window as f64).collect();
    let vols: Vec<f64> = (0..n).map(|i| {
        let var = slice.iter().map(|d| (d[i] - means[i]).powi(2)).sum::<f64>() / window as f64;
        var.sqrt().max(1e-6)
    }).collect();
    let w: Vec<f64> = (0..n).map(|i| {
        let lp_bonus = match &asset_types[i] {
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
    // ── Jump-adjusted risk metrics (Spadafora et al. arXiv:1803.07021) ──────
    /// 1-day 95% VaR adjusted for jump-diffusion decomposition.
    pub jump_var_95: f64,
    /// Fraction of total return variance explained by jump events.
    pub jump_variance_fraction: f64,
    /// Poisson jump intensity: expected number of jump events per trading day.
    pub jump_lambda: f64,
}

pub fn compute_metrics(
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
            jump_var_95: 0.0, jump_variance_fraction: 0.0, jump_lambda: 0.0,
        };
    }

    let ann = 252.0_f64;
    let rf_daily = 0.05 / ann;

    let total_return = nav[n - 1] / nav[0] - 1.0;
    let ann_return = (1.0 + total_return).powf(ann / (n - 1) as f64) - 1.0;
    let ann_return = if ann_return.is_finite() { ann_return } else { 0.0 };

    let excess: Vec<f64> = daily_returns.iter().map(|r| r - rf_daily).collect();
    let mean_ex: f64 = excess.iter().sum::<f64>() / excess.len() as f64;
    let var_ex: f64  = excess.iter().map(|r| (r - mean_ex).powi(2)).sum::<f64>() / excess.len() as f64;
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

    // Sample history to ≤120 points regardless of horizon length
    let sample = (n / 120).max(1);
    let portfolio_history: Vec<PortfolioPoint> = nav.iter().enumerate()
        .filter(|(i, _)| i % sample == 0 || *i == n - 1)
        .map(|(i, &v)| {
            let dt = start_date + Duration::days(i as i64);
            PortfolioPoint {
                date: dt.format("%Y-%m-%d").to_string(),
                value: (v * 100.0).round() / 100.0,
            }
        })
        .collect();

    // ── Jump-adjusted VaR (arXiv:1803.07021) ────────────────────────────────
    let jv = crate::jump_var::jump_adjusted_var(
        daily_returns,
        0.95,
        &crate::jump_var::JumpDetectorConfig::default(),
    );

    SimMetrics {
        annualised_return: ann_return, sharpe, sortino, max_drawdown: max_dd, calmar,
        win_rate, volatility: vol, es95, turnover, portfolio_history,
        jump_var_95: jv.jump_var,
        jump_variance_fraction: jv.decomposition.jump_variance_fraction,
        jump_lambda: jv.decomposition.lambda,
    }
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
    /// Trading days (1 trading day = 1/252 of a year in GBM). Max 1260 = 5 trading years.
    pub days: Option<usize>,
    pub initial_capital: Option<f64>,
    pub risk_profile: Option<String>,
    pub seed: Option<u32>,
    /// When provided and asset_types include Equity/Derivatives/Spot, simulates a synthetic
    /// ranked universe of this size using power-law market-cap distribution (Gabaix 2009).
    /// Valid range: 500–3000.
    pub top_n: Option<usize>,
    /// Explicit list of curated asset IDs to include. When provided (and top_n is None),
    /// only these specific instruments are simulated. Ignored when top_n is set.
    pub asset_ids: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct SimResult {
    pub metrics: SimMetrics,
    pub assets: Vec<AssetInfo>,
    /// Total instruments in the simulated universe (may differ from assets.len() when top_n is large)
    pub universe_size: usize,
}

pub fn run_simulation(req: SimRequest) -> Result<SimResult, String> {
    let universe_key = match req.universe {
        UniverseId::Web3Defi   => "web3_defi",
        UniverseId::Web3Crypto => "web3_crypto",
        UniverseId::Hybrid     => "hybrid",
        UniverseId::TradFi     => "trad_fi",
    };

    let type_filter = req.asset_types.clone().unwrap_or_default();

    // ── Build working asset list ──────────────────────────────────────────────
    let selected: Vec<WorkAsset> = if let Some(top_n) = req.top_n {
        // Validate top_n range
        if top_n < 500 || top_n > 3000 {
            return Err(format!("top_n must be between 500 and 3000, got {}", top_n));
        }

        // Determine which eligible types to expand synthetically
        let eligible_types: Vec<AssetType> = if type_filter.is_empty() {
            // Default eligible types for each universe
            match req.universe {
                UniverseId::TradFi | UniverseId::Hybrid =>
                    vec![AssetType::Equity, AssetType::Derivatives],
                UniverseId::Web3Crypto | UniverseId::Web3Defi =>
                    vec![AssetType::Spot, AssetType::Derivatives],
            }
        } else {
            type_filter.iter().filter(|t| is_top_n_eligible(t)).cloned().collect()
        };

        if eligible_types.is_empty() {
            return Err("top_n requires at least one rankable asset type: equity, derivatives, or spot".into());
        }

        // Distribute top_n evenly across eligible types
        let per_type = top_n / eligible_types.len();
        let remainder = top_n % eligible_types.len();

        let mut assets: Vec<WorkAsset> = Vec::with_capacity(top_n);
        for (i, at) in eligible_types.iter().enumerate() {
            let count = per_type + if i < remainder { 1 } else { 0 };
            assets.extend(build_synthetic_universe(at, count));
        }
        assets
    } else {
        let id_filter = req.asset_ids.as_deref().unwrap_or(&[]);
        // Use curated static catalogue filtered by universe + asset types + optional explicit IDs
        ASSETS.iter()
            .filter(|a| a.universes.contains(&universe_key))
            .filter(|a| type_filter.is_empty() || type_filter.contains(&a.asset_type))
            .filter(|a| id_filter.is_empty() || id_filter.contains(&a.id.to_string()))
            .map(|a| WorkAsset {
                id: a.id.to_string(),
                label: a.label.to_string(),
                asset_type: a.asset_type.clone(),
                mu: a.mu,
                sigma: a.sigma,
                cluster: a.cluster,
            })
            .collect()
    };

    if selected.is_empty() {
        return Err("No assets matched the selected universe and asset-type filters".into());
    }

    // Max 1260 trading days = 5 trading years (GBM dt = 1/252)
    let days            = req.days.unwrap_or(252).min(1260);
    let initial_capital = req.initial_capital.unwrap_or(100_000.0);
    let seed            = req.seed.unwrap_or(42);
    let params          = req.params.unwrap_or(StrategyParams { lookback: None, short_window: None, long_window: None });

    // Leverage cap by risk profile
    let leverage_cap: f64 = match req.risk_profile.as_deref().unwrap_or("moderate") {
        "conservative" => 0.50,
        "aggressive"   => 1.00,
        _              => 0.80,
    };

    let lookback = params.lookback.unwrap_or(20);
    let short_w  = params.short_window.unwrap_or(10);
    let long_w   = params.long_window.unwrap_or(50);
    let n        = selected.len();

    // GBM returns: days × n
    let raw_returns = generate_returns(&selected, days, seed);

    // Build price index for strategies that need it
    let mut prices: Vec<Vec<f64>> = vec![vec![100.0f64; n]];
    for day in &raw_returns {
        let prev = prices.last().unwrap();
        prices.push((0..n).map(|i| prev[i] * (1.0 + day[i])).collect());
    }

    let asset_types: Vec<AssetType> = selected.iter().map(|a| a.asset_type.clone()).collect();

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
                StrategyId::DeltaNeutral         => delta_neutral_weights(&asset_types),
                StrategyId::QuantValue           => quant_value_weights(&raw_returns, t, lookback, n),
                StrategyId::StatArb              => stat_arb_weights(&raw_returns, t, lookback),
                StrategyId::MlAlpha              => ml_alpha_weights(&raw_returns, &prices, t),
                StrategyId::LiquidityProvisionOpt => liquidity_provision_weights(&asset_types, &raw_returns, t, lookback),
            };
            // Apply leverage cap
            current_weights = raw_w.into_iter().map(|w| w * leverage_cap).collect();
        }

        all_weights.push(current_weights.clone());

        let port_return: f64 = current_weights.iter()
            .zip(raw_returns[t].iter())
            .map(|(w, r)| w * r)
            .sum();

        // Proportional slippage: 5bps per rebalance on weight moved
        let slippage = if t % rebal_freq == 0 {
            current_weights.iter().map(|w| w * 0.0005).sum::<f64>()
        } else { 0.0 };

        let net = port_return - slippage;
        port_returns.push(net);
        nav.push(nav.last().unwrap() * (1.0 + net));
    }

    let start_date = Utc::now() - Duration::days(days as i64);
    let metrics = compute_metrics(&nav, &port_returns, &all_weights, start_date);

    let universe_size = selected.len();

    // Cap the returned asset list at 50 to keep the response payload bounded
    let assets: Vec<AssetInfo> = selected.into_iter().take(50).map(|a| AssetInfo {
        id: a.id,
        label: a.label,
        asset_type: format!("{:?}", a.asset_type).to_lowercase(),
    }).collect();

    Ok(SimResult { metrics, assets, universe_size })
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Pure math ──────────────────────────────────────────────────────────────

    #[test]
    fn equal_weights_sum_to_one() {
        let w = equal_weights(5);
        assert!((w.iter().sum::<f64>() - 1.0).abs() < 1e-10);
        assert_eq!(w.len(), 5);
    }

    #[test]
    fn normalise_pos_zero_vector_returns_equal_weights() {
        let w = normalise_pos(vec![0.0, 0.0, 0.0]);
        assert!((w.iter().sum::<f64>() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn cholesky_diagonal_positive() {
        let clusters = vec![0, 1, 2, 3];
        let l = cholesky(&clusters);
        for i in 0..clusters.len() {
            assert!(l[i][i] > 0.0, "diagonal must be positive");
        }
    }

    #[test]
    fn cholesky_lower_triangular() {
        let clusters = vec![0, 1, 2];
        let l = cholesky(&clusters);
        for i in 0..3 {
            for j in (i + 1)..3 {
                assert_eq!(l[i][j], 0.0, "upper triangle must be zero");
            }
        }
    }

    #[test]
    fn rng_output_in_unit_interval() {
        let mut rng = Rng::new(42);
        for _ in 0..1000 {
            let v = rng.next_f64();
            assert!(v >= 0.0 && v <= 1.0);
        }
    }

    #[test]
    fn generate_returns_shape_correct() {
        let assets = build_synthetic_universe(&AssetType::Equity, 10);
        let ret = generate_returns(&assets, 50, 42);
        assert_eq!(ret.len(), 50);
        assert_eq!(ret[0].len(), 10);
    }

    #[test]
    fn generate_returns_finite() {
        let assets = build_synthetic_universe(&AssetType::Spot, 5);
        let ret = generate_returns(&assets, 100, 1);
        for day in &ret {
            for &r in day {
                assert!(r.is_finite(), "all GBM returns must be finite");
            }
        }
    }

    // ── Synthetic universe ─────────────────────────────────────────────────────

    #[test]
    fn synthetic_universe_correct_count() {
        let assets = build_synthetic_universe(&AssetType::Equity, 500);
        assert_eq!(assets.len(), 500);
    }

    #[test]
    fn synthetic_universe_power_law_vol_monotone() {
        let assets = build_synthetic_universe(&AssetType::Equity, 100);
        // Smaller ranks (higher number) should have higher volatility
        assert!(assets.last().unwrap().sigma > assets.first().unwrap().sigma,
            "rank 100 should be more volatile than rank 1");
    }

    #[test]
    fn synthetic_universe_size_premium_monotone() {
        let assets = build_synthetic_universe(&AssetType::Equity, 100);
        // Smaller ranks should have higher expected return (size premium)
        assert!(assets.last().unwrap().mu > assets.first().unwrap().mu,
            "rank 100 should have higher mu than rank 1 (size premium)");
    }

    #[test]
    fn synthetic_universe_all_params_positive() {
        for at in &[AssetType::Equity, AssetType::Derivatives, AssetType::Spot] {
            let assets = build_synthetic_universe(at, 100);
            for a in &assets {
                assert!(a.mu > 0.0, "mu must be positive");
                assert!(a.sigma > 0.0, "sigma must be positive");
            }
        }
    }

    // ── Strategies ────────────────────────────────────────────────────────────

    #[test]
    fn momentum_weights_warm_up_returns_equal() {
        let returns = vec![vec![0.01, -0.01]; 5];
        let w = momentum_weights(&returns, 3, 10); // t < lookback
        assert!((w[0] - w[1]).abs() < 1e-10, "warm-up should give equal weights");
    }

    #[test]
    fn risk_parity_weights_inverse_vol() {
        // Asset 0 has high vol, asset 1 has low vol → asset 1 gets more weight
        let mut returns: Vec<Vec<f64>> = vec![];
        let mut rng = Rng::new(7);
        for _ in 0..30 {
            returns.push(vec![rng.normal() * 0.05, rng.normal() * 0.01]);
        }
        let w = risk_parity_weights(&returns, 30, 30);
        assert!(w[1] > w[0], "low-vol asset should get higher risk parity weight");
    }

    #[test]
    fn delta_neutral_weights_only_stable_assets() {
        let types = vec![AssetType::Spot, AssetType::StablecoinYield, AssetType::Lending, AssetType::Equity];
        let w = delta_neutral_weights(&types);
        assert_eq!(w[0], 0.0, "spot should be zero in delta-neutral");
        assert_eq!(w[3], 0.0, "equity should be zero in delta-neutral");
        assert!(w[1] > 0.0, "stablecoin yield should have positive weight");
        assert!(w[2] > 0.0, "lending should have positive weight");
    }

    #[test]
    fn kelly_weights_capped_at_40pct_per_asset() {
        let assets = build_synthetic_universe(&AssetType::Equity, 5);
        let ret = generate_returns(&assets, 30, 42);
        let w = kelly_weights(&ret, 30, 30);
        for &wi in &w {
            assert!(wi <= 0.40 + 1e-10, "kelly weight must not exceed 40% cap");
        }
    }

    #[test]
    fn all_strategy_weights_sum_to_at_most_one() {
        let assets = build_synthetic_universe(&AssetType::Equity, 10);
        let ret = generate_returns(&assets, 100, 42);
        let mut prices = vec![vec![100.0f64; 10]];
        for day in &ret {
            let prev = prices.last().unwrap().clone();
            prices.push((0..10).map(|i| prev[i] * (1.0 + day[i])).collect());
        }
        let types: Vec<AssetType> = assets.iter().map(|a| a.asset_type.clone()).collect();

        let weights_list = vec![
            equal_weights(10),
            momentum_weights(&ret, 50, 20),
            mean_reversion_weights(&prices, 50, 20),
            trend_following_weights(&prices, 50, 10, 30),
            risk_parity_weights(&ret, 50, 20),
            kelly_weights(&ret, 50, 20),
            quant_value_weights(&ret, 50, 20, 10),
            stat_arb_weights(&ret, 50, 20),
            ml_alpha_weights(&ret, &prices, 50),
            delta_neutral_weights(&types),
            liquidity_provision_weights(&types, &ret, 50, 20),
        ];

        for w in weights_list {
            let s: f64 = w.iter().sum();
            assert!(s <= 1.0 + 1e-9, "weights must sum to ≤ 1, got {}", s);
            assert!(s >= 0.0, "weights must be non-negative sum");
        }
    }

    // ── Metrics ───────────────────────────────────────────────────────────────

    #[test]
    fn compute_metrics_empty_nav_returns_zeros() {
        let m = compute_metrics(&[100.0], &[], &[], Utc::now());
        assert_eq!(m.sharpe, 0.0);
        assert_eq!(m.max_drawdown, 0.0);
    }

    #[test]
    fn compute_metrics_max_drawdown_bounded() {
        let nav: Vec<f64> = (0..252).map(|i| 100.0 * (1.0 + 0.001 * i as f64)).collect();
        let rets: Vec<f64> = nav.windows(2).map(|w| w[1] / w[0] - 1.0).collect();
        let weights: Vec<Vec<f64>> = vec![vec![1.0]; rets.len()];
        let m = compute_metrics(&nav, &rets, &weights, Utc::now());
        assert!(m.max_drawdown >= 0.0 && m.max_drawdown <= 1.0);
    }

    #[test]
    fn compute_metrics_sharpe_finite() {
        let assets = build_synthetic_universe(&AssetType::Equity, 5);
        let ret = generate_returns(&assets, 252, 42);
        let nav: Vec<f64> = ret.iter().scan(100_000.0, |acc, day| {
            let r: f64 = day.iter().sum::<f64>() / day.len() as f64;
            *acc *= 1.0 + r;
            Some(*acc)
        }).collect();
        let daily: Vec<f64> = nav.windows(2).map(|w| w[1] / w[0] - 1.0).collect();
        let weights: Vec<Vec<f64>> = vec![equal_weights(5); daily.len()];
        let m = compute_metrics(&nav, &daily, &weights, Utc::now());
        assert!(m.sharpe.is_finite());
        assert!(m.sortino.is_finite());
        assert!(m.calmar.is_finite());
    }

    // ── SimRequest validation ─────────────────────────────────────────────────

    #[test]
    fn run_simulation_basic_trad_fi() {
        let req = SimRequest {
            universe: UniverseId::TradFi,
            asset_types: None,
            strategy: StrategyId::RiskParity,
            params: None,
            days: Some(252),
            initial_capital: Some(100_000.0),
            risk_profile: Some("moderate".to_string()),
            seed: Some(1),
            top_n: None,
        };
        let result = run_simulation(req).unwrap();
        assert!(result.metrics.sharpe.is_finite());
        assert!(result.metrics.max_drawdown >= 0.0);
        assert!(!result.metrics.portfolio_history.is_empty());
    }

    fn portfolio_history<'a>(r: &'a SimResult) -> &'a Vec<PortfolioPoint> {
        &r.metrics.portfolio_history
    }

    #[test]
    fn run_simulation_top_n_equity_500() {
        let req = SimRequest {
            universe: UniverseId::TradFi,
            asset_types: Some(vec![AssetType::Equity]),
            strategy: StrategyId::Momentum,
            params: Some(StrategyParams { lookback: Some(20), short_window: None, long_window: None }),
            days: Some(252),
            initial_capital: Some(100_000.0),
            risk_profile: Some("aggressive".to_string()),
            seed: Some(99),
            top_n: Some(500),
        };
        let result = run_simulation(req).unwrap();
        assert_eq!(result.universe_size, 500);
        assert!(result.assets.len() <= 50, "returned asset list capped at 50");
        assert!(result.metrics.sharpe.is_finite());
    }

    #[test]
    fn run_simulation_top_n_derivatives_1000() {
        let req = SimRequest {
            universe: UniverseId::TradFi,
            asset_types: Some(vec![AssetType::Derivatives]),
            strategy: StrategyId::TrendFollowing,
            params: None,
            days: Some(504),
            initial_capital: None,
            risk_profile: None,
            seed: Some(7),
            top_n: Some(1000),
        };
        let result = run_simulation(req).unwrap();
        assert_eq!(result.universe_size, 1000);
        assert!(result.metrics.annualised_return.is_finite());
    }

    #[test]
    fn run_simulation_five_trading_years() {
        let req = SimRequest {
            universe: UniverseId::Hybrid,
            asset_types: None,
            strategy: StrategyId::EqualWeight,
            params: None,
            days: Some(1260), // 5 trading years
            initial_capital: Some(1_000_000.0),
            risk_profile: Some("conservative".to_string()),
            seed: Some(42),
            top_n: None,
        };
        let result = run_simulation(req).unwrap();
        // History must span ~5 years of trading days
        assert!(!portfolio_history(&result).is_empty());
        assert!(result.metrics.max_drawdown >= 0.0 && result.metrics.max_drawdown <= 1.0);
    }

    #[test]
    fn run_simulation_days_capped_at_1260() {
        let req = SimRequest {
            universe: UniverseId::TradFi,
            asset_types: None,
            strategy: StrategyId::EqualWeight,
            params: None,
            days: Some(9999), // should be clamped to 1260
            initial_capital: None,
            risk_profile: None,
            seed: Some(1),
            top_n: None,
        };
        let result = run_simulation(req).unwrap();
        // portfolio_history is sampled to ≤120 points, so it must be non-empty
        assert!(!portfolio_history(&result).is_empty());
    }

    #[test]
    fn run_simulation_top_n_too_small_returns_error() {
        let req = SimRequest {
            universe: UniverseId::TradFi,
            asset_types: Some(vec![AssetType::Equity]),
            strategy: StrategyId::EqualWeight,
            params: None,
            days: Some(252),
            initial_capital: None,
            risk_profile: None,
            seed: None,
            top_n: Some(100), // below minimum of 500
        };
        assert!(run_simulation(req).is_err());
    }

    #[test]
    fn run_simulation_top_n_too_large_returns_error() {
        let req = SimRequest {
            universe: UniverseId::TradFi,
            asset_types: Some(vec![AssetType::Equity]),
            strategy: StrategyId::EqualWeight,
            params: None,
            days: Some(252),
            initial_capital: None,
            risk_profile: None,
            seed: None,
            top_n: Some(5000), // above maximum of 3000
        };
        assert!(run_simulation(req).is_err());
    }

    #[test]
    fn run_simulation_ineligible_type_with_top_n_returns_error() {
        let req = SimRequest {
            universe: UniverseId::Web3Defi,
            asset_types: Some(vec![AssetType::Lending]), // not rankable
            strategy: StrategyId::EqualWeight,
            params: None,
            days: Some(252),
            initial_capital: None,
            risk_profile: None,
            seed: None,
            top_n: Some(500),
        };
        assert!(run_simulation(req).is_err());
    }

    #[test]
    fn run_simulation_all_strategies_produce_finite_metrics() {
        use StrategyId::*;
        let strategies = [
            EqualWeight, Momentum, MeanReversion, TrendFollowing, RiskParity,
            Kelly, DeltaNeutral, QuantValue, StatArb, MlAlpha, Canslim, LiquidityProvisionOpt,
        ];
        for strategy in strategies {
            let req = SimRequest {
                universe: UniverseId::Hybrid,
                asset_types: None,
                strategy,
                params: Some(StrategyParams { lookback: Some(20), short_window: Some(10), long_window: Some(40) }),
                days: Some(126),
                initial_capital: Some(50_000.0),
                risk_profile: Some("moderate".to_string()),
                seed: Some(42),
                top_n: None,
            };
            let result = run_simulation(req).unwrap();
            assert!(result.metrics.sharpe.is_finite(),   "sharpe finite");
            assert!(result.metrics.sortino.is_finite(),  "sortino finite");
            assert!(result.metrics.volatility.is_finite(),"vol finite");
            assert!(result.metrics.max_drawdown >= 0.0,  "max_drawdown non-negative");
            assert!(result.metrics.win_rate >= 0.0 && result.metrics.win_rate <= 1.0, "win_rate in [0,1]");
        }
    }

    #[test]
    fn run_simulation_top_n_multi_type_split_correctly() {
        // two eligible types with top_n=1000 → 500 each
        let req = SimRequest {
            universe: UniverseId::TradFi,
            asset_types: Some(vec![AssetType::Equity, AssetType::Derivatives]),
            strategy: StrategyId::RiskParity,
            params: None,
            days: Some(126),
            initial_capital: None,
            risk_profile: None,
            seed: Some(5),
            top_n: Some(1000),
        };
        let result = run_simulation(req).unwrap();
        assert_eq!(result.universe_size, 1000);
    }
}
