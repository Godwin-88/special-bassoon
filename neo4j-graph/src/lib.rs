//! # Neo4j Graph Query Layer
//!
//! Handles all interactions with the Neo4j knowledge graph:
//! - Protocol relationships and risk models
//! - Strategy templates and prerequisites
//! - Cross-domain DeFi adaptations
//! - Graph embeddings for RL state construction

pub mod client;
pub mod protocol;
pub mod concepts;
pub mod strategies;
pub mod embeddings;
pub mod risk_graph;
pub mod evt;
pub mod garch;
pub mod bayesian_network;

pub use client::*;
pub use protocol::*;
pub use concepts::*;
pub use strategies::*;
pub use embeddings::*;
pub use risk_graph::*;
pub use evt::EVTEstimator;
pub use garch::GARCHModel;
pub use bayesian_network::{BayesianRiskNetwork, Evidence, NodeState};
