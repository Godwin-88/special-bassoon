//! # Data Pipeline
//!
//! Ingests on-chain data from QuickNode, social signals, and whale movements.
//! Constructs state vectors for RL agent.

pub mod on_chain;
pub mod social_signals;
pub mod whale_tracking;
pub mod state_constructor;
pub mod protocol_ingester;

pub use on_chain::*;
pub use social_signals::*;
pub use whale_tracking::*;
pub use state_constructor::*;
pub use protocol_ingester::ProtocolIngester;
