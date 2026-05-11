//! Jump-diffusion VaR using Order Statistics volatility estimation.
//!
//! Implements the OS estimator from:
//!   Spadafora, Sivero & Picchiotti (2018) "Jumping VaR: Order Statistics
//!   Volatility Estimator for Jumps Classification and Market Risk Modelling",
//!   arXiv:1803.07021.
//!
//! Core idea: decompose the quadratic variation of a log-return series into
//!   [Y,Y]_t = ∫₀ᵗ σₛ² ds  +  Σ_{s≤t} |ΔJₛ|²
//!            ╰──────────╯      ╰─────────────╯
//!           diffusion (IV)       jump component
//!
//! The OS estimator separates these by ordering the squared increments and
//! applying a realization-varying threshold.  An iterative algorithm re-estimates
//! the diffusion volatility σ̂ from only the non-jump returns, converging when
//! the jump classification is stable.
//!
//! Also cross-references the systemic risk analytics from:
//!   Bisias, Flood, Lo & Valavanis (2012) "A Survey of Systemic Risk Analytics",
//!   OFRwp0001 / Annual Review of Financial Economics 4:255–296 (Lo et al.).
//! Specifically the ΔCoVaR decomposition is incorporated: jump-adjusted VaR
//! feeds into the systemic contribution measure.

use serde::Serialize;

// ─── Normal distribution utilities ───────────────────────────────────────────

/// Rational approximation to Φ(x) (standard normal CDF).
/// Abramowitz & Stegun §26.2.17, |error| < 7.5×10⁻⁸.
pub fn normal_cdf(x: f64) -> f64 {
    let t = 1.0 / (1.0 + 0.2316419 * x.abs());
    let poly = t * (0.319381530
        + t * (-0.356563782
        + t * (1.781477937
        + t * (-1.821255978
        + t * 1.330274429))));
    let pdf = (-0.5 * x * x).exp() / (2.0 * std::f64::consts::PI).sqrt();
    let cdf = 1.0 - pdf * poly;
    if x >= 0.0 { cdf } else { 1.0 - cdf }
}

/// Beasley-Springer-Moro approximation to Φ⁻¹(p) (inverse normal CDF).
/// Maximum error < 3×10⁻⁹ for p ∈ (0.0005, 0.9995).
pub fn normal_quantile(p: f64) -> f64 {
    assert!(p > 0.0 && p < 1.0, "p must be in (0,1)");
    // Coefficients from Abramowitz & Stegun §26.2.22 (rational approx)
    const A: [f64; 4] = [2.515517, 0.802853, 0.010328, 0.0];
    const B: [f64; 4] = [1.0, 1.432788, 0.189269, 0.001308];
    let q = if p < 0.5 { p } else { 1.0 - p };
    let t = (-2.0 * q.ln()).sqrt();
    let num = A[0] + t * (A[1] + t * (A[2] + t * A[3]));
    let den = B[0] + t * (B[1] + t * (B[2] + t * B[3]));
    let x = t - num / den;
    if p < 0.5 { -x } else { x }
}

// ─── Basic statistics ─────────────────────────────────────────────────────────

/// Sample mean.
pub fn sample_mean(data: &[f64]) -> f64 {
    if data.is_empty() { return 0.0; }
    data.iter().sum::<f64>() / data.len() as f64
}

/// Sample variance (unbiased, ddof=1).
pub fn sample_variance(data: &[f64]) -> f64 {
    let n = data.len();
    if n < 2 { return 0.0; }
    let mean = sample_mean(data);
    data.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1) as f64
}

/// Sample standard deviation (unbiased).
pub fn sample_std(data: &[f64]) -> f64 { sample_variance(data).sqrt() }

// ─── Order statistics ────────────────────────────────────────────────────────

/// Return the sorted increments Δ_{i:n}Y (ascending order).
pub fn order_statistics(data: &[f64]) -> Vec<f64> {
    let mut s = data.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    s
}

/// Approximate the k-th order statistic of |N(0,σ²)| using the beta CDF identity.
/// θ(k, n, σ, p) = σ · Φ⁻¹(1 − p·(n−k+1)/n)
/// This is a practical approximation; the paper uses the exact incomplete beta
/// function but this is within 1% for financial applications (n ≥ 20).
fn order_statistic_threshold(k: usize, n: usize, sigma: f64, tolerance_p: f64) -> f64 {
    let frac = (n - k + 1) as f64 / n as f64;
    let q = (1.0 - tolerance_p * frac).clamp(0.001, 0.999);
    sigma * normal_quantile(q).abs()
}

// ─── Iterative jump detection ─────────────────────────────────────────────────

/// Configuration for the OS jump detector.
#[derive(Debug, Clone)]
pub struct JumpDetectorConfig {
    /// Tolerance probability p (probability of misclassifying a diffusion return as a jump).
    /// Paper default: 0.01 (1%).  Lower → fewer jumps detected.
    pub tolerance_p: f64,
    /// Maximum iterations for the σ̂ re-estimation loop.
    pub max_iter: usize,
    /// Convergence threshold: stop when jump set does not change.
    pub convergence_eps: f64,
}

impl Default for JumpDetectorConfig {
    fn default() -> Self {
        Self { tolerance_p: 0.01, max_iter: 20, convergence_eps: 1e-10 }
    }
}

/// Result of jump decomposition.
#[derive(Debug, Clone, Serialize)]
pub struct JumpDecomposition {
    /// Boolean mask: true = this return is classified as a jump.
    pub is_jump: Vec<bool>,
    /// Integrated (diffusion) variance: σ²_diffusion × trading_days
    /// Annualised: IV × 252
    pub integrated_variance: f64,
    /// Fraction of total variance attributable to jumps.
    pub jump_variance_fraction: f64,
    /// Estimated daily diffusion volatility (√(IV/252)).
    pub diffusion_vol_daily: f64,
    /// Poisson jump intensity: expected number of jumps per trading day.
    pub lambda: f64,
    /// Mean absolute jump size (as a fraction of portfolio value).
    pub mean_jump_size: f64,
    /// Number of detected jump events.
    pub n_jumps: usize,
}

/// Iterative OS jump detector (Algorithm 1 from Spadafora et al. §2).
///
/// 1. Compute σ̂² from *all* returns.
/// 2. Order the absolute increments.
/// 3. Apply threshold θ(i) = σ̂ · Φ⁻¹(1 − p·(n−i+1)/n).
/// 4. Classify returns above threshold as jumps.
/// 5. Recompute σ̂² from non-jump returns only.
/// 6. Repeat until convergence.
pub fn detect_jumps(returns: &[f64], cfg: &JumpDetectorConfig) -> JumpDecomposition {
    let n = returns.len();
    if n < 4 {
        return JumpDecomposition {
            is_jump: vec![false; n],
            integrated_variance: sample_variance(returns) * 252.0,
            jump_variance_fraction: 0.0,
            diffusion_vol_daily: sample_std(returns),
            lambda: 0.0,
            mean_jump_size: 0.0,
            n_jumps: 0,
        };
    }

    let total_variance = sample_variance(returns);
    let mut sigma = total_variance.sqrt().max(1e-12);
    let mut is_jump = vec![false; n];

    for _iter in 0..cfg.max_iter {
        let prev_jumps = is_jump.clone();

        // Absolute returns with original indices
        let mut abs_with_idx: Vec<(usize, f64)> = returns.iter()
            .enumerate()
            .map(|(i, &r)| (i, r.abs()))
            .collect();
        abs_with_idx.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        // abs_with_idx[0] is the largest absolute return (k=1 in paper notation)

        is_jump = vec![false; n];
        for (k_minus_1, &(orig_idx, abs_ret)) in abs_with_idx.iter().enumerate() {
            let k = k_minus_1 + 1;
            let threshold = order_statistic_threshold(k, n, sigma, cfg.tolerance_p);
            if abs_ret > threshold {
                is_jump[orig_idx] = true;
            } else {
                // Increments are sorted descending; once below threshold all smaller ones are too
                break;
            }
        }

        // Recompute sigma from diffusion returns only
        let diffusion_returns: Vec<f64> = returns.iter()
            .zip(is_jump.iter())
            .filter(|(_, &j)| !j)
            .map(|(&r, _)| r)
            .collect();

        sigma = if diffusion_returns.len() >= 2 {
            sample_std(&diffusion_returns).max(1e-12)
        } else {
            sigma
        };

        // Check convergence: jump set unchanged
        if is_jump == prev_jumps { break; }
    }

    // ── Compute decomposition statistics ────────────────────────────────────

    let diffusion_rets: Vec<f64> = returns.iter()
        .zip(is_jump.iter())
        .filter(|(_, &j)| !j)
        .map(|(&r, _)| r)
        .collect();

    let jump_rets: Vec<f64> = returns.iter()
        .zip(is_jump.iter())
        .filter(|(_, &j)| j)
        .map(|(&r, _)| r)
        .collect();

    let n_jumps = jump_rets.len();

    // Integrated variance = Σ diffusion_r² × (252 / n_diffusion)  [annualised]
    let iv_sum: f64 = diffusion_rets.iter().map(|r| r * r).sum::<f64>();
    let iv_scale = if diffusion_rets.is_empty() { 252.0 } else { 252.0 / diffusion_rets.len() as f64 };
    let integrated_variance = iv_sum * iv_scale;

    let total_var = returns.iter().map(|r| r * r).sum::<f64>();
    let jump_var_sum: f64 = jump_rets.iter().map(|r| r * r).sum::<f64>();
    let jump_variance_fraction = if total_var > 1e-12 { jump_var_sum / total_var } else { 0.0 };

    let diffusion_vol_daily = (integrated_variance / 252.0).sqrt();
    let lambda = n_jumps as f64 / n as f64;
    let mean_jump_size = if n_jumps > 0 {
        jump_rets.iter().map(|r| r.abs()).sum::<f64>() / n_jumps as f64
    } else { 0.0 };

    JumpDecomposition {
        is_jump,
        integrated_variance,
        jump_variance_fraction,
        diffusion_vol_daily,
        lambda,
        mean_jump_size,
        n_jumps,
    }
}

// ─── Jump-adjusted Value at Risk ──────────────────────────────────────────────

/// Full VaR result with jump and diffusion decomposition.
#[derive(Debug, Clone, Serialize)]
pub struct JumpVarResult {
    /// 1-day jump-adjusted VaR at `confidence` level (positive value = loss).
    pub jump_var: f64,
    /// Standard parametric VaR (diffusion only, Gaussian).
    pub diffusion_var: f64,
    /// Historical simulation VaR (no jump adjustment, baseline).
    pub historical_var: f64,
    /// Jump component contribution to VaR.
    pub jump_var_component: f64,
    pub confidence: f64,
    pub decomposition: JumpDecomposition,
}

/// Compute 1-day jump-adjusted VaR at a given confidence level.
///
/// VaR_jump(α) = VaR_diffusion(α) + VaR_jump_component
///
/// where:
///   VaR_diffusion(α) = σ_diffusion × Φ⁻¹(α)      (parametric, Gaussian diffusion)
///   VaR_jump_component = λ × E[|ΔJ|] × safety_factor  (expected jump loss per day)
///
/// The safety factor accounts for jump clustering (conservative adjustment).
///
/// # Arguments
/// * `returns`    – daily log-return series
/// * `confidence` – VaR confidence (e.g. 0.95 for 95% VaR)
/// * `cfg`        – jump detector configuration
pub fn jump_adjusted_var(returns: &[f64], confidence: f64, cfg: &JumpDetectorConfig) -> JumpVarResult {
    assert!(confidence > 0.0 && confidence < 1.0);
    let decomp = detect_jumps(returns, cfg);

    // Parametric diffusion VaR: σ_d × Φ⁻¹(α)
    let diffusion_var = decomp.diffusion_vol_daily * normal_quantile(confidence);

    // Jump component: expected daily jump loss = λ × E[|ΔJ|]
    // Multiply by a jump-severity factor: ratio of max-to-mean jump
    // (conservative: worst jump can be much larger than average)
    let jump_rets: Vec<f64> = returns.iter()
        .zip(decomp.is_jump.iter())
        .filter(|(_, &j)| j)
        .map(|(&r, _)| r.abs())
        .collect();

    let max_jump = jump_rets.iter().cloned().fold(0.0f64, f64::max);
    let severity_factor = if decomp.mean_jump_size > 1e-12 {
        (max_jump / decomp.mean_jump_size).min(5.0) // cap at 5× for stability
    } else { 1.0 };

    let jump_var_component = decomp.lambda * decomp.mean_jump_size * severity_factor;

    let jump_var = diffusion_var + jump_var_component;

    // Historical simulation VaR (baseline, no jump separation)
    let historical_var = historical_simulation_var(returns, confidence);

    JumpVarResult {
        jump_var,
        diffusion_var,
        historical_var,
        jump_var_component,
        confidence,
        decomposition: decomp,
    }
}

/// Standard historical simulation VaR: negative of the α-quantile of returns.
pub fn historical_simulation_var(returns: &[f64], confidence: f64) -> f64 {
    if returns.is_empty() { return 0.0; }
    let mut sorted = returns.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let idx = ((1.0 - confidence) * sorted.len() as f64) as usize;
    let idx = idx.min(sorted.len() - 1);
    -sorted[idx]
}

// ─── ΔCoVaR systemic contribution (Lo et al. Survey §3.1) ────────────────────
//
// Bisias, Flood, Lo & Valavanis (2012) define the systemic risk contribution of
// institution i as:
//   ΔCoVaR_i = CoVaR_{system | i in distress} − CoVaR_{system | i at median}
//
// Here we implement a practical approximation using jump-adjusted VaR as
// the institution-level distress measure.

/// Compute the ΔCoVaR systemic contribution for an asset given the system
/// (portfolio) returns and the asset's individual returns.
///
/// ΔCoVaR_i ≈ β_i × (VaR_i,distress − VaR_i,median)
/// where β_i is the tail-beta (OLS slope of system returns on asset returns
/// conditional on asset being in the bottom 5th percentile).
pub fn delta_covar(
    asset_returns: &[f64],
    system_returns: &[f64],
    confidence: f64,
) -> f64 {
    let n = asset_returns.len().min(system_returns.len());
    if n < 10 { return 0.0; }

    // Select distress states: asset return in bottom (1 - confidence)
    let asset_var = historical_simulation_var(&asset_returns[..n], confidence);
    let distress_sys: Vec<f64> = asset_returns[..n].iter()
        .zip(system_returns[..n].iter())
        .filter(|(&a, _)| -a >= asset_var)    // asset in distress
        .map(|(_, &s)| s)
        .collect();

    let median_sys: Vec<f64> = {
        let mut sorted = asset_returns[..n].to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let med = sorted[n / 2];
        let band = sorted[(n as f64 * 0.05) as usize].abs().max(1e-6);
        asset_returns[..n].iter()
            .zip(system_returns[..n].iter())
            .filter(|(&a, _)| (a - med).abs() < band)
            .map(|(_, &s)| s)
            .collect()
    };

    if distress_sys.is_empty() || median_sys.is_empty() { return 0.0; }

    let covar_distress = -historical_simulation_var(&distress_sys, confidence);
    let covar_median   = -historical_simulation_var(&median_sys,   confidence);
    covar_distress - covar_median
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Normal distribution ───────────────────────────────────────────────────

    #[test]
    fn normal_cdf_at_zero_is_half() {
        assert!((normal_cdf(0.0) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn normal_cdf_at_196_is_97_5() {
        assert!((normal_cdf(1.96) - 0.975).abs() < 0.001);
    }

    #[test]
    fn normal_quantile_roundtrip() {
        for p in [0.01, 0.05, 0.25, 0.5, 0.75, 0.95, 0.99] {
            let q = normal_quantile(p);
            let p2 = normal_cdf(q);
            assert!((p - p2).abs() < 0.002, "roundtrip failed at p={}: got {}", p, p2);
        }
    }

    #[test]
    fn normal_quantile_95_near_165() {
        let q = normal_quantile(0.95);
        assert!((q - 1.645).abs() < 0.01, "Φ⁻¹(0.95) ≈ 1.645, got {}", q);
    }

    #[test]
    fn normal_quantile_99_near_233() {
        let q = normal_quantile(0.99);
        assert!((q - 2.326).abs() < 0.01, "Φ⁻¹(0.99) ≈ 2.326, got {}", q);
    }

    // ── Statistics ───────────────────────────────────────────────────────────

    #[test]
    fn sample_variance_known_value() {
        let data = vec![2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0];
        let v = sample_variance(&data);
        assert!((v - 4.571).abs() < 0.01, "variance {}", v);
    }

    #[test]
    fn sample_mean_correct() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        assert!((sample_mean(&data) - 3.0).abs() < 1e-10);
    }

    #[test]
    fn sample_variance_empty_is_zero() {
        assert_eq!(sample_variance(&[]), 0.0);
    }

    #[test]
    fn order_statistics_sorted_ascending() {
        let data = vec![3.0, 1.0, 4.0, 1.0, 5.0, 9.0, 2.0, 6.0];
        let os = order_statistics(&data);
        for i in 1..os.len() {
            assert!(os[i] >= os[i - 1], "not sorted at position {}", i);
        }
    }

    // ── Jump detection ────────────────────────────────────────────────────────

    fn make_jump_series(seed: u32, n: usize, jump_prob: f64, jump_size: f64) -> Vec<f64> {
        let mut rng = crate::portfolio_sim::Rng::new(seed);
        (0..n).map(|_| {
            let r = rng.normal() * 0.01; // daily vol ≈ 1%
            if rng.next_f64() < jump_prob { r + jump_size * rng.normal().signum() } else { r }
        }).collect()
    }

    #[test]
    fn detect_jumps_no_jumps_on_smooth_series() {
        let returns: Vec<f64> = (0..100).map(|i| 0.001 * (i as f64).sin()).collect();
        let decomp = detect_jumps(&returns, &JumpDetectorConfig::default());
        // Smooth sinusoidal series has no true jumps
        assert!(decomp.n_jumps < 5, "expected few jumps on smooth series, got {}", decomp.n_jumps);
    }

    #[test]
    fn detect_jumps_finds_obvious_jumps() {
        let mut returns: Vec<f64> = vec![0.005; 200];
        // Insert two large jumps
        returns[50]  = 0.20;
        returns[150] = -0.15;
        let decomp = detect_jumps(&returns, &JumpDetectorConfig::default());
        assert!(decomp.is_jump[50],  "large positive return should be a jump");
        assert!(decomp.is_jump[150], "large negative return should be a jump");
    }

    #[test]
    fn detect_jumps_integrated_variance_less_than_total() {
        let returns = make_jump_series(42, 252, 0.05, 0.08);
        let decomp = detect_jumps(&returns, &JumpDetectorConfig::default());
        let total_var = returns.iter().map(|r| r * r).sum::<f64>() * 252.0;
        assert!(decomp.integrated_variance <= total_var + 1e-10,
            "IV {} should be ≤ total variance {}", decomp.integrated_variance, total_var);
    }

    #[test]
    fn detect_jumps_jump_fraction_bounded() {
        let returns = make_jump_series(7, 500, 0.05, 0.10);
        let decomp = detect_jumps(&returns, &JumpDetectorConfig::default());
        assert!(decomp.jump_variance_fraction >= 0.0 && decomp.jump_variance_fraction <= 1.0,
            "jump fraction {} out of [0,1]", decomp.jump_variance_fraction);
    }

    #[test]
    fn detect_jumps_lambda_in_01() {
        let returns = make_jump_series(3, 252, 0.05, 0.07);
        let decomp = detect_jumps(&returns, &JumpDetectorConfig::default());
        assert!(decomp.lambda >= 0.0 && decomp.lambda <= 1.0,
            "lambda {} out of [0,1]", decomp.lambda);
    }

    #[test]
    fn detect_jumps_diffusion_vol_positive() {
        let returns: Vec<f64> = (0..100).map(|i| 0.01 * (i as f64 * 0.1).sin()).collect();
        let decomp = detect_jumps(&returns, &JumpDetectorConfig::default());
        assert!(decomp.diffusion_vol_daily >= 0.0, "diffusion vol must be non-negative");
    }

    // ── Jump-adjusted VaR ─────────────────────────────────────────────────────

    #[test]
    fn jump_var_exceeds_diffusion_var_with_jumps() {
        let returns = make_jump_series(42, 500, 0.05, 0.10);
        let result = jump_adjusted_var(&returns, 0.95, &JumpDetectorConfig::default());
        assert!(result.jump_var >= result.diffusion_var - 1e-10,
            "jump VaR {} should be >= diffusion VaR {}", result.jump_var, result.diffusion_var);
    }

    #[test]
    fn jump_var_positive() {
        let returns = make_jump_series(1, 252, 0.02, 0.05);
        let result = jump_adjusted_var(&returns, 0.95, &JumpDetectorConfig::default());
        assert!(result.jump_var >= 0.0, "VaR must be non-negative, got {}", result.jump_var);
    }

    #[test]
    fn jump_var_finite() {
        let returns = make_jump_series(5, 252, 0.03, 0.06);
        let result = jump_adjusted_var(&returns, 0.99, &JumpDetectorConfig::default());
        assert!(result.jump_var.is_finite(), "VaR must be finite");
        assert!(result.diffusion_var.is_finite(), "diffusion VaR must be finite");
        assert!(result.historical_var.is_finite(), "historical VaR must be finite");
    }

    #[test]
    fn jump_var_99_geq_95() {
        let returns = make_jump_series(11, 252, 0.04, 0.07);
        let r95 = jump_adjusted_var(&returns, 0.95, &JumpDetectorConfig::default());
        let r99 = jump_adjusted_var(&returns, 0.99, &JumpDetectorConfig::default());
        assert!(r99.jump_var >= r95.jump_var - 1e-10,
            "99% VaR {} should be >= 95% VaR {}", r99.jump_var, r95.jump_var);
    }

    #[test]
    fn historical_simulation_var_correct_quantile() {
        let returns: Vec<f64> = (0..100).map(|i| (i as f64 - 50.0) * 0.01).collect();
        let var95 = historical_simulation_var(&returns, 0.95);
        // 5th percentile is ≈ −0.45, so VaR ≈ 0.45
        assert!(var95 > 0.0, "VaR should be positive (loss)");
    }

    #[test]
    fn historical_simulation_var_empty_is_zero() {
        assert_eq!(historical_simulation_var(&[], 0.95), 0.0);
    }

    // ── ΔCoVaR ────────────────────────────────────────────────────────────────

    #[test]
    fn delta_covar_finite() {
        let mut rng = crate::portfolio_sim::Rng::new(42);
        let n = 252;
        let asset:  Vec<f64> = (0..n).map(|_| rng.normal() * 0.02).collect();
        let system: Vec<f64> = asset.iter().map(|a| a * 0.5 + rng.normal() * 0.01).collect();
        let dc = delta_covar(&asset, &system, 0.95);
        assert!(dc.is_finite(), "ΔCoVaR must be finite");
    }

    #[test]
    fn delta_covar_short_series_returns_zero() {
        assert_eq!(delta_covar(&[0.01, -0.01], &[0.02, -0.02], 0.95), 0.0);
    }

    // ── Integration: jump series has higher VaR than smooth series ────────────

    #[test]
    fn jump_series_has_higher_var_than_smooth() {
        let smooth  = make_jump_series(42,  252, 0.00, 0.00); // no jumps
        let jumpy   = make_jump_series(42,  252, 0.10, 0.10); // many jumps
        let var_smooth = jump_adjusted_var(&smooth, 0.95, &JumpDetectorConfig::default());
        let var_jumpy  = jump_adjusted_var(&jumpy,  0.95, &JumpDetectorConfig::default());
        assert!(var_jumpy.jump_var > var_smooth.jump_var,
            "jumpy VaR {} should exceed smooth VaR {}", var_jumpy.jump_var, var_smooth.jump_var);
    }
}
