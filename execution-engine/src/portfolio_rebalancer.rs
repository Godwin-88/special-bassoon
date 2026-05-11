use solfest_core::{Portfolio, RLAction, Allocation};
use crate::bridge_router::BridgeRouter;
use anyhow::Result;
use chrono::Utc;
use tracing::info;
use uuid::Uuid;

pub struct PortfolioRebalancer {
    bridge_router: BridgeRouter,
}

impl PortfolioRebalancer {
    pub fn new() -> Self {
        Self {
            bridge_router: BridgeRouter::new(),
        }
    }

    /// Rebalance portfolio based on RL action
    pub async fn rebalance(&self, portfolio: &mut Portfolio, action: &RLAction) -> Result<()> {
        let target_pct = action.allocation_pct.as_float();
        let target_amount = portfolio.total_value_usd * target_pct;
        
        // Find existing allocation for this protocol/chain if any
        let existing_idx = portfolio.allocations.iter().position(|a| 
            a.chain == action.chain.as_str() && a.protocol == action.protocol.as_str()
        );

        if let Some(idx) = existing_idx {
            // Update existing allocation
            let allocation = &mut portfolio.allocations[idx];
            allocation.percentage = target_pct;
            allocation.amount_usd = target_amount;
            allocation.current_value_usd = target_amount;
        } else {
            // Create new allocation
            let new_allocation = Allocation {
                id: Uuid::new_v4().to_string(),
                chain: action.chain.as_str().to_string(),
                protocol: action.protocol.as_str().to_string(),
                pool_id: None,
                strategy: "auto_yield".to_string(),
                amount_usd: target_amount,
                percentage: target_pct,
                entry_price: 1.0,
                entry_time: Utc::now(),
                current_value_usd: target_amount,
                unrealized_pnl_usd: 0.0,
                apy: 0.0,
                risk_score: 0.0,
            };
            portfolio.allocations.push(new_allocation);
        }

        // Identify cross-chain needs (simplified: assume funding comes from non-target allocations)
        // In reality, we'd calculate net flows between all protocols
        if !portfolio.allocations.is_empty() {
            let source = portfolio.allocations[0].chain.as_str();
            if source != action.chain.as_str() {
                if let Ok(route) = self.bridge_router.find_best_route(source, action.chain.as_str(), target_amount).await {
                    info!("Estimated bridge fee {} via {}", route.estimated_fee_usd, route.bridge_name);
                }
            }
        }
        
        portfolio.last_rebalanced = Some(Utc::now());
        Ok(())
    }
}
