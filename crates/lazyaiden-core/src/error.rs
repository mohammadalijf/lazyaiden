use std::path::PathBuf;

use fellow_client::FellowError;

/// Result alias for this crate.
pub type Result<T, E = CoreError> = std::result::Result<T, E>;

/// Failures of configuration, credentials and device handling.
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    /// The config file is unreadable or invalid.
    #[error("config {}: {message}", path.display())]
    Config {
        /// The config file.
        path: PathBuf,
        /// What went wrong.
        message: String,
    },
    /// The credential store failed.
    #[error("credential store: {0}")]
    Credentials(String),
    /// The brewer API failed.
    #[error(transparent)]
    Fellow(#[from] FellowError),
    /// No brewer matches.
    #[error("no brewer matches {0:?}")]
    NotFound(String),
    /// A brewer name matches several devices.
    #[error("{query:?} is ambiguous; matches: {}", matches.join(", "))]
    Ambiguous {
        /// What the user typed.
        query: String,
        /// Matching brewer ids.
        matches: Vec<String>,
    },
}
