/// Graph embeddings for state construction — uses feature hashing + spectral projection.
/// SVD replaced with power-iteration based dimensionality reduction (no LAPACK required).
use ndarray::{s, Array1, Array2};
use std::collections::HashMap;
use crate::client::GraphClient;
use anyhow::Result;
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone)]
pub struct GraphEmbeddings {
    /// Protocol similarity embeddings (protocol_name -> 64-dim vector)
    pub protocol_embeddings: HashMap<String, Array1<f64>>,
    /// Risk correlation embeddings
    pub risk_embeddings: HashMap<String, Array1<f64>>,
}

impl GraphEmbeddings {
    pub fn new() -> Self {
        Self {
            protocol_embeddings: HashMap::new(),
            risk_embeddings: HashMap::new(),
        }
    }

    /// Compute embeddings for all protocols using spectral projection on the
    /// protocol-concept incidence matrix. Falls back to feature hashing when
    /// the graph has insufficient data (< 2 protocols or concepts).
    pub async fn compute_embeddings(&mut self, client: &GraphClient) -> Result<()> {
        let mapping = client.get_protocol_concept_mapping().await?;

        let mut protocols: Vec<String> = mapping.iter().map(|(p, _)| p.clone()).collect();
        protocols.sort(); protocols.dedup();
        let mut concepts: Vec<String> = mapping.iter().map(|(_, c)| c.clone()).collect();
        concepts.sort(); concepts.dedup();

        if protocols.len() < 2 || concepts.is_empty() {
            return self.compute_fallback_embeddings(client).await;
        }

        let n_p = protocols.len();
        let n_c = concepts.len();
        let mut incidence = Array2::<f64>::zeros((n_p, n_c));

        let p_idx: HashMap<&String, usize> = protocols.iter().enumerate().map(|(i,p)|(p,i)).collect();
        let c_idx: HashMap<&String, usize> = concepts.iter().enumerate().map(|(i,c)|(c,i)).collect();
        for (proto, concept) in &mapping {
            if let (Some(&pi), Some(&ci)) = (p_idx.get(proto), c_idx.get(concept)) {
                incidence[[pi, ci]] = 1.0;
            }
        }

        // Similarity matrix: S = M * M^T  (n_p × n_p)
        let similarity = incidence.dot(&incidence.t());

        // Spectral embedding: extract top-64 eigenvectors via power iteration + deflation
        let n_components = 64.min(n_p);
        let embeddings = spectral_embed(&similarity, n_components);

        self.protocol_embeddings.clear();
        for (i, proto) in protocols.iter().enumerate() {
            let vec = embeddings.row(i).to_owned();
            self.protocol_embeddings.insert(proto.clone(), vec);
        }

        // Risk embeddings via feature hashing
        let risk_mapping = client.get_concept_categories().await?;
        let mut risk_to_features: HashMap<String, Vec<String>> = HashMap::new();
        for (name, category) in risk_mapping {
            if category.to_lowercase().contains("risk") {
                risk_to_features.entry(name.clone()).or_default().push(category);
                risk_to_features.entry(name).or_default().push("risk".to_string());
            }
        }
        for (risk, features) in risk_to_features {
            self.risk_embeddings.insert(risk, self.generate_hashed_embedding(&features, 64));
        }

        Ok(())
    }

    async fn compute_fallback_embeddings(&mut self, client: &GraphClient) -> Result<()> {
        let mapping = client.get_protocol_concept_mapping().await?;
        let mut p2c: HashMap<String, Vec<String>> = HashMap::new();
        for (proto, concept) in mapping {
            p2c.entry(proto).or_default().push(concept);
        }
        self.protocol_embeddings.clear();
        for (proto, concepts) in p2c {
            self.protocol_embeddings.insert(proto, self.generate_hashed_embedding(&concepts, 64));
        }

        let risk_mapping = client.get_concept_categories().await?;
        let mut risk_to_features: HashMap<String, Vec<String>> = HashMap::new();
        for (name, category) in risk_mapping {
            if category.to_lowercase().contains("risk") {
                risk_to_features.entry(name.clone()).or_default().push(category);
                risk_to_features.entry(name).or_default().push("risk".to_string());
            }
        }
        for (risk, features) in risk_to_features {
            self.risk_embeddings.insert(risk, self.generate_hashed_embedding(&features, 64));
        }
        Ok(())
    }

    fn generate_hashed_embedding(&self, features: &[String], dims: usize) -> Array1<f64> {
        let mut v = Array1::<f64>::zeros(dims);
        for feature in features {
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            feature.hash(&mut hasher);
            let hash = hasher.finish();
            let idx = (hash as usize) % dims;
            let sign = if (hash >> 32) % 2 == 0 { 1.0 } else { -1.0 };
            v[idx] += sign;
        }
        let norm = v.dot(&v).sqrt();
        if norm > 0.0 { v.mapv_inplace(|x| x / norm); }
        v
    }

    pub fn get_protocol_embedding(&self, protocol_name: &str) -> Option<Array1<f64>> {
        self.protocol_embeddings.get(protocol_name).cloned()
    }

    /// Load previously computed embeddings from a JSON file written by `embed_data`.
    pub fn load_from_file(path: &str) -> Result<Self> {
        let raw = std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("Cannot read {}: {}", path, e))?;
        let val: serde_json::Value = serde_json::from_str(&raw)?;

        let mut proto_embs = HashMap::new();
        if let Some(obj) = val.get("protocol_embeddings").and_then(|v| v.as_object()) {
            for (k, v) in obj {
                let vec: Vec<f64> = serde_json::from_value(v.clone())?;
                proto_embs.insert(k.clone(), Array1::from_vec(vec));
            }
        }

        let mut risk_embs = HashMap::new();
        if let Some(obj) = val.get("risk_embeddings").and_then(|v| v.as_object()) {
            for (k, v) in obj {
                let vec: Vec<f64> = serde_json::from_value(v.clone())?;
                risk_embs.insert(k.clone(), Array1::from_vec(vec));
            }
        }

        Ok(Self {
            protocol_embeddings: proto_embs,
            risk_embeddings: risk_embs,
        })
    }
}

/// Extract top-k eigenvectors of a symmetric matrix via power iteration + Hotelling deflation.
/// Returns an (n × k) matrix where each column is a scaled eigenvector.
fn spectral_embed(matrix: &Array2<f64>, k: usize) -> Array2<f64> {
    let n = matrix.nrows();
    let k = k.min(n);
    let mut result = Array2::<f64>::zeros((n, k));
    let mut a = matrix.clone();

    for col in 0..k {
        let (lambda, v) = power_iteration(&a);
        if lambda.abs() < 1e-10 { break; }

        // Store scaled eigenvector as embedding column
        let scaled = v.mapv(|x| x * lambda.sqrt().max(0.0));
        result.column_mut(col).assign(&scaled);

        // Hotelling deflation: A' = A - λ·(v⊗v)
        let outer = Array2::from_shape_fn((n, n), |(i, j)| v[i] * v[j]);
        a = a - outer * lambda;
    }
    result
}

fn power_iteration(matrix: &Array2<f64>) -> (f64, Array1<f64>) {
    let n = matrix.nrows();
    if n == 0 { return (0.0, Array1::zeros(0)); }
    let mut v = Array1::<f64>::ones(n) / (n as f64).sqrt();
    let mut lambda = 0.0;
    for _ in 0..200 {
        let av = matrix.dot(&v);
        let lnew = v.dot(&av);
        if (lnew - lambda).abs() < 1e-8 { return (lnew, v); }
        let norm = av.dot(&av).sqrt();
        if norm < 1e-12 { return (0.0, v); }
        v = av / norm;
        lambda = lnew;
    }
    (lambda, v)
}
