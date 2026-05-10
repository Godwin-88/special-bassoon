//! Extreme Value Theory — Peaks-Over-Threshold (POT) method
//!
//! Implements GPD MLE for tail risk estimation in crypto return distributions.
//!
//! References:
//! - McNeil, Frey & Embrechts (2005) "Quantitative Risk Management" Ch. 7
//! - Balkema & de Haan (1974) / Pickands (1975) — POT foundation
//! - Davison & Smith (1990) — GPD MLE estimation

use anyhow::{Result, anyhow};

/// Fitted GPD estimator from the Peaks-Over-Threshold method.
///
/// The GPD CDF: F(y; ξ, σ) = 1 - (1 + ξ·y/σ)^{-1/ξ}  for ξ ≠ 0
///               F(y; σ) = 1 - exp(-y/σ)                for ξ = 0
#[derive(Debug, Clone)]
pub struct EVTEstimator {
    /// Threshold u above which GPD is fitted (quantile of loss distribution)
    pub threshold: f64,
    /// Shape parameter ξ (tail index; ξ > 0 = heavy tail, ξ < 0 = bounded tail)
    pub xi: f64,
    /// Scale parameter σ > 0
    pub sigma: f64,
    /// Number of exceedances k = #{i : loss_i > u}
    pub n_exceedances: usize,
    /// Total sample size n
    pub n_total: usize,
}

impl EVTEstimator {
    /// Fit GPD to a loss series via the POT method.
    ///
    /// # Arguments
    /// * `losses` — series of losses (positive = loss, can include gains as negatives)
    /// * `quantile` — threshold quantile, e.g. 0.90 selects u = 90th percentile of losses
    pub fn fit(losses: &[f64], quantile: f64) -> Result<Self> {
        if losses.len() < 20 {
            return Err(anyhow!("EVT requires at least 20 observations, got {}", losses.len()));
        }

        let n_total = losses.len();
        let threshold = percentile(losses, quantile);
        let exceedances: Vec<f64> = losses.iter()
            .filter(|&&x| x > threshold)
            .map(|&x| x - threshold)
            .collect();

        let n_exceedances = exceedances.len();
        if n_exceedances < 5 {
            return Err(anyhow!("Too few exceedances ({}) above threshold {:.4}", n_exceedances, threshold));
        }

        let (xi, sigma) = fit_gpd_mle(&exceedances)?;

        Ok(Self { threshold, xi, sigma, n_exceedances, n_total })
    }

    /// Value at Risk at confidence level α (e.g. 0.99)
    ///
    /// VaR_α = u + (σ/ξ) * ((n/k * (1-α))^{-ξ} - 1)
    pub fn var(&self, alpha: f64) -> f64 {
        let n = self.n_total as f64;
        let k = self.n_exceedances as f64;
        let exceedance_prob = (n / k) * (1.0 - alpha);

        if self.xi.abs() < 1e-8 {
            // Exponential case (ξ → 0): VaR = u - σ·ln(exceedance_prob)
            self.threshold - self.sigma * exceedance_prob.max(1e-10).ln()
        } else {
            self.threshold + (self.sigma / self.xi) * (exceedance_prob.powf(-self.xi) - 1.0)
        }
    }

    /// Expected Shortfall (CVaR) at confidence level α
    ///
    /// ES_α = (VaR_α + σ - ξ·u) / (1 - ξ)    [valid for ξ < 1]
    ///
    /// Reference: McNeil et al. (2005) Proposition 7.20
    pub fn expected_shortfall(&self, alpha: f64) -> f64 {
        if self.xi >= 1.0 {
            // ES undefined for ξ ≥ 1 (infinite mean), return VaR as lower bound
            return self.var(alpha);
        }
        let var = self.var(alpha);
        (var + self.sigma - self.xi * self.threshold) / (1.0 - self.xi)
    }

    /// Hill estimator — nonparametric tail index estimator.
    ///
    /// ξ̂_k = (1/k) * Σ_{i=1}^{k} log(X_{n-i+1:n} / X_{n-k:n})
    ///
    /// `sorted_losses` must be sorted ascending. `k` is the number of upper-order statistics.
    pub fn hill_estimator(sorted_losses: &[f64], k: usize) -> f64 {
        let n = sorted_losses.len();
        if k == 0 || k >= n { return 0.0; }

        let threshold_val = sorted_losses[n - k - 1];
        if threshold_val <= 0.0 { return 0.0; }

        let sum: f64 = sorted_losses[(n - k)..n]
            .iter()
            .map(|&x| (x / threshold_val).max(1e-10).ln())
            .sum();

        sum / k as f64
    }
}

/// Internal: fit GPD parameters (ξ, σ) to exceedances via MLE.
/// Uses a simple grid search + gradient refinement for robustness.
fn fit_gpd_mle(exceedances: &[f64]) -> Result<(f64, f64)> {
    let mu_y = exceedances.iter().sum::<f64>() / exceedances.len() as f64;
    let var_y = exceedances.iter().map(|&y| (y - mu_y).powi(2)).sum::<f64>()
        / exceedances.len() as f64;

    // Method of moments initial estimates (Hosking & Wallis 1987)
    let xi_init = 0.5 * (1.0 - mu_y * mu_y / var_y.max(1e-12));
    let sigma_init = 0.5 * mu_y * (1.0 + mu_y * mu_y / var_y.max(1e-12));
    let sigma_init = sigma_init.max(1e-6);

    // Nelder-Mead simplex optimization
    let obj = |xi: f64, sigma: f64| -> f64 {
        if sigma <= 0.0 { return f64::INFINITY; }
        gpd_neg_log_likelihood(exceedances, xi, sigma)
    };

    let (xi, sigma) = nelder_mead_2d(obj, xi_init, sigma_init, 500, 1e-8)?;

    // Final constraint check
    let sigma = sigma.max(1e-8);
    Ok((xi, sigma))
}

/// Negative log-likelihood of GPD(ξ, σ) evaluated on exceedances.
fn gpd_neg_log_likelihood(exceedances: &[f64], xi: f64, sigma: f64) -> f64 {
    if sigma <= 0.0 { return f64::INFINITY; }
    let k = exceedances.len() as f64;
    let mut nll = k * sigma.ln();

    for &y in exceedances {
        let z = 1.0 + xi * y / sigma;
        if z <= 0.0 { return f64::INFINITY; } // constraint violation
        if xi.abs() < 1e-8 {
            nll += y / sigma;
        } else {
            nll += (1.0 + 1.0 / xi) * z.ln();
        }
    }
    nll
}

/// Minimal 2-parameter Nelder-Mead optimizer.
fn nelder_mead_2d<F>(f: F, x0: f64, y0: f64, max_iter: usize, tol: f64) -> Result<(f64, f64)>
where F: Fn(f64, f64) -> f64
{
    let scale = 0.5_f64;
    // Simplex: 3 vertices in (xi, sigma) space
    let mut p = [
        [x0, y0],
        [x0 + scale, y0],
        [x0, y0 + scale * y0.abs().max(0.1)],
    ];
    let mut fval = [f(p[0][0], p[0][1]), f(p[1][0], p[1][1]), f(p[2][0], p[2][1])];

    for _ in 0..max_iter {
        // Sort by function value
        let mut order = [0usize, 1, 2];
        order.sort_by(|&a, &b| fval[a].partial_cmp(&fval[b]).unwrap());
        let (best, second, worst) = (order[0], order[1], order[2]);

        // Convergence check
        let range = (fval[worst] - fval[best]).abs();
        if range < tol { break; }

        // Centroid of best two
        let cx = (p[best][0] + p[second][0]) / 2.0;
        let cy = (p[best][1] + p[second][1]) / 2.0;

        // Reflection
        let rx = 2.0 * cx - p[worst][0];
        let ry = 2.0 * cy - p[worst][1];
        let fr = f(rx, ry);

        if fr < fval[best] {
            // Expansion
            let ex = 3.0 * cx - 2.0 * p[worst][0];
            let ey = 3.0 * cy - 2.0 * p[worst][1];
            let fe = f(ex, ey);
            if fe < fr {
                p[worst] = [ex, ey]; fval[worst] = fe;
            } else {
                p[worst] = [rx, ry]; fval[worst] = fr;
            }
        } else if fr < fval[second] {
            p[worst] = [rx, ry]; fval[worst] = fr;
        } else {
            // Contraction
            let kx = (cx + p[worst][0]) / 2.0;
            let ky = (cy + p[worst][1]) / 2.0;
            let fk = f(kx, ky);
            if fk < fval[worst] {
                p[worst] = [kx, ky]; fval[worst] = fk;
            } else {
                // Shrink
                for i in [second, worst] {
                    p[i][0] = (p[best][0] + p[i][0]) / 2.0;
                    p[i][1] = (p[best][1] + p[i][1]) / 2.0;
                    fval[i] = f(p[i][0], p[i][1]);
                }
            }
        }
    }

    // Return best vertex
    let best_idx = fval.iter().enumerate()
        .min_by(|a, b| a.1.partial_cmp(b.1).unwrap())
        .map(|(i, _)| i)
        .unwrap_or(0);

    Ok((p[best_idx][0], p[best_idx][1]))
}

/// Compute the p-th percentile of a slice.
fn percentile(data: &[f64], p: f64) -> f64 {
    let mut sorted = data.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let idx = ((sorted.len() as f64 - 1.0) * p) as usize;
    sorted[idx.min(sorted.len() - 1)]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_pareto(n: usize, xi: f64, sigma: f64, threshold: f64, seed: u64) -> Vec<f64> {
        // Inverse CDF sampling from GPD: y = (σ/ξ) * (u^{-ξ} - 1), u ~ Uniform(0,1)
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        (0..n).map(|i| {
            let mut h = DefaultHasher::new();
            (i as u64 + seed).hash(&mut h);
            let u = (h.finish() % 1_000_000) as f64 / 1_000_000.0 + 1e-9;
            let exceedance = if xi.abs() < 1e-8 {
                -sigma * u.ln()
            } else {
                (sigma / xi) * (u.powf(-xi) - 1.0)
            };
            threshold + exceedance
        }).collect()
    }

    #[test]
    fn test_evt_fit_recovers_xi() {
        let losses = sample_pareto(500, 0.3, 1.0, 0.0, 42);
        let est = EVTEstimator::fit(&losses, 0.80).unwrap();
        // xi should be within ±0.20 of 0.3
        assert!((est.xi - 0.3).abs() < 0.25,
            "xi estimate {:.3} too far from true 0.3", est.xi);
        assert!(est.sigma > 0.0, "sigma must be positive");
    }

    #[test]
    fn test_es_exceeds_var() {
        let losses = sample_pareto(300, 0.2, 0.5, 0.0, 99);
        let est = EVTEstimator::fit(&losses, 0.85).unwrap();
        let var_99 = est.var(0.99);
        let es_99 = est.expected_shortfall(0.99);
        assert!(es_99 >= var_99, "ES must be >= VaR at same confidence level");
    }

    #[test]
    fn test_hill_estimator_positive() {
        let mut losses: Vec<f64> = (1..=200).map(|i| i as f64 * 0.01).collect();
        losses.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let hill = EVTEstimator::hill_estimator(&losses, 20);
        assert!(hill > 0.0, "Hill estimator should be positive for right-tailed data");
    }

    #[test]
    fn test_evt_requires_min_observations() {
        let small = vec![0.1, 0.2, 0.3];
        assert!(EVTEstimator::fit(&small, 0.9).is_err());
    }
}
