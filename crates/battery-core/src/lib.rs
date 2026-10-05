//! Pure prediction and control logic, durable recovery, and isolated hardware adapters.
pub mod controller;
pub mod engine;
pub mod estimator;
pub mod locale;
pub mod model;
pub mod platform;
pub mod service;
pub mod simulator;
pub mod storage;

pub use engine::Engine;
pub use model::*;
pub use platform::Hardware;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Hardware(String),
    #[error("{0}")]
    InvalidInput(String),
    #[error("Another BatteryDeadline instance is using this data folder.")]
    AlreadyRunning,
    #[error("Local storage could not be accessed: {0}")]
    Storage(#[from] rusqlite::Error),
    #[error("Local files could not be accessed: {0}")]
    Io(#[from] std::io::Error),
    #[error("Stored data could not be read: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Diagnostics archive could not be created: {0}")]
    Zip(#[from] zip::result::ZipError),
}

pub type Result<T> = std::result::Result<T, Error>;
