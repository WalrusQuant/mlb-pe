#[derive(Debug, thiserror::Error)]
pub enum PnsError {
    #[error("data error: {0}")]
    Data(String),
    #[error("normalization error: {0}")]
    Normalize(String),
    #[error("feature error: {0}")]
    Feature(String),
    #[error("model error: {0}")]
    Model(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("CSV error: {0}")]
    Csv(#[from] csv::Error),
    #[error("fixture error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("MLB data request failed: {0}")]
    Http(#[from] reqwest::Error),
}

pub type Result<T> = std::result::Result<T, PnsError>;
