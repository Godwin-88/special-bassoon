use solfest_core::{Portfolio, RLAction, ExecutionConstraints, StateComponents};
use anyhow::{Result, anyhow};

pub struct ConstraintChecker;

impl ConstraintChecker {
    pub fn new() -> Self {
        Self
    }

    /// Check if rebalancing is safe given Bayesian systemic risk scores
    pub fn check_before_execution(
        &self,
        portfolio: &Portfolio,
        action: &RLAction,
        constraints: &ExecutionConstraints,
        risk_components: &StateComponents,
    ) -> Result<()> {
        // 1. Slippage check
        if constraints.max_slippage_bps < 10 {
            return Err(anyhow!("Slippage constraint too tight"));
        }

        // 2. Systemic Risk check (Bayesian)
        // Ensure liquidity stress and contagion are not in critical regime
        if risk_components.social.liquidity_stress > 0.8 {
            return Err(anyhow!("Liquidity stress critical: {:.2}", risk_components.social.liquidity_stress));
        }

        if risk_components.social.contagion_index > 0.7 {
            return Err(anyhow!("Contagion risk critical: {:.2}", risk_components.social.contagion_index));
        }

        // 3. Protocol support check
        if !action.protocol.supported_chains().contains(&action.chain) {
            return Err(anyhow!("Protocol {:?} not supported on chain {:?}", action.protocol, action.chain));
        }

        Ok(())
    }

    pub fn check_after_execution(
        &self,
        portfolio: &Portfolio,
        _action: &RLAction,
        constraints: &ExecutionConstraints,
    ) -> Result<()> {
        // Check drawdown
        if portfolio.metrics.max_drawdown_pct > constraints.max_drawdown_pct {
            return Err(anyhow!("Max drawdown constraint violated: {:.2}% > {:.2}%", 
                portfolio.metrics.max_drawdown_pct, constraints.max_drawdown_pct));
        }

        Ok(())
    }
}
