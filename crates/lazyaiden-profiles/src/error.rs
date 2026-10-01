use std::path::PathBuf;

use fellow_client::{FellowError, ValidationIssue};

/// Result alias for the service layer.
pub type Result<T, E = ProfileError> = std::result::Result<T, E>;

/// Problems reading or writing the local store.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// Filesystem failure.
    #[error("{}: {source}", path.display())]
    Io {
        /// File or directory involved.
        path: PathBuf,
        /// Underlying error.
        source: std::io::Error,
    },
    /// A YAML file could not be parsed.
    #[error("{}: {message}", path.display())]
    Parse {
        /// The malformed file.
        path: PathBuf,
        /// Parser message including line and column when known.
        message: String,
    },
    /// No local profile has this name.
    #[error("no local profile named {0:?}")]
    NotFound(String),
    /// The name cannot be used as a file name.
    #[error(
        "invalid profile name {0:?}: use letters, digits, '-', '_' and '.', not starting with '.'"
    )]
    InvalidName(String),
}

/// Everything [`ProfileService`](crate::ProfileService) can fail with.
#[derive(Debug, thiserror::Error)]
pub enum ProfileError {
    /// The brewer API failed.
    #[error(transparent)]
    Fellow(#[from] FellowError),
    /// The local store failed.
    #[error(transparent)]
    Store(#[from] StoreError),
    /// The profile data is not valid for the brewer.
    #[error("invalid profile: {}", .0.iter().map(ToString::to_string).collect::<Vec<_>>().join("; "))]
    Validation(Vec<ValidationIssue>),
    /// No local or remote profile matches.
    #[error("no profile matches {0:?}")]
    NotFound(String),
    /// A title matches several profiles; use the file name instead.
    #[error("{query:?} is ambiguous; matches: {}", matches.join(", "))]
    Ambiguous {
        /// What the user typed.
        query: String,
        /// Names of the matching profiles.
        matches: Vec<String>,
    },
    /// The operation needs the profile to exist on the brewer first.
    #[error("profile {0:?} has not been pushed to this brewer yet")]
    NotPushed(String),
    /// A local profile with this name or title already exists.
    #[error("{0}")]
    Conflict(String),
    /// Imported text is not a valid profile document.
    #[error("cannot parse profile: {0}")]
    Parse(String),
}
