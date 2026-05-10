use serde::{Deserialize, Serialize};

/// Risk constraints to enforce during rebalancing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionConstraints {
    /// Maximum slippage allowed per trade (in basis points, e.g., 50 = 0.5%)
    pub max_slippage_bps: u32,

    /// Minimum liquidity required to execute trade (USD)
    pub min_liquidity_usd: f64,

    /// Maximum gas cost as % of trade amount (e.g., 0.05 = 0.05%)
    pub max_gas_cost_pct: f64,

    /// Maximum bridge latency allowed (seconds)
    pub max_bridge_latency_sec: u32,

    /// Maximum portfolio drawdown from peak (%, e.g., 10.0)
    pub max_drawdown_pct: f64,

    /// Maximum exposure to single protocol (%, e.g., 50.0)
    pub max_protocol_exposure_pct: f64,

    /// Maximum exposure to single chain (%, e.g., 70.0)
    pub max_chain_exposure_pct: f64,

    /// Maximum leverage (1.0 = no leverage, >1.0 = leveraged)
    pub max_leverage: f64,

    /// Minimum rebalance interval (hours)
    pub min_rebalance_interval_hours: u32,
}

impl Default for ExecutionConstraints {
    fn default() -> Self {
        Self {
            max_slippage_bps: 50,                  // 0.5%
            min_liquidity_usd: 10_000_000.0,       // $10M minimum
            max_gas_cost_pct: 0.05,                // 0.05% of trade
            max_bridge_latency_sec: 300,           // 5 minutes
            max_drawdown_pct: 10.0,                // 10% max drawdown
            max_protocol_exposure_pct: 50.0,       // 50% per protocol
            max_chain_exposure_pct: 70.0,          // 70% per chain
            max_leverage: 1.0,                     // No leverage by default
            min_rebalance_interval_hours: 24,      // Daily rebalancing
        }
    }
}

/// Conservative constraints (lower risk)
impl ExecutionConstraints {
    pub fn conservative() -> Self {
        Self {
            max_slippage_bps: 25,                  // 0.25%
            min_liquidity_usd: 20_000_000.0,       // $20M
            max_gas_cost_pct: 0.03,
            max_bridge_latency_sec: 180,
            max_drawdown_pct: 5.0,
            max_protocol_exposure_pct: 30.0,
            max_chain_exposure_pct: 50.0,
            max_leverage: 1.0,
            min_rebalance_interval_hours: 24,
        }
    }

    pub fn aggressive() -> Self {
        Self {
            max_slippage_bps: 100,                 // 1.0%
            min_liquidity_usd: 5_000_000.0,        // $5M
            max_gas_cost_pct: 0.1,
            max_bridge_latency_sec: 600,           // 10 minutes
            max_drawdown_pct: 20.0,
            max_protocol_exposure_pct: 70.0,
            max_chain_exposure_pct: 90.0,
            max_leverage: 1.5,
            min_rebalance_interval_hours: 12,
        }
    }
}

/// Result of constraint validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstraintViolation {
    pub constraint_name: String,
    pub current_value: f64,
    pub max_allowed: f64,
    pub violation_pct: f64,
}

/// Validation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub violations: Vec<ConstraintViolation>,
    pub passed_checks: usize,
}

impl ValidationResult {
    pub fn new() -> Self {
        Self {
            is_valid: true,
            violations: vec![],
            passed_checks: 0,
        }
    }

    pub fn add_violation(&mut self, violation: ConstraintViolation) {
        self.is_valid = false;
        self.violations.push(violation);
    }

    pub fn mark_passed(&mut self) {
        self.passed_checks += 1;
    }
}

/// User risk profile (customizable constraints)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskProfile {
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub description: Option<String>,
    pub risk_level: RiskLevel,                   // basic, intermediate, advanced
    pub constraints: ExecutionConstraints,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum RiskLevel {
    Conservative,
    Moderate,
    Aggressive,
}

impl RiskProfile {
    pub fn new_conservative(user_id: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            user_id,
            name: "Conservative".to_string(),
            description: Some("Low risk, focus on capital preservation".to_string()),
            risk_level: RiskLevel::Conservative,
            constraints: ExecutionConstraints::conservative(),
        }
    }

    pub fn new_moderate(user_id: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            user_id,
            name: "Moderate".to_string(),
            description: Some("Balanced risk/return".to_string()),
            risk_level: RiskLevel::Moderate,
            constraints: ExecutionConstraints::default(),
        }
    }

    pub fn new_aggressive(user_id: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            user_id,
            name: "Aggressive".to_string(),
            description: Some("Higher risk for higher returns".to_string()),
            risk_level: RiskLevel::Aggressive,
            constraints: ExecutionConstraints::aggressive(),
        }
    }

    pub fn custom(user_id: String, constraints: ExecutionConstraints) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            user_id,
            name: "Custom".to_string(),
            description: None,
            risk_level: RiskLevel::Moderate,
            constraints,
        }
    }
}
