use std::io;

use thiserror::Error;

pub type Result<T> = std::result::Result<T, SignalLightError>;

#[derive(Debug, Error)]
pub enum SignalLightError {
    #[error("{0}")]
    InvalidUsage(String),
    #[error("{0}")]
    InvalidSignal(String),
    #[error("{0}")]
    InvalidRequest(String),
    #[error("{0}")]
    Configuration(String),
    #[error("{0}")]
    Runtime(String),
    #[error("{0}")]
    Timeout(String),
    #[error("{0}")]
    Hardware(String),
    #[error("{0}")]
    Protocol(String),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

impl SignalLightError {
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::InvalidUsage(_)
            | Self::InvalidSignal(_)
            | Self::InvalidRequest(_)
            | Self::Configuration(_) => 2,
            Self::Runtime(_)
            | Self::Timeout(_)
            | Self::Hardware(_)
            | Self::Protocol(_)
            | Self::Io(_)
            | Self::Json(_) => 1,
        }
    }
}
