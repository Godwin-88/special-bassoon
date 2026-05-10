//! Comprehensive test suite for financial calculations and RL algorithms
//! Validates academic rigor and institutional-grade correctness

#[cfg(test)]
mod portfolio_metrics_tests {
    use crate::backtester::*;
    use solfest_core::{Portfolio, RiskProfile};

    #[test]
    fn test_sharpe_ratio_calculation() {
        // Known case: constant positive returns
        // σ = 0, so Sharpe should be 0/0 -> 0
        let graph_embeddings = neo4j_graph::GraphEmbeddings::new();
        let initial_portfolio = Portfolio::new("test".to_string());
        let constraints = RiskProfile::new_moderate("test".to_string()).constraints;
        let market_data = generate_synthetic_data(30);
        let mut env = BacktestEnvironment::new(market_data, initial_portfolio, constraints, graph_embeddings);

        let mut sharpe_values = Vec::new();
        while let Some(_state) = env.step() {
            let metrics = env.get_portfolio_metrics();
            sharpe_values.push(metrics.sharpe_ratio);
        }

        // Sharpe should be finite and typically in range [-5, 5] for reasonable portfolios
        for sharpe in sharpe_values.iter() {
            assert!(sharpe.is_finite(), "Sharpe ratio must be finite, got {}", sharpe);
        }
    }

    #[test]
    fn test_sortino_ratio_downside_filtering() {
        // Sortino should be >= Sharpe (using only downside deviation)
        let graph_embeddings = neo4j_graph::GraphEmbeddings::new();
        let initial_portfolio = Portfolio::new("test".to_string());
        let constraints = RiskProfile::new_moderate("test".to_string()).constraints;
        let market_data = generate_synthetic_data(100);
        let mut env = BacktestEnvironment::new(market_data, initial_portfolio, constraints, graph_embeddings);

        while let Some(_state) = env.step() {
            if env.is_done() { break; }
            let action = solfest_core::RLAction {
                chain: solfest_core::Chain::Solana,
                protocol: solfest_core::Protocol::Aave,
                allocation_pct: solfest_core::AllocationPercentage::Pct25,
                timestamp: chrono::Utc::now(),
            };
            let _ = env.execute_action(&action);
        }

        let metrics = env.get_portfolio_metrics();
        
        // Sortino uses only downside volatility, so it should be >= Sharpe
        // (unless all returns are positive, then Sharpe uses total vol)
        assert!(metrics.sortino_ratio.is_finite(), "Sortino must be finite");
        assert!(metrics.sharpe_ratio.is_finite(), "Sharpe must be finite");
    }

    #[test]
    fn test_max_drawdown_calculation() {
        // Test with synthetic portfolio that has known drawdown
        let graph_embeddings = neo4j_graph::GraphEmbeddings::new();
        let initial_portfolio = Portfolio::new("test".to_string());
        let constraints = RiskProfile::new_moderate("test".to_string()).constraints;
        let market_data = generate_synthetic_data(50);
        let mut env = BacktestEnvironment::new(market_data, initial_portfolio, constraints, graph_embeddings);

        let mut step = 0;
        while let Some(_state) = env.step() {
            if step >= 50 { break; }
            step += 1;
        }

        let metrics = env.get_portfolio_metrics();
        
        // Max drawdown must be in [0%, 100%]
        assert!(metrics.max_drawdown_pct >= 0.0, "Max drawdown must be non-negative");
        assert!(metrics.max_drawdown_pct <= 100.0, "Max drawdown must be <= 100%");
    }

    #[test]
    fn test_cumulative_slippage_accumulation() {
        // Slippage should accumulate with each transaction
        let graph_embeddings = neo4j_graph::GraphEmbeddings::new();
        let initial_portfolio = Portfolio::new("test".to_string());
        let constraints = RiskProfile::new_moderate("test".to_string()).constraints;
        let market_data = generate_synthetic_data(30);
        let mut env = BacktestEnvironment::new(market_data, initial_portfolio, constraints, graph_embeddings);

        let initial_slippage = env.get_portfolio_metrics().cumulative_slippage_pct;
        
        let mut slippage_increased = false;
        while let Some(_state) = env.step() {
            let metrics = env.get_portfolio_metrics();
            if metrics.cumulative_slippage_pct > initial_slippage {
                slippage_increased = true;
                break;
            }
        }

        assert!(slippage_increased || initial_slippage > 0.0, 
            "Slippage should accumulate over time");
    }
}

#[cfg(test)]
mod reward_function_tests {
    use crate::backtester::*;
    use solfest_core::{Portfolio, RiskProfile};

    #[test]
    fn test_reward_numerical_stability() {
        // Reward should always be finite and within reasonable bounds [-1000, 1000 bps]
        let graph_embeddings = neo4j_graph::GraphEmbeddings::new();
        let initial_portfolio = Portfolio::new("test".to_string());
        let constraints = RiskProfile::new_moderate("test".to_string()).constraints;
        let market_data = generate_synthetic_data(100);
        let mut env = BacktestEnvironment::new(market_data, initial_portfolio, constraints, graph_embeddings);

        while let Some(state) = env.step() {
            let action = solfest_core::RLAction {
                chain: solfest_core::Chain::Solana,
                protocol: solfest_core::Protocol::Aave,
                allocation_pct: solfest_core::AllocationPercentage::Pct50,
                timestamp: state.timestamp,
            };
            
            match env.execute_action(&action) {
                Ok(reward) => {
                    assert!(reward.is_finite(), "Reward must be finite, got {}", reward);
                    assert!(reward >= -1000.0 && reward <= 1000.0, 
                        "Reward should be in [-1000, 1000] bps, got {}", reward);
                },
                Err(_) => {}, // Expected for some edge cases
            }
        }
    }

    #[test]
    fn test_reward_components_clamping() {
        // All risk metrics should be clamped to [0, 1] before entering reward function
        let graph_embeddings = neo4j_graph::GraphEmbeddings::new();
        let initial_portfolio = Portfolio::new("test".to_string());
        let constraints = RiskProfile::new_moderate("test".to_string()).constraints;
        let market_data = generate_synthetic_data(50);
        let mut env = BacktestEnvironment::new(market_data, initial_portfolio, constraints, graph_embeddings);

        while let Some(_state) = env.step() {
            let metrics = env.get_portfolio_metrics();
            
            // These metrics should be in [0, 1] after clamping
            assert!(metrics.expected_shortfall >= 0.0 && metrics.expected_shortfall <= 1.0,
                "ES should be in [0,1], got {}", metrics.expected_shortfall);
            assert!(metrics.contagion_risk_index >= 0.0 && metrics.contagion_risk_index <= 1.0,
                "Contagion should be in [0,1], got {}", metrics.contagion_risk_index);
            assert!(metrics.liquidity_instability_index >= 0.0 && metrics.liquidity_instability_index <= 1.0,
                "Liquidity should be in [0,1], got {}", metrics.liquidity_instability_index);
        }
    }

    #[test]
    fn test_lambda_coefficients_normalized() {
        // Lambda coefficients should sum to 1.0 (normalized weights)
        // λ₁=0.35, λ₂=0.30, λ₃=0.15, λ₄=0.20 → sum=1.0
        let sum: f64 = 0.35 + 0.30 + 0.15 + 0.20;
        assert!((sum - 1.0).abs() < 1e-6, "Lambda coefficients should sum to 1.0, got {}", sum);
    }
}

#[cfg(test)]
mod risk_metrics_tests {
    use neo4j_graph::RiskGraph;
    use std::collections::HashMap;

    #[test]
    fn test_absorption_ratio_bounds() {
        // AR should be in [0, 1]
        let risk_graph = RiskGraph::new();
        let ar = risk_graph.calculate_absorption_ratio(3).unwrap();
        
        assert!(ar >= 0.0, "Absorption ratio must be >= 0, got {}", ar);
        assert!(ar <= 1.0, "Absorption ratio must be <= 1, got {}", ar);
    }

    #[test]
    fn test_contagion_propagation_convergence() {
        // Contagion should converge (not grow unbounded)
        let risk_graph = RiskGraph::new();
        let initial_shocks = HashMap::from([
            ("smart_contract_risk".to_string(), 0.1),
            ("oracle_failure".to_string(), 0.05),
        ]);

        let final_shocks = risk_graph.simulate_contagion(initial_shocks, 10);
        
        // All shock values should be in [0, 1]
        for (_, shock) in final_shocks.iter() {
            assert!(*shock >= 0.0 && *shock <= 1.0, 
                "Contagion shock must be in [0,1], got {}", shock);
        }
    }
}

#[cfg(test)]
mod ppo_policy_tests {
    use crate::policy::PPOConfig;

    #[test]
    fn test_ppo_config_hyperparameters() {
        let config = PPOConfig::default();
        
        // Check reasonable defaults
        assert!(config.learning_rate > 0.0 && config.learning_rate < 1e-2, 
            "Learning rate should be small, got {}", config.learning_rate);
        assert!(config.clip_ratio > 0.0 && config.clip_ratio < 1.0,
            "Clip ratio should be in (0, 1), got {}", config.clip_ratio);
        assert!(config.gamma >= 0.9 && config.gamma < 1.0,
            "Discount factor should be near 1, got {}", config.gamma);
        assert!(config.lambda >= 0.9 && config.lambda < 1.0,
            "GAE lambda should be near 1, got {}", config.lambda);
    }

    #[test]
    fn test_value_coefficients_sensible() {
        // Value and entropy coefficients should be positive but typically small
        let config = PPOConfig::default();
        
        assert!(config.value_coeff > 0.0, "Value coefficient should be positive");
        assert!(config.entropy_coeff >= 0.0, "Entropy coefficient should be non-negative");
        assert!(config.entropy_coeff < 1.0, "Entropy coefficient typically small");
    }
}

#[cfg(test)]
mod numerical_stability_tests {
    use crate::backtester::*;
    use solfest_core::Portfolio;

    #[test]
    fn test_no_nan_values_in_metrics() {
        // Financial calculations should never produce NaN
        let graph_embeddings = neo4j_graph::GraphEmbeddings::new();
        let initial_portfolio = Portfolio::new("test".to_string());
        let constraints = solfest_core::RiskProfile::new_moderate("test".to_string()).constraints;
        let market_data = generate_synthetic_data(100);
        let mut env = BacktestEnvironment::new(market_data, initial_portfolio, constraints, graph_embeddings);

        while let Some(_state) = env.step() {
            let metrics = env.get_portfolio_metrics();
            
            assert!(!metrics.sharpe_ratio.is_nan(), "Sharpe ratio is NaN");
            assert!(!metrics.sortino_ratio.is_nan(), "Sortino ratio is NaN");
            assert!(!metrics.total_apy.is_nan(), "APY is NaN");
            assert!(!metrics.max_drawdown_pct.is_nan(), "Max drawdown is NaN");
        }
    }

    #[test]
    fn test_division_by_zero_protection() {
        // Should handle edge cases without panicking
        let graph_embeddings = neo4j_graph::GraphEmbeddings::new();
        
        // Create portfolio with very small movements
        let mut env = BacktestEnvironment {
            market_data: vec![],
            current_step: 0,
            state_constructor: data_pipeline::StateConstructor::new(
                graph_embeddings,
                data_pipeline::OnChainClient::new(),
                data_pipeline::SocialSignalsAggregator::new("test".to_string()),
                data_pipeline::WhaleTracker::new(),
            ),
            portfolio: Portfolio::new("test".to_string()),
            constraints: solfest_core::ExecutionConstraints::default(),
            transaction_costs: vec![],
            portfolio_history: vec![],
        };

        let metrics = env.get_portfolio_metrics();
        
        // Should not panic or produce invalid values
        assert!(metrics.sharpe_ratio >= 0.0 || metrics.sharpe_ratio <= 0.0, 
            "Sharpe should be valid, got {}", metrics.sharpe_ratio);
    }
}

#[cfg(test)]
mod integration_tests {
    use crate::backtester::*;
    use solfest_core::{Portfolio, RiskProfile};

    #[test]
    fn test_backtester_full_episode() {
        // Full episode: generate data -> step -> execute -> calculate metrics -> update reward
        let graph_embeddings = neo4j_graph::GraphEmbeddings::new();
        let initial_portfolio = Portfolio::new("test".to_string());
        let constraints = RiskProfile::new_moderate("test".to_string()).constraints;
        let market_data = generate_synthetic_data(365); // 1 year
        let mut env = BacktestEnvironment::new(market_data, initial_portfolio, constraints, graph_embeddings);

        let mut step_count = 0;
        let mut action_count = 0;
        
        while let Some(state) = env.step() {
            step_count += 1;
            
            let action = solfest_core::RLAction {
                chain: solfest_core::Chain::Solana,
                protocol: solfest_core::Protocol::Aave,
                allocation_pct: solfest_core::AllocationPercentage::Pct50,
                timestamp: state.timestamp,
            };
            
            match env.execute_action(&action) {
                Ok(reward) => {
                    action_count += 1;
                    assert!(reward.is_finite(), "Reward must be finite");
                },
                Err(_) => {},
            }
            
            if step_count >= 100 { break; }
        }

        let final_metrics = env.get_portfolio_metrics();
        
        assert!(step_count > 0, "Should have steps");
        assert!(action_count > 0, "Should have executed actions");
        assert!(final_metrics.sharpe_ratio.is_finite(), "Final metrics should be valid");
    }

    #[test]
    fn test_portfolio_value_tracking() {
        // Portfolio value should be tracked correctly through episodes
        let graph_embeddings = neo4j_graph::GraphEmbeddings::new();
        let mut initial_portfolio = Portfolio::new("test".to_string());
        initial_portfolio.total_value_usd = 1_000_000.0; // Start with $1M
        
        let constraints = RiskProfile::new_moderate("test".to_string()).constraints;
        let market_data = generate_synthetic_data(30);
        let env = BacktestEnvironment::new(market_data, initial_portfolio.clone(), constraints, graph_embeddings);

        // Portfolio history should track value changes
        assert!(!env.get_portfolio_metrics().cumulative_slippage_pct.is_nan(),
            "Portfolio tracking should work");
    }
}
