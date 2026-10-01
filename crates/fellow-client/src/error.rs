use std::fmt;

use crate::models::Device;

/// Convenience alias used throughout the crate.
pub type Result<T, E = FellowError> = std::result::Result<T, E>;

/// A single problem found while validating user-supplied data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationIssue {
    /// The offending field, using the API's camelCase name.
    pub field: String,
    /// Human-readable explanation including the allowed range.
    pub message: String,
}

impl ValidationIssue {
    /// Creates an issue for `field`.
    pub fn new(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            message: message.into(),
        }
    }
}

impl fmt::Display for ValidationIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.field, self.message)
    }
}

fn join_issues(issues: &[ValidationIssue]) -> String {
    issues
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("; ")
}

fn describe_devices(devices: &[Device]) -> String {
    devices
        .iter()
        .map(Device::label)
        .collect::<Vec<_>>()
        .join(", ")
}

/// Everything that can go wrong when talking to Fellow.
#[derive(Debug, thiserror::Error)]
pub enum FellowError {
    /// Login failed or the token could not be refreshed.
    #[error("authentication failed: {0}")]
    Auth(String),
    /// The server answered with an unexpected non-success status.
    #[error("API error {status}: {body}")]
    Api {
        /// HTTP status code.
        status: u16,
        /// Raw response body.
        body: String,
    },
    /// Local validation rejected the data before it was sent.
    #[error("validation failed: {}", join_issues(.0))]
    Validation(Vec<ValidationIssue>),
    /// The request never produced a response (DNS, TLS, timeout, ...).
    #[error("network error: {0}")]
    Network(String),
    /// The requested resource does not exist.
    #[error("not found: {0}")]
    NotFound(String),
    /// The account has several brewers and none has been selected.
    #[error("several brewers found and none selected: {}", describe_devices(.0))]
    BrewerNotSelected(Vec<Device>),
    /// The account has no brewers.
    #[error("no brewers found on this account")]
    NoDevices,
    /// The response could not be decoded into the expected shape.
    #[error("unexpected response: {0}")]
    Decode(String),
}
