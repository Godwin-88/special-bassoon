use solfest_core::{Portfolio, RLAction, ExecutionConstraints, StateComponents};
use crate::portfolio_rebalancer::PortfolioRebalancer;
use crate::constraint_checker::ConstraintChecker;
use anyhow::Result;

pub struct ExecutionEngine {
    rebalancer: PortfolioRebalancer,
    checker: ConstraintChecker,
}

impl ExecutionEngine {
    pub fn new() -> Self {
        Self {
            rebalancer: PortfolioRebalancer::new(),
            checker: ConstraintChecker::new(),
        }
    }

    /// Execute a rebalancing action with safety checks
    pub async fn execute_rebalance(
        &self,
        portfolio: &mut Portfolio,
        action: &RLAction,
        constraints: &ExecutionConstraints,
        risk_components: &StateComponents,
    ) -> Result<()> {
        // 1. Pre-execution constraint check
        self.checker.check_before_execution(portfolio, action, constraints, risk_components)?;

        // 2. Perform rebalancing
        self.rebalancer.rebalance(portfolio, action).await?;

        // 3. Post-execution validation
        self.checker.check_after_execution(portfolio, action, constraints)?;

        Ok(())
    }
}
