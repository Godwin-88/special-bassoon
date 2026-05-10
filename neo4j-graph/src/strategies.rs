/// Trading strategies and their prerequisites
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Strategy {
    pub name: String,
    pub category: String,
    pub prerequisites: Vec<String>,
}
