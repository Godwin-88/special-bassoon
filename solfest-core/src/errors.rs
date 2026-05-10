use thiserror::Error;

#[derive(Error, Debug)]
pub enum BridgeError {
    #[error("Neo4j graph error: {0}")]
    GraphError(String),

    #[error("Data pipeline error: {0}")]
    DataPipelineError(String),

    #[error("RL agent error: {0}")]
    RLAgentError(String),

    #[error("Execution error: {0}")]
    ExecutionError(String),

    #[error("Constraint violation: {0}")]
    ConstraintViolation(String),

    #[error("Slippage exceeded: expected {expected}%, got {actual}%")]
    SlippageExceeded { expected: f64, actual: f64 },

    #[error("Insufficient liquidity: required {required}, available {available}")]
    InsufficientLiquidity { required: f64, available: f64 },

    #[error("Bridge routing failed: {0}")]
    BridgeRoutingFailed(String),

    #[error("Portfolio error: {0}")]
    PortfolioError(String),

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("API error: {0}")]
    ApiError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Invalid action: {0}")]
    InvalidAction(String),

    #[error("Unknown error: {0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, BridgeError>;
