//! # Bridge Capital Markets Agent - Core Types
//!
//! Defines fundamental data structures for portfolio management, RL state/action spaces,
//! and constraint enforcement.

pub mod portfolio;
pub mod state;
pub mod action;
pub mod constraints;
pub mod errors;

pub use portfolio::*;
pub use state::*;
pub use action::*;
pub use constraints::*;
pub use errors::*;
