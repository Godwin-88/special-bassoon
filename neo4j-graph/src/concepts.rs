/// Finance concepts and their relationships
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Concept {
    pub name: String,
    pub definition: String,
    pub category: String,
    pub difficulty: String,
}
