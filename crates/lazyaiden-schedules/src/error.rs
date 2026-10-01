use fellow_client::{FellowError, ValidationIssue};

/// Result alias for this crate.
pub type Result<T, E = ScheduleError> = std::result::Result<T, E>;

/// Everything [`ScheduleService`](crate::ScheduleService) can fail with.
#[derive(Debug, thiserror::Error)]
pub enum ScheduleError {
    /// The brewer API failed.
    #[error(transparent)]
    Fellow(#[from] FellowError),
    /// The schedule data is not valid.
    #[error("invalid schedule: {}", .0.iter().map(ToString::to_string).collect::<Vec<_>>().join("; "))]
    Validation(Vec<ValidationIssue>),
    /// No schedule or profile matches.
    #[error("not found: {0}")]
    NotFound(String),
    /// A profile title matches several profiles; use the id.
    #[error("{query:?} is ambiguous; matches: {}", matches.join(", "))]
    Ambiguous {
        /// What the user typed.
        query: String,
        /// Ids of the matching profiles.
        matches: Vec<String>,
    },
    /// A time or weekday expression could not be parsed.
    #[error("cannot parse {what} {input:?}: {reason}")]
    Parse {
        /// `time` or `days`.
        what: &'static str,
        /// The offending input.
        input: String,
        /// What was wrong.
        reason: String,
    },
}
