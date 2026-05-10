/// Risk correlation graph for understanding contagion paths
use ndarray::{Array1, Array2};
use crate::client::GraphClient;
use anyhow::Result;
use rand::SeedableRng;
use rand_distr::{Distribution, Normal};
use std::collections::HashMap;

pub struct RiskGraph {
    /// Adjacency matrix: risk_i -> risk_j indicates correlation
    pub correlation_matrix: Array2<f64>,
    /// Sorted list of risk node names
    pub risk_nodes: Vec<String>,
    /// Mapping from risk name to index in matrix
    pub node_to_idx: HashMap<String, usize>,
}

impl RiskGraph {
    pub fn new() -> Self {
        Self {
            correlation_matrix: Array2::zeros((0, 0)),
            risk_nodes: vec![],
            node_to_idx: HashMap::new(),
        }
    }

    /// Construct a 7-node mock risk graph for testing and API use when Neo4j is unavailable.
    /// Nodes represent the primary DeFi systemic risk categories.
    pub fn new_with_mock_data() -> Self {
        let names = vec![
            "bridge_failure", "stablecoin_depeg", "oracle_manipulation",
            "governance_attack", "liquidity_crunch", "validator_concentration", "liquidation_cascade",
        ];
        let n = names.len();
        let risk_nodes: Vec<String> = names.iter().map(|s| s.to_string()).collect();
        let node_to_idx: HashMap<String, usize> = risk_nodes.iter()
            .enumerate()
            .map(|(i, name)| (name.clone(), i))
            .collect();

        // Empirically motivated correlation weights (symmetric)
        let weights: [[f64; 7]; 7] = [
            [1.0, 0.6, 0.2, 0.1, 0.7, 0.1, 0.8],
            [0.6, 1.0, 0.5, 0.2, 0.6, 0.1, 0.7],
            [0.2, 0.5, 1.0, 0.3, 0.2, 0.1, 0.4],
            [0.1, 0.2, 0.3, 1.0, 0.1, 0.4, 0.3],
            [0.7, 0.6, 0.2, 0.1, 1.0, 0.2, 0.9],
            [0.1, 0.1, 0.1, 0.4, 0.2, 1.0, 0.2],
            [0.8, 0.7, 0.4, 0.3, 0.9, 0.2, 1.0],
        ];
        let correlation_matrix = Array2::from_shape_fn((n, n), |(i, j)| weights[i][j]);

        Self { correlation_matrix, risk_nodes, node_to_idx }
    }

    /// Build the risk correlation graph from Neo4j data
    pub async fn build_from_graph(&mut self, client: &GraphClient) -> Result<()> {
        let correlations = client.get_risk_correlations().await?;
        
        // Collect all unique risk names
        let mut unique_risks = std::collections::HashSet::new();
        for corr in &correlations {
            unique_risks.insert(corr.from_risk.clone());
            unique_risks.insert(corr.to_risk.clone());
        }
        
        self.risk_nodes = unique_risks.into_iter().collect();
        self.risk_nodes.sort();
        
        self.node_to_idx = self.risk_nodes.iter()
            .enumerate()
            .map(|(i, name)| (name.clone(), i))
            .collect();
            
        let n = self.risk_nodes.len();
        self.correlation_matrix = Array2::zeros((n, n));
        
        // Diagonal is always 1.0 (self-correlation)
        for i in 0..n {
            self.correlation_matrix[[i, i]] = 1.0;
        }
        
        for corr in correlations {
            if let (Some(&i), Some(&j)) = (self.node_to_idx.get(&corr.from_risk), self.node_to_idx.get(&corr.to_risk)) {
                self.correlation_matrix[[i, j]] = 1.0;
                // If it's a correlation, it's usually symmetric
                self.correlation_matrix[[j, i]] = 1.0;
            }
        }
        
        Ok(())
    }

    /// Get correlation between two risks
    pub fn get_correlation(&self, risk_a: &str, risk_b: &str) -> f64 {
        if let (Some(&i), Some(&j)) = (self.node_to_idx.get(risk_a), self.node_to_idx.get(risk_b)) {
            self.correlation_matrix[[i, j]]
        } else {
            0.0
        }
    }

    /// Deterministic linear contagion propagation (legacy).
    #[deprecated(note = "Use simulate_contagion_mc for stochastic Monte Carlo propagation")]
    pub fn simulate_contagion(&self, initial_shocks: HashMap<String, f64>, iterations: usize) -> HashMap<String, f64> {
        let n = self.risk_nodes.len();
        let mut current_shocks = Array1::zeros(n);
        for (risk, shock) in initial_shocks {
            if let Some(&idx) = self.node_to_idx.get(&risk) {
                current_shocks[idx] = shock;
            }
        }
        for _ in 0..iterations {
            let next = self.correlation_matrix.dot(&current_shocks);
            current_shocks = next.mapv(|s| s.min(1.0));
        }
        self.risk_nodes.iter().enumerate()
            .map(|(i, name)| (name.clone(), current_shocks[i]))
            .collect()
    }

    /// Stochastic Monte Carlo contagion simulation (Eisenberg-Noe inspired).
    ///
    /// Models three mechanisms absent from the deterministic version:
    /// 1. Stochastic amplification — Normal(0, noise_std) perturbation per step
    /// 2. Binary liquidation cascade — if shock exceeds `liquidation_threshold`, node tips to 1.0
    /// 3. Averaging over `n_simulations` independent runs for expected contagion level
    ///
    /// Reference: Eisenberg & Noe (2001), "Systemic Risk in Financial Networks"
    pub fn simulate_contagion_mc(
        &self,
        initial_shocks: HashMap<String, f64>,
        iterations: usize,
        n_simulations: usize,
        liquidation_threshold: f64,
        noise_std: f64,
        rng_seed: u64,
    ) -> HashMap<String, f64> {
        let n = self.risk_nodes.len();
        if n == 0 || n_simulations == 0 { return HashMap::new(); }

        let mut base_shocks = Array1::zeros(n);
        for (risk, shock) in &initial_shocks {
            if let Some(&idx) = self.node_to_idx.get(risk) {
                base_shocks[idx] = *shock;
            }
        }

        let normal = Normal::new(0.0f64, noise_std).expect("valid normal distribution");
        let mut accumulated: Array1<f64> = Array1::zeros(n);

        for sim in 0..n_simulations {
            let mut s = base_shocks.clone();
            let mut rng = rand::rngs::StdRng::seed_from_u64(rng_seed.wrapping_add(sim as u64));

            for _ in 0..iterations {
                let det = self.correlation_matrix.dot(&s);
                let mut next = Array1::zeros(n);
                for i in 0..n {
                    let noisy = det[i] + normal.sample(&mut rng);
                    let clamped = noisy.clamp(0.0, 1.0);
                    // Binary tipping: exceeding threshold triggers full cascade on node
                    next[i] = if clamped > liquidation_threshold { 1.0 } else { clamped };
                }
                s = next;
            }
            accumulated = accumulated + s;
        }

        let mean_shocks = accumulated / n_simulations as f64;
        self.risk_nodes.iter().enumerate()
            .map(|(i, name)| (name.clone(), mean_shocks[i].clamp(0.0, 1.0)))
            .collect()
    }

    /// Calculate Absorption Ratio (Kritzman et al. 2011)
    /// Detects synchronized market fragility where all assets move together
    /// 
    /// Formula: AR_n = sum(lambda_i for i=1 to n) / sum(all lambda_i)
    /// where lambda_i are eigenvalues of correlation matrix, sorted descending
    /// 
    /// Interpretation:
    /// - AR > 0.6: HIGH SYSTEMIC RISK (all assets correlated) 
    /// - AR in [0.4, 0.6]: Medium risk
    /// - AR < 0.4: Low systemic risk (diversification working)
    pub fn calculate_absorption_ratio(&self, n_top: usize) -> Result<f64> {
        if self.correlation_matrix.is_empty() {
            return Ok(0.0);
        }

        let n = self.correlation_matrix.nrows();
        if n == 0 || n_top == 0 { return Ok(0.0); }
        let n_top_clamped = n_top.min(n);

        // Compute eigenvalues using power iteration (numerically stable)
        let eigenvalues = self.compute_eigenvalues(n_top_clamped)?;
        
        // Sum of top eigenvalues
        let sum_top: f64 = eigenvalues.iter().sum();
        
        // Trace (sum of diagonal) = sum of all eigenvalues for correlation matrices
        let trace: f64 = self.correlation_matrix.diag().iter().sum();
        let trace = trace.max(1e-8);
        
        // Absorption ratio (clamped to [0, 1])
        Ok((sum_top / trace).min(1.0).max(0.0))
    }

    /// Compute top K eigenvalues using power iteration with Hotelling deflation.
    ///
    /// After each eigenpair (λ_k, v_k) is found, applies rank-1 deflation:
    ///   A' = A - λ_k · (v_k ⊗ v_k)
    ///
    /// This preserves matrix symmetry and ensures subsequent iterations find
    /// eigenvectors orthogonal to all previously found eigenvectors.
    ///
    /// Reference: Hotelling, H. (1933). Analysis of a complex of statistical variables.
    fn compute_eigenvalues(&self, k: usize) -> Result<Vec<f64>> {
        let n = self.correlation_matrix.nrows();
        let mut eigenvalues = Vec::new();
        let mut a = self.correlation_matrix.clone();

        for _ in 0..k.min(n) {
            let (lambda, v) = self.power_iteration(&a)?;
            eigenvalues.push(lambda.max(0.0));

            if lambda.abs() > 1e-8 {
                // Hotelling rank-1 deflation: A' = A - λ·(v⊗v)
                let outer = Array2::from_shape_fn((n, n), |(i, j)| v[i] * v[j]);
                a = a - outer * lambda;
            }
        }

        Ok(eigenvalues)
    }

    /// Power iteration: returns (largest eigenvalue, corresponding eigenvector) of a symmetric matrix.
    fn power_iteration(&self, matrix: &Array2<f64>) -> Result<(f64, Array1<f64>)> {
        let n = matrix.nrows();
        if n == 0 { return Ok((0.0, Array1::zeros(0))); }

        let mut v = Array1::ones(n) / (n as f64).sqrt();
        let mut lambda = 0.0;
        const MAX_ITER: usize = 200;
        const TOLERANCE: f64 = 1e-8;

        for _ in 0..MAX_ITER {
            let av = matrix.dot(&v);
            let lambda_new = v.dot(&av);

            if (lambda_new - lambda).abs() < TOLERANCE {
                return Ok((lambda_new, v));
            }

            let norm = av.dot(&av).sqrt();
            if norm < 1e-12 { return Ok((0.0, v)); }

            v = av / norm;
            lambda = lambda_new;
        }

        Ok((lambda, v))
    }
}
