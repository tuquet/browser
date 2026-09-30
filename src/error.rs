use thiserror::Error;

#[derive(Error, Debug)]
pub enum BrowserError {
    #[error("Browser runtime not found: {0}")]
    NotFound(String),

    #[error("Failed to download browser runtime: {0}")]
    DownloadError(String),

    #[error("Failed to unpack browser archive: {0}")]
    ExtractionError(String),

    #[error("Failed to spawn browser process: {0}")]
    ProcessSpawnError(String),

    #[error("CDP connection failed: {0}")]
    CdpConnectionError(String),

    #[error("Timeout waiting for browser: {0}")]
    Timeout(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Generic browser error: {0}")]
    Other(String),
}
