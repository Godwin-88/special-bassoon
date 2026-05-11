//! GARCH(1,1) Volatility Model
//!
//! Generalized Autoregressive Conditional Heteroskedasticity with MLE estimation.
//! Replaces the synthetic sine-wave volatility in the backtester.
//!
//! References:
//! - Bollerslev, T. (1986). "Generalized Autoregressive Conditional Heteroskedasticity."
//!   Journal of Econometrics, 31, 307-327.
//! - Engle, R.F. (1982). "Autoregressive Conditional Heteroscedasticity with Estimates of
//!   the Variance of United Kingdom Inflation." Econometrica, 50(4), 987-1007.

use anyhow::{Result, anyhow};

/// Fitted GARCH(1,1) model: σ²_t = ω + α·ε²_{t-1} + β·σ²_{t-1}
///
/// Stationarity constraint: α + β < 1
/// Non-negativity:          ω > 0, α ≥ 0, β ≥ 0
#[derive(Debug, Clone)]
pub struct GARCHModel {
    /// ω: long-run variance intercept (unconditional variance floor)
    pub omega: f64,
    /// α: ARCH coefficient — reaction to new information (recent shock weight)
    pub alpha: f64,
    /// β: GARCH coefficient — variance persistence (memory of past volatility)
    pub beta: f64,
}

/// Results from running GARCH on a return series
#[derive(Debug, Clone)]
pub struct GARCHResult {
    pub conditional_variances: Vec<f64>,
    pub log_likelihood: f64,
    /// α + β: persistence — near 1 means long memory of volatility shocks
    pub persistence: f64,
    /// ω / (1 - α - β): long-run average variance
    pub unconditional_variance: f64,
}

impl GARCHModel {
    /// Fit GARCH(1,1) to a return series via MLE.
    ///
    /// Uses a Nelder-Mead simplex optimizer on the Gaussian log-likelihood.
    /// Returns error if series is too short (<30 observations) or fit fails.
    pub fn fit(returns: &[f64]) -> Result<Self> {
        if returns.len() < 30 {
            return Err(anyhow!("GARCH requires at least 30 returns, got {}", returns.len()));
        }

        // Initial parameter estimates using unconditional variance
        let sample_var = sample_variance(returns);
        let omega_init = sample_var * 0.05;
        let alpha_init = 0.10_f64;
        let beta_init = 0.85_f64;

        let obj = |omega: f64, alpha: f64, beta: f64| -> f64 {
            if omega <= 0.0 || alpha < 0.0 || beta < 0.0 || alpha + beta >= 0.9999 {
                return f64::INFINITY;
            }
            -garch_log_likelihood(returns, omega, alpha, beta)
        };

        let (omega, alpha, beta) = nelder_mead_3d(obj, omega_init, alpha_init, beta_init, 1000, 1e-9)?;

        // Final constraint enforcement
        let omega = omega.max(1e-10);
        let alpha = alpha.max(0.0);
        let beta = beta.max(0.0);
        // Re-scale if stationarity violated
        let (alpha, beta) = if alpha + beta >= 1.0 {
            let scale = 0.999 / (alpha + beta);
            (alpha * scale, beta * scale)
        } else {
            (alpha, beta)
        };

        Ok(Self { omega, alpha, beta })
    }

    /// Compute the conditional variance series σ²_1, ..., σ²_T.
    /// σ²_1 is initialized to the sample variance.
    pub fn conditional_variances(&self, returns: &[f64]) -> Vec<f64> {
        if returns.is_empty() { return vec![]; }
        let mut variances = Vec::with_capacity(returns.len());
        let mut sigma_sq = sample_variance(returns).max(1e-10);

        for &r in returns {
            variances.push(sigma_sq);
            sigma_sq = self.omega + self.alpha * r * r + self.beta * sigma_sq;
            sigma_sq = sigma_sq.max(1e-12);
        }
        variances
    }

    /// Compute full GARCH result including log-likelihood and derived statistics.
    pub fn evaluate(&self, returns: &[f64]) -> GARCHResult {
        let conditional_variances = self.conditional_variances(returns);
        let ll = garch_log_likelihood(returns, self.omega, self.alpha, self.beta);
        let persistence = self.alpha + self.beta;
        let unconditional_variance = if persistence < 1.0 {
            self.omega / (1.0 - persistence)
        } else {
            f64::INFINITY
        };
        GARCHResult { conditional_variances, log_likelihood: ll, persistence, unconditional_variance }
    }

    /// h-step ahead variance forecast from current state.
    ///
    /// σ²_{T+h} = ω·Σ_{i=0}^{h-2} (α+β)^i + (α+β)^{h-1}·σ²_{T+1}
    ///
    /// Converges to unconditional variance as h → ∞ when α+β < 1.
    pub fn forecast(&self, last_eps_sq: f64, last_sigma_sq: f64, h: usize) -> Vec<f64> {
        if h == 0 { return vec![]; }
        let p = self.alpha + self.beta;
        let uv = if p < 1.0 { self.omega / (1.0 - p) } else { last_sigma_sq };

        // One-step ahead
        let sigma_sq_1 = if p < 1.0 {
            self.omega + self.alpha * last_eps_sq + self.beta * last_sigma_sq
        } else {
            uv.max(self.omega + self.alpha * last_eps_sq + self.beta * last_sigma_sq)
        };

        let mut forecasts = Vec::with_capacity(h);
        forecasts.push(sigma_sq_1);

        for step in 1..h {
            // E[σ²_{T+step+1}] = ω + (α+β)·E[σ²_{T+step}]
            let prev = forecasts[step - 1];
            let next = self.omega + p * prev;
            forecasts.push(next);
        }
        forecasts
    }
}

/// Gaussian GARCH(1,1) log-likelihood: L = -0.5·Σ[log(σ²_t) + ε²_t/σ²_t]
fn garch_log_likelihood(returns: &[f64], omega: f64, alpha: f64, beta: f64) -> f64 {
    if returns.is_empty() { return 0.0; }
    let mut sigma_sq = sample_variance(returns).max(1e-12);
    let mut ll = 0.0;

    for &r in returns {
        if sigma_sq <= 0.0 { return f64::NEG_INFINITY; }
        ll -= 0.5 * (sigma_sq.ln() + r * r / sigma_sq);
        sigma_sq = omega + alpha * r * r + beta * sigma_sq;
        sigma_sq = sigma_sq.max(1e-12);
    }
    ll
}

fn sample_variance(data: &[f64]) -> f64 {
    if data.len() < 2 { return 1e-6; }
    let mean = data.iter().sum::<f64>() / data.len() as f64;
    data.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / (data.len() - 1) as f64
}

/// Nelder-Mead simplex for 3-parameter optimization.
fn nelder_mead_3d<F>(
    f: F,
    x0: f64, y0: f64, z0: f64,
    max_iter: usize, tol: f64,
) -> Result<(f64, f64, f64)>
where F: Fn(f64, f64, f64) -> f64
{
    let s = 0.2_f64;
    // 4-vertex simplex
    let mut p: [[f64; 3]; 4] = [
        [x0,      y0,      z0],
        [x0 + s * x0.abs().max(1e-6), y0, z0],
        [x0, y0 + s,      z0],
        [x0, y0,      z0 + s],
    ];
    let mut fval: [f64; 4] = [
        f(p[0][0], p[0][1], p[0][2]),
        f(p[1][0], p[1][1], p[1][2]),
        f(p[2][0], p[2][1], p[2][2]),
        f(p[3][0], p[3][1], p[3][2]),
    ];

    for _ in 0..max_iter {
        let mut order = [0usize, 1, 2, 3];
        order.sort_by(|&a, &b| fval[a].partial_cmp(&fval[b]).unwrap());
        let (best, worst) = (order[0], order[3]);
        let second_worst = order[2];

        if (fval[worst] - fval[best]).abs() < tol { break; }

        // Centroid of best 3
        let cx = (p[order[0]][0] + p[order[1]][0] + p[order[2]][0]) / 3.0;
        let cy = (p[order[0]][1] + p[order[1]][1] + p[order[2]][1]) / 3.0;
        let cz = (p[order[0]][2] + p[order[1]][2] + p[order[2]][2]) / 3.0;

        // Reflection
        let rx = 2.0 * cx - p[worst][0];
        let ry = 2.0 * cy - p[worst][1];
        let rz = 2.0 * cz - p[worst][2];
        let fr = f(rx, ry, rz);

        if fr < fval[best] {
            // Expansion
            let ex = 3.0 * cx - 2.0 * p[worst][0];
            let ey = 3.0 * cy - 2.0 * p[worst][1];
            let ez = 3.0 * cz - 2.0 * p[worst][2];
            let fe = f(ex, ey, ez);
            if fe < fr { p[worst] = [ex, ey, ez]; fval[worst] = fe; }
            else        { p[worst] = [rx, ry, rz]; fval[worst] = fr; }
        } else if fr < fval[second_worst] {
            p[worst] = [rx, ry, rz]; fval[worst] = fr;
        } else {
            // Contraction
            let kx = (cx + p[worst][0]) / 2.0;
            let ky = (cy + p[worst][1]) / 2.0;
            let kz = (cz + p[worst][2]) / 2.0;
            let fk = f(kx, ky, kz);
            if fk < fval[worst] {
                p[worst] = [kx, ky, kz]; fval[worst] = fk;
            } else {
                // Shrink
                for i in 1..4 {
                    let idx = order[i];
                    p[idx][0] = (p[best][0] + p[idx][0]) / 2.0;
                    p[idx][1] = (p[best][1] + p[idx][1]) / 2.0;
                    p[idx][2] = (p[best][2] + p[idx][2]) / 2.0;
                    fval[idx] = f(p[idx][0], p[idx][1], p[idx][2]);
                }
            }
        }
    }

    let best = fval.iter().enumerate()
        .min_by(|a, b| a.1.partial_cmp(b.1).unwrap())
        .map(|(i, _)| i).unwrap_or(0);

    Ok((p[best][0], p[best][1], p[best][2]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ar1_returns(n: usize, vol: f64, seed: u64) -> Vec<f64> {
        // Simple pseudo-random returns with roughly constant vol
        (0..n).map(|i| {
            let x = ((i as f64 * 1.6180339887 + seed as f64) * 2.0 * std::f64::consts::PI).sin();
            x * vol
        }).collect()
    }

    #[test]
    fn test_garch_fit_stationarity() {
        let returns = ar1_returns(200, 0.01, 42);
        let model = GARCHModel::fit(&returns).unwrap();
        assert!(model.omega > 0.0, "omega must be positive");
        assert!(model.alpha >= 0.0, "alpha must be non-negative");
        assert!(model.beta >= 0.0, "beta must be non-negative");
        assert!(model.alpha + model.beta < 1.0, "stationarity: alpha+beta must be < 1");
    }

    #[test]
    fn test_garch_conditional_variances_positive() {
        let returns = ar1_returns(100, 0.02, 7);
        let model = GARCHModel::fit(&returns).unwrap();
        let variances = model.conditional_variances(&returns);
        assert_eq!(variances.len(), returns.len());
        for v in &variances {
            assert!(*v > 0.0 && v.is_finite(), "all conditional variances must be positive finite");
        }
    }

    #[test]
    fn test_garch_forecast_converges() {
        let returns = ar1_returns(200, 0.01, 3);
        let model = GARCHModel::fit(&returns).unwrap();
        let last_r = *returns.last().unwrap();
        let last_v = model.conditional_variances(&returns).last().copied().unwrap();
        let forecasts = model.forecast(last_r * last_r, last_v, 50);

        let uv = model.omega / (1.0 - model.alpha - model.beta);
        let final_forecast = forecasts.last().unwrap();
        // At h=50, forecast should be within 50% of unconditional variance
        assert!((final_forecast - uv).abs() / uv < 0.5,
            "50-step forecast {:.6} should be near unconditional variance {:.6}", final_forecast, uv);
    }

    #[test]
    fn test_garch_requires_min_observations() {
        let too_short = vec![0.01; 10];
        assert!(GARCHModel::fit(&too_short).is_err());
    }
}
