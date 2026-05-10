/// Protocol-specific operations and metadata
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Protocol {
    pub name: String,
    pub category: String,
    pub invariant: String,
    pub fee_tiers: Vec<String>,
}
