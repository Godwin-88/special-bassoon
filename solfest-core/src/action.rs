use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// Action space: Discrete actions from RL agent
/// Total actions: chain (4) × protocol (8) × allocation_pct (20) = ~640 actions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct RLAction {
    pub chain: Chain,
    pub protocol: Protocol,
    pub allocation_pct: AllocationPercentage,     // 5%, 10%, ..., 100%
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Chain {
    Solana,
    Ethereum,
    Base,
    Arbitrum,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Protocol {
    // DEXs
    UniswapV3,
    UniswapV2,
    Curve,
    Balancer,
    
    // Lending
    Aave,
    Compound,
    
    // Stablecoins/Staking
    MakerDAO,
    Lido,
    
    // Derivatives
    Dydx,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AllocationPercentage {
    Pct5,
    Pct10,
    Pct15,
    Pct20,
    Pct25,
    Pct30,
    Pct35,
    Pct40,
    Pct45,
    Pct50,
    Pct55,
    Pct60,
    Pct65,
    Pct70,
    Pct75,
    Pct80,
    Pct85,
    Pct90,
    Pct95,
    Pct100,
}

impl AllocationPercentage {
    pub fn as_float(&self) -> f64 {
        match self {
            Self::Pct5 => 0.05,
            Self::Pct10 => 0.10,
            Self::Pct15 => 0.15,
            Self::Pct20 => 0.20,
            Self::Pct25 => 0.25,
            Self::Pct30 => 0.30,
            Self::Pct35 => 0.35,
            Self::Pct40 => 0.40,
            Self::Pct45 => 0.45,
            Self::Pct50 => 0.50,
            Self::Pct55 => 0.55,
            Self::Pct60 => 0.60,
            Self::Pct65 => 0.65,
            Self::Pct70 => 0.70,
            Self::Pct75 => 0.75,
            Self::Pct80 => 0.80,
            Self::Pct85 => 0.85,
            Self::Pct90 => 0.90,
            Self::Pct95 => 0.95,
            Self::Pct100 => 1.00,
        }
    }

    pub fn from_float(f: f64) -> Option<Self> {
        match (f * 100.0).round() as i32 {
            5 => Some(Self::Pct5),
            10 => Some(Self::Pct10),
            15 => Some(Self::Pct15),
            20 => Some(Self::Pct20),
            25 => Some(Self::Pct25),
            30 => Some(Self::Pct30),
            35 => Some(Self::Pct35),
            40 => Some(Self::Pct40),
            45 => Some(Self::Pct45),
            50 => Some(Self::Pct50),
            55 => Some(Self::Pct55),
            60 => Some(Self::Pct60),
            65 => Some(Self::Pct65),
            70 => Some(Self::Pct70),
            75 => Some(Self::Pct75),
            80 => Some(Self::Pct80),
            85 => Some(Self::Pct85),
            90 => Some(Self::Pct90),
            95 => Some(Self::Pct95),
            100 => Some(Self::Pct100),
            _ => None,
        }
    }

    pub fn all() -> Vec<Self> {
        vec![
            Self::Pct5, Self::Pct10, Self::Pct15, Self::Pct20, Self::Pct25,
            Self::Pct30, Self::Pct35, Self::Pct40, Self::Pct45, Self::Pct50,
            Self::Pct55, Self::Pct60, Self::Pct65, Self::Pct70, Self::Pct75,
            Self::Pct80, Self::Pct85, Self::Pct90, Self::Pct95, Self::Pct100,
        ]
    }
}

impl Chain {
    pub fn all() -> Vec<Self> {
        vec![Self::Solana, Self::Ethereum, Self::Base, Self::Arbitrum]
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Solana => "solana",
            Self::Ethereum => "ethereum",
            Self::Base => "base",
            Self::Arbitrum => "arbitrum",
        }
    }
}

impl Protocol {
    pub fn all() -> Vec<Self> {
        vec![
            Self::UniswapV3, Self::UniswapV2, Self::Curve, Self::Balancer,
            Self::Aave, Self::Compound, Self::MakerDAO, Self::Lido, Self::Dydx,
        ]
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::UniswapV3 => "uniswap_v3",
            Self::UniswapV2 => "uniswap_v2",
            Self::Curve => "curve",
            Self::Balancer => "balancer",
            Self::Aave => "aave",
            Self::Compound => "compound",
            Self::MakerDAO => "makerdao",
            Self::Lido => "lido",
            Self::Dydx => "dydx",
        }
    }

    pub fn supported_chains(&self) -> Vec<Chain> {
        match self {
            Self::UniswapV3 => vec![Chain::Ethereum, Chain::Base, Chain::Arbitrum],
            Self::UniswapV2 => vec![Chain::Ethereum],
            Self::Curve => vec![Chain::Ethereum, Chain::Arbitrum],
            Self::Balancer => vec![Chain::Ethereum, Chain::Arbitrum],
            Self::Aave => vec![Chain::Ethereum, Chain::Arbitrum],
            Self::Compound => vec![Chain::Ethereum],
            Self::MakerDAO => vec![Chain::Ethereum],
            Self::Lido => vec![Chain::Ethereum],
            Self::Dydx => vec![Chain::Arbitrum],
        }
    }
}

impl RLAction {
    pub fn to_action_index(&self) -> usize {
        let chain_idx = match self.chain {
            Chain::Solana => 0,
            Chain::Ethereum => 1,
            Chain::Base => 2,
            Chain::Arbitrum => 3,
        };
        
        let protocol_idx = match self.protocol {
            Protocol::UniswapV3 => 0,
            Protocol::UniswapV2 => 1,
            Protocol::Curve => 2,
            Protocol::Balancer => 3,
            Protocol::Aave => 4,
            Protocol::Compound => 5,
            Protocol::MakerDAO => 6,
            Protocol::Lido => 7,
            Protocol::Dydx => 8,
        };

        let allocation_idx = self.allocation_pct as usize;

        // Total action space: 4 chains × 9 protocols × 20 allocations = 720 actions
        chain_idx * (9 * 20) + protocol_idx * 20 + allocation_idx
    }

    pub fn from_action_index(idx: usize) -> Option<Self> {
        if idx >= 720 {
            return None;
        }

        let chain_idx = idx / (9 * 20);
        let remainder = idx % (9 * 20);
        let protocol_idx = remainder / 20;
        let allocation_idx = remainder % 20;

        let chain = match chain_idx {
            0 => Chain::Solana,
            1 => Chain::Ethereum,
            2 => Chain::Base,
            3 => Chain::Arbitrum,
            _ => return None,
        };

        let protocol = match protocol_idx {
            0 => Protocol::UniswapV3,
            1 => Protocol::UniswapV2,
            2 => Protocol::Curve,
            3 => Protocol::Balancer,
            4 => Protocol::Aave,
            5 => Protocol::Compound,
            6 => Protocol::MakerDAO,
            7 => Protocol::Lido,
            8 => Protocol::Dydx,
            _ => return None,
        };

        let allocation_pct = AllocationPercentage::all()[allocation_idx];

        Some(Self {
            chain,
            protocol,
            allocation_pct,
            timestamp: Utc::now(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_indexing() {
        let action = RLAction {
            chain: Chain::Ethereum,
            protocol: Protocol::Aave,
            allocation_pct: AllocationPercentage::Pct50,
            timestamp: Utc::now(),
        };

        let idx = action.to_action_index();
        let reconstructed = RLAction::from_action_index(idx).unwrap();
        
        assert_eq!(reconstructed.chain, action.chain);
        assert_eq!(reconstructed.protocol, action.protocol);
        assert_eq!(reconstructed.allocation_pct, action.allocation_pct);
    }

    #[test]
    fn test_allocation_percentage_roundtrip() {
        let pct = AllocationPercentage::Pct75;
        let float = pct.as_float();
        let reconstructed = AllocationPercentage::from_float(float).unwrap();
        assert_eq!(pct, reconstructed);
    }
}
