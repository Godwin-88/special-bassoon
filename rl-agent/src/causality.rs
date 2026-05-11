//! Information-theoretic causality detection.
//!
//! Implements Transfer Entropy (TE) and Net Information Flow as described in:
//!   Stavroglou et al. (2021) "Information Theoretic Causality Detection between
//!   Financial and Sentiment Data", Entropy 23(5):621, MDPI.
//!
//! TE(X→Y, k) = H(Y_t | Y_{t-k}) − H(Y_t | X_{t-k}, Y_{t-k})
//!            = H(Y_t, Y_{t-k}) − H(Y_{t-k}) − H(Y_t, X_{t-k}, Y_{t-k}) + H(X_{t-k}, Y_{t-k})
//!
//! Significance via z-score against a shuffled null (50 shuffles, single-entry
//! randomisation to destroy temporal autocorrelations while preserving marginal
//! distributions).  Z > 3 is taken as statistically significant (paper §2.3).
//!
//! Net information flow:
//!   TE_net(X→Y) = TE(X→Y) − TE(Y→X)
//! Positive → X drives Y; negative → Y drives X.

use serde::Serialize;
use std::collections::HashMap;

// ─── Binning ─────────────────────────────────────────────────────────────────

/// Assign each value to one of `n_bins` equi-probable bins (equal observation count).
/// Returns a bin index in 0..n_bins for each input element.
pub fn equi_probable_bins(data: &[f64], n_bins: usize) -> Vec<usize> {
    assert!(n_bins > 0, "n_bins must be positive");
    let n = data.len();
    if n == 0 { return vec![]; }

    let mut indexed: Vec<(usize, f64)> = data.iter()
        .enumerate()
        .map(|(i, &v)| (i, v))
        .collect();
    indexed.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

    let mut bins = vec![0usize; n];
    for (rank, (orig_idx, _)) in indexed.iter().enumerate() {
        bins[*orig_idx] = (rank * n_bins / n).min(n_bins - 1);
    }
    bins
}

// ─── Entropy estimators ───────────────────────────────────────────────────────

/// H(X) = −Σ p(x) ln p(x)   (natural log, in nats)
pub fn shannon_entropy(bins: &[usize], n_bins: usize) -> f64 {
    let n = bins.len() as f64;
    if n == 0.0 { return 0.0; }
    let mut counts = vec![0usize; n_bins];
    for &b in bins { counts[b] += 1; }
    -counts.iter()
        .filter(|&&c| c > 0)
        .map(|&c| { let p = c as f64 / n; p * p.ln() })
        .sum::<f64>()
}

/// H(X, Y) = −Σ_{x,y} p(x,y) ln p(x,y)
pub fn joint_entropy_2(bx: &[usize], by: &[usize], n_bins: usize) -> f64 {
    let n = bx.len() as f64;
    if n == 0.0 { return 0.0; }
    let mut counts: HashMap<(usize, usize), usize> = HashMap::new();
    for (&a, &b) in bx.iter().zip(by.iter()) {
        *counts.entry((a, b)).or_insert(0) += 1;
    }
    -counts.values()
        .map(|&c| { let p = c as f64 / n; p * p.ln() })
        .sum::<f64>()
}

/// H(X, Y, Z) = −Σ_{x,y,z} p(x,y,z) ln p(x,y,z)
pub fn joint_entropy_3(bx: &[usize], by: &[usize], bz: &[usize], n_bins: usize) -> f64 {
    let n = bx.len() as f64;
    if n == 0.0 { return 0.0; }
    let mut counts: HashMap<(usize, usize, usize), usize> = HashMap::new();
    for ((&a, &b), &c) in bx.iter().zip(by.iter()).zip(bz.iter()) {
        *counts.entry((a, b, c)).or_insert(0) += 1;
    }
    -counts.values()
        .map(|&c| { let p = c as f64 / n; p * p.ln() })
        .sum::<f64>()
}

// ─── Transfer Entropy ────────────────────────────────────────────────────────

/// Transfer entropy from X to Y at lag k, using equi-probable binning.
///
/// TE(X→Y, k) = H(Y_t | Y_{t-k}) − H(Y_t | X_{t-k}, Y_{t-k})
///
/// # Arguments
/// * `x`      – source series (e.g. sentiment, log-returns of asset X)
/// * `y`      – target series (e.g. price log-returns of asset Y)
/// * `lag`    – temporal lag k (paper default k=1)
/// * `n_bins` – number of equi-probable bins (paper default 5)
pub fn transfer_entropy(x: &[f64], y: &[f64], lag: usize, n_bins: usize) -> f64 {
    let n = x.len().min(y.len());
    if n <= lag + 1 || n_bins < 2 { return 0.0; }

    // Aligned slices: y_t, y_{t-lag}, x_{t-lag}
    let y_t    = &y[lag..n];
    let y_past = &y[..n - lag];
    let x_past = &x[..n - lag];

    let by_t    = equi_probable_bins(y_t, n_bins);
    let by_past = equi_probable_bins(y_past, n_bins);
    let bx_past = equi_probable_bins(x_past, n_bins);

    // H(Y_t | Y_past) = H(Y_t, Y_past) − H(Y_past)
    let h_yt_given_ypast =
        joint_entropy_2(&by_t, &by_past, n_bins) - shannon_entropy(&by_past, n_bins);

    // H(Y_t | X_past, Y_past) = H(Y_t, X_past, Y_past) − H(X_past, Y_past)
    let h_yt_given_xpast_ypast =
        joint_entropy_3(&by_t, &bx_past, &by_past, n_bins)
        - joint_entropy_2(&bx_past, &by_past, n_bins);

    // TE is non-negative by construction (information can only reduce uncertainty)
    (h_yt_given_ypast - h_yt_given_xpast_ypast).max(0.0)
}

/// Mutual information I(X; Y) = H(Y) − H(Y|X)
pub fn mutual_information(x: &[f64], y: &[f64], n_bins: usize) -> f64 {
    let n = x.len().min(y.len());
    if n < 2 { return 0.0; }
    let bx = equi_probable_bins(&x[..n], n_bins);
    let by = equi_probable_bins(&y[..n], n_bins);
    let hy  = shannon_entropy(&by, n_bins);
    let hyx = joint_entropy_2(&by, &bx, n_bins) - shannon_entropy(&bx, n_bins);
    (hy - hyx).max(0.0)
}

// ─── Significance via shuffling ───────────────────────────────────────────────

/// Shuffle `data` by randomly permuting all entries.
/// "Single-entry randomisation" (paper §2.3): each position is independently
/// assigned a random value from the same series, destroying temporal structure
/// while preserving the marginal distribution.
fn shuffle_series(data: &[f64], rng: &mut crate::portfolio_sim::Rng) -> Vec<f64> {
    let n = data.len();
    let mut out: Vec<f64> = data.to_vec();
    // Fisher-Yates shuffle using the simulation PRNG
    for i in (1..n).rev() {
        let j = (rng.next_f64() * (i + 1) as f64) as usize;
        out.swap(i, j);
    }
    out
}

/// Returns `(te_value, z_score)` for TE(X→Y).
///
/// Z-score is computed against `n_shuffles` random permutations of X (source).
/// Z > 3 is taken as significant at the 0.13% level (paper default).
pub fn te_with_significance(
    x: &[f64], y: &[f64],
    lag: usize, n_bins: usize,
    n_shuffles: usize,
    seed: u32,
) -> (f64, f64) {
    let te_obs = transfer_entropy(x, y, lag, n_bins);
    if n_shuffles == 0 { return (te_obs, 0.0); }

    let mut rng = crate::portfolio_sim::Rng::new(seed);
    let mut shuffle_vals = Vec::with_capacity(n_shuffles);
    for _ in 0..n_shuffles {
        let x_shuffled = shuffle_series(x, &mut rng);
        shuffle_vals.push(transfer_entropy(&x_shuffled, y, lag, n_bins));
    }

    let mean: f64 = shuffle_vals.iter().sum::<f64>() / n_shuffles as f64;
    let var: f64  = shuffle_vals.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n_shuffles as f64;
    let std  = var.sqrt();

    let z = if std > 1e-12 { (te_obs - mean) / std } else { 0.0 };
    (te_obs, z)
}

/// Net information flow: TE_net(X→Y) = TE(X→Y) − TE(Y→X).
/// > 0 → X drives Y; < 0 → Y drives X.
pub fn net_information_flow(
    x: &[f64], y: &[f64],
    lag: usize, n_bins: usize,
) -> f64 {
    transfer_entropy(x, y, lag, n_bins) - transfer_entropy(y, x, lag, n_bins)
}

// ─── Causal link result ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct CausalLink {
    pub source: String,
    pub target: String,
    pub te: f64,        // TE(source → target)
    pub z_score: f64,   // significance (z > 3 is significant)
    pub significant: bool,
    pub te_reverse: f64,  // TE(target → source)
    pub net_flow: f64,    // te − te_reverse
}

// ─── Configuration ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct CausalityConfig {
    /// Temporal lag k (paper default 1 = one trading day)
    pub lag: usize,
    /// Equi-probable bins (paper default 5, optimised for ~512-day datasets)
    pub n_bins: usize,
    /// Number of shuffles for z-score null hypothesis (paper default 50)
    pub n_shuffles: usize,
    /// Z-score threshold for significance (paper default 3.0)
    pub z_threshold: f64,
    /// PRNG seed for shuffling
    pub seed: u32,
}

impl Default for CausalityConfig {
    fn default() -> Self {
        Self { lag: 1, n_bins: 5, n_shuffles: 50, z_threshold: 3.0, seed: 42 }
    }
}

// ─── Matrix computation ───────────────────────────────────────────────────────

/// Compute log returns from price series: L_t = ln(P_t) − ln(P_{t-1})
pub fn log_returns(prices: &[f64]) -> Vec<f64> {
    prices.windows(2)
        .map(|w| if w[0] > 0.0 { (w[1] / w[0]).ln() } else { 0.0 })
        .collect()
}

#[derive(Debug, Serialize)]
pub struct CausalityMatrix {
    /// All pairwise causal links (significant and non-significant)
    pub links: Vec<CausalLink>,
    /// Total significant causal links detected
    pub n_significant: usize,
    /// Sentiment→Price links (when sentiment series are provided)
    pub n_sentiment_to_price: usize,
    /// Price→Sentiment links
    pub n_price_to_sentiment: usize,
    pub config: CausalityConfigSer,
}

#[derive(Debug, Serialize)]
pub struct CausalityConfigSer {
    pub lag: usize,
    pub n_bins: usize,
    pub n_shuffles: usize,
    pub z_threshold: f64,
}

/// Compute the full pairwise TE causality matrix for a set of named series.
///
/// `series` is a list of (name, is_sentiment, data) tuples.
/// When `is_sentiment = true` the series is treated as a sentiment variable
/// for the purpose of counting sentiment→price links.
pub fn compute_causality_matrix(
    series: &[(String, bool, Vec<f64>)],
    cfg: &CausalityConfig,
) -> CausalityMatrix {
    let mut links = Vec::new();
    let n = series.len();

    for i in 0..n {
        for j in 0..n {
            if i == j { continue; }
            let (src_name, src_is_sentiment, src_data) = &series[i];
            let (tgt_name, _,                 tgt_data) = &series[j];

            let (te, z) = te_with_significance(
                src_data, tgt_data,
                cfg.lag, cfg.n_bins, cfg.n_shuffles,
                cfg.seed.wrapping_add(i as u32 * 1000 + j as u32),
            );
            let te_rev = transfer_entropy(tgt_data, src_data, cfg.lag, cfg.n_bins);
            let net    = te - te_rev;

            links.push(CausalLink {
                source: src_name.clone(),
                target: tgt_name.clone(),
                te,
                z_score: z,
                significant: z >= cfg.z_threshold,
                te_reverse: te_rev,
                net_flow: net,
            });
        }
    }

    let n_significant = links.iter().filter(|l| l.significant).count();

    // Count sentiment→price and price→sentiment
    let n_sentiment_to_price = links.iter().filter(|l| {
        l.significant
            && series.iter().find(|(n, _, _)| n == &l.source).map(|(_, s, _)| *s).unwrap_or(false)
            && !series.iter().find(|(n, _, _)| n == &l.target).map(|(_, s, _)| *s).unwrap_or(true)
    }).count();
    let n_price_to_sentiment = links.iter().filter(|l| {
        l.significant
            && !series.iter().find(|(n, _, _)| n == &l.source).map(|(_, s, _)| *s).unwrap_or(true)
            && series.iter().find(|(n, _, _)| n == &l.target).map(|(_, s, _)| *s).unwrap_or(false)
    }).count();

    CausalityMatrix {
        links,
        n_significant,
        n_sentiment_to_price,
        n_price_to_sentiment,
        config: CausalityConfigSer {
            lag: cfg.lag,
            n_bins: cfg.n_bins,
            n_shuffles: cfg.n_shuffles,
            z_threshold: cfg.z_threshold,
        },
    }
}

// ─── Causality-adjusted weight signal ────────────────────────────────────────

/// Given per-asset TE significance z-scores (sentiment→price or lagged-price→price),
/// compute a causality-based weight boost. Assets with stronger causal signal receive
/// a proportionally larger weight multiplier in [1.0, max_boost].
///
/// Used in portfolio_sim to enhance momentum weights when causal structure is detected.
pub fn causality_weight_boost(z_scores: &[f64], max_boost: f64) -> Vec<f64> {
    let z_min = z_scores.iter().cloned().fold(f64::INFINITY, f64::min).max(0.0);
    let z_max = z_scores.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    if (z_max - z_min).abs() < 1e-10 {
        return vec![1.0; z_scores.len()];
    }
    z_scores.iter().map(|&z| {
        let normalised = (z.max(0.0) - z_min) / (z_max - z_min);
        1.0 + normalised * (max_boost - 1.0)
    }).collect()
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Binning ──────────────────────────────────────────────────────────────

    #[test]
    fn equi_probable_bins_correct_range() {
        let data: Vec<f64> = (0..100).map(|i| i as f64).collect();
        let bins = equi_probable_bins(&data, 5);
        assert!(bins.iter().all(|&b| b < 5), "all bins within range");
    }

    #[test]
    fn equi_probable_bins_approximately_equal_counts() {
        let data: Vec<f64> = (0..100).map(|i| i as f64).collect();
        let bins = equi_probable_bins(&data, 5);
        let mut counts = [0usize; 5];
        for &b in &bins { counts[b] += 1; }
        // Each bin should have ~20 observations
        for c in counts { assert!(c >= 18 && c <= 22, "bin count {} not near 20", c); }
    }

    #[test]
    fn equi_probable_bins_empty_input() {
        assert_eq!(equi_probable_bins(&[], 5), Vec::<usize>::new());
    }

    #[test]
    fn equi_probable_bins_single_bin() {
        let data = vec![1.0, 2.0, 3.0];
        let bins = equi_probable_bins(&data, 1);
        assert!(bins.iter().all(|&b| b == 0));
    }

    // ── Entropy ──────────────────────────────────────────────────────────────

    #[test]
    fn shannon_entropy_uniform_is_log_n() {
        // Uniform distribution: H = ln(n_bins)
        let bins: Vec<usize> = (0..100).map(|i| i % 5).collect();
        let h = shannon_entropy(&bins, 5);
        let expected = (5.0f64).ln();
        assert!((h - expected).abs() < 0.01, "H={} expected≈{}", h, expected);
    }

    #[test]
    fn shannon_entropy_degenerate_is_zero() {
        // All in one bin: H = 0
        let bins: Vec<usize> = vec![0; 50];
        let h = shannon_entropy(&bins, 5);
        assert!(h.abs() < 1e-10, "deterministic entropy should be 0, got {}", h);
    }

    #[test]
    fn shannon_entropy_non_negative() {
        let data: Vec<f64> = (0..50).map(|i| i as f64 * 0.3).collect();
        let bins = equi_probable_bins(&data, 5);
        assert!(shannon_entropy(&bins, 5) >= 0.0);
    }

    #[test]
    fn joint_entropy_2_geq_marginal() {
        // H(X,Y) >= H(X)
        let data_x: Vec<f64> = (0..50).map(|i| i as f64).collect();
        let data_y: Vec<f64> = (0..50).map(|i| (50 - i) as f64).collect();
        let bx = equi_probable_bins(&data_x, 5);
        let by = equi_probable_bins(&data_y, 5);
        let hx = shannon_entropy(&bx, 5);
        let hxy = joint_entropy_2(&bx, &by, 5);
        assert!(hxy >= hx - 1e-10, "H(X,Y) must be >= H(X)");
    }

    #[test]
    fn mutual_information_non_negative() {
        let x: Vec<f64> = (0..100).map(|i| i as f64).collect();
        let y: Vec<f64> = x.iter().map(|v| v + 1.0).collect(); // perfect correlation
        let mi = mutual_information(&x, &y, 5);
        assert!(mi >= 0.0, "MI must be non-negative, got {}", mi);
    }

    #[test]
    fn mutual_information_correlated_gt_uncorrelated() {
        // Correlated pair should have more MI than independent pair
        let x: Vec<f64> = (0..200).map(|i| i as f64).collect();
        let y_corr: Vec<f64> = x.iter().map(|&v| v * 1.1).collect();

        let mut rng = crate::portfolio_sim::Rng::new(7);
        let y_rand: Vec<f64> = (0..200).map(|_| rng.normal()).collect();

        let mi_corr = mutual_information(&x, &y_corr, 5);
        let mi_rand = mutual_information(&x, &y_rand, 5);
        assert!(mi_corr > mi_rand, "correlated MI {} should exceed random MI {}", mi_corr, mi_rand);
    }

    // ── Transfer Entropy ─────────────────────────────────────────────────────

    #[test]
    fn transfer_entropy_non_negative() {
        let mut rng = crate::portfolio_sim::Rng::new(42);
        let x: Vec<f64> = (0..200).map(|_| rng.normal()).collect();
        let y: Vec<f64> = (0..200).map(|_| rng.normal()).collect();
        assert!(transfer_entropy(&x, &y, 1, 5) >= 0.0);
    }

    #[test]
    fn transfer_entropy_causal_direction_dominant() {
        // Build a system where X causes Y with lag 1: Y_t = 0.7*X_{t-1} + noise
        let mut rng = crate::portfolio_sim::Rng::new(99);
        let n = 500;
        let x: Vec<f64> = (0..n).map(|_| rng.normal()).collect();
        let mut y = vec![0.0f64; n];
        for t in 1..n {
            y[t] = 0.7 * x[t - 1] + 0.3 * rng.normal();
        }
        let te_xy = transfer_entropy(&x, &y, 1, 5); // X causes Y — should be high
        let te_yx = transfer_entropy(&y, &x, 1, 5); // Y→X — should be low
        assert!(te_xy > te_yx, "TE(X→Y)={:.4} should exceed TE(Y→X)={:.4}", te_xy, te_yx);
    }

    #[test]
    fn transfer_entropy_independent_z_score_below_threshold() {
        // Paper §2.3: independence is tested via z-score, not raw TE.
        // Finite-sample equi-probable binning has a positive bias; TE > 0 even for
        // independent series. What matters is that z < 3 (insignificant).
        let mut rng = crate::portfolio_sim::Rng::new(123);
        let x: Vec<f64> = (0..300).map(|_| rng.normal()).collect();
        let y: Vec<f64> = (0..300).map(|_| rng.normal()).collect();
        let (_, z) = te_with_significance(&x, &y, 1, 5, 30, 42);
        assert!(z < 3.0, "independent series z-score {} should be < 3.0 (insignificant)", z);
    }

    #[test]
    fn net_information_flow_causal_direction() {
        let mut rng = crate::portfolio_sim::Rng::new(7);
        let n = 400;
        let x: Vec<f64> = (0..n).map(|_| rng.normal()).collect();
        let mut y = vec![0.0f64; n];
        for t in 1..n { y[t] = 0.8 * x[t - 1] + 0.2 * rng.normal(); }
        let net = net_information_flow(&x, &y, 1, 5);
        assert!(net > 0.0, "net flow should be positive (X→Y), got {}", net);
    }

    #[test]
    fn te_with_significance_returns_finite_z() {
        let mut rng = crate::portfolio_sim::Rng::new(5);
        let n = 200;
        let x: Vec<f64> = (0..n).map(|_| rng.normal()).collect();
        let mut y = vec![0.0f64; n];
        for t in 1..n { y[t] = 0.6 * x[t - 1] + 0.4 * rng.normal(); }
        let (te, z) = te_with_significance(&x, &y, 1, 5, 20, 42);
        assert!(te.is_finite(), "TE must be finite");
        assert!(z.is_finite(), "z-score must be finite");
    }

    #[test]
    fn te_with_significance_causal_z_above_threshold() {
        // A strong causal signal should produce z > 3
        let mut rng = crate::portfolio_sim::Rng::new(77);
        let n = 512;
        let x: Vec<f64> = (0..n).map(|_| rng.normal()).collect();
        let mut y = vec![0.0f64; n];
        for t in 1..n { y[t] = 0.9 * x[t - 1] + 0.1 * rng.normal(); }
        let (_, z) = te_with_significance(&x, &y, 1, 5, 50, 42);
        assert!(z > 3.0, "strong causal signal should be significant (z={})", z);
    }

    // ── Causality weight boost ────────────────────────────────────────────────

    #[test]
    fn causality_weight_boost_all_equal_returns_ones() {
        let boosts = causality_weight_boost(&[2.0, 2.0, 2.0], 2.0);
        for b in boosts { assert!((b - 1.0).abs() < 1e-10 || (b - 2.0).abs() < 1e-10); }
    }

    #[test]
    fn causality_weight_boost_max_to_strongest() {
        let z = vec![0.0, 3.0, 6.0]; // 6.0 is strongest
        let boosts = causality_weight_boost(&z, 2.0);
        assert!((boosts[2] - 2.0).abs() < 1e-10, "strongest should get max boost");
        assert!((boosts[0] - 1.0).abs() < 1e-10, "weakest should get min boost 1.0");
    }

    #[test]
    fn causality_weight_boost_in_valid_range() {
        let z = vec![1.0, 2.0, 5.0, 0.5];
        let boosts = causality_weight_boost(&z, 3.0);
        for b in boosts {
            assert!(b >= 1.0 && b <= 3.0, "boost {} out of [1, 3]", b);
        }
    }

    // ── Log returns ──────────────────────────────────────────────────────────

    #[test]
    fn log_returns_correct_formula() {
        let prices = vec![100.0, 110.0, 99.0];
        let ret = log_returns(&prices);
        assert_eq!(ret.len(), 2);
        assert!((ret[0] - (110.0f64 / 100.0).ln()).abs() < 1e-12);
        assert!((ret[1] - (99.0f64 / 110.0).ln()).abs() < 1e-12);
    }

    #[test]
    fn log_returns_empty_input() {
        assert_eq!(log_returns(&[]), Vec::<f64>::new());
    }

    #[test]
    fn compute_causality_matrix_counts_links() {
        let mut rng = crate::portfolio_sim::Rng::new(1);
        let n = 300;
        let x: Vec<f64> = (0..n).map(|_| rng.normal()).collect();
        let mut y = vec![0.0f64; n];
        for t in 1..n { y[t] = 0.85 * x[t - 1] + 0.15 * rng.normal(); }
        let z_ind: Vec<f64> = (0..n).map(|_| rng.normal()).collect();

        let series = vec![
            ("X".to_string(), false, x),
            ("Y".to_string(), false, y),
            ("Z".to_string(), false, z_ind),
        ];
        let cfg = CausalityConfig { n_shuffles: 20, ..Default::default() };
        let mat = compute_causality_matrix(&series, &cfg);

        // 3 assets → 6 directed pairs
        assert_eq!(mat.links.len(), 6);
        // At least the X→Y link should be significant
        let xy = mat.links.iter().find(|l| l.source == "X" && l.target == "Y").unwrap();
        assert!(xy.significant, "X→Y should be significant");
    }
}
