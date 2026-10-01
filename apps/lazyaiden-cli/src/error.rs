use lazyaiden_core::CoreError;
use lazyaiden_core::profiles::{ProfileError, StoreError};
use lazyaiden_core::schedules::ScheduleError;
use lazyaiden_core::{Device, FellowError, ProfileDraft};

/// Everything a command can fail with, mapped to a process exit code.
#[derive(Debug, thiserror::Error)]
pub enum CliError {
    /// Wrong arguments (beyond what clap catches).
    #[error("{0}")]
    Usage(String),
    /// Configuration, credentials or device selection failed.
    #[error(transparent)]
    Core(#[from] CoreError),
    /// A profile operation failed.
    #[error(transparent)]
    Profile(#[from] ProfileError),
    /// A schedule operation failed.
    #[error(transparent)]
    Schedule(#[from] ScheduleError),
    /// Reading or writing a stream failed.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// A batch operation had failures that were already reported in the output.
    #[error("some operations failed")]
    Reported,
}

impl CliError {
    /// Message to print on stderr; `None` when the details were already printed.
    pub fn message(&self) -> Option<String> {
        match self {
            CliError::Reported => None,
            other => Some(other.to_string()),
        }
    }

    /// Exit code: `2` usage, `3` authentication, `4` invalid data, `5` not found,
    /// `6` conflict, `7` network/server, `1` anything else.
    pub fn exit_code(&self) -> u8 {
        fn fellow(e: &FellowError) -> u8 {
            match e {
                FellowError::Auth(_) => 3,
                FellowError::Validation(_) | FellowError::Decode(_) => 4,
                FellowError::NotFound(_)
                | FellowError::BrewerNotSelected(_)
                | FellowError::NoDevices => 5,
                FellowError::Network(_) | FellowError::Api { .. } => 7,
            }
        }
        match self {
            CliError::Usage(_) => 2,
            CliError::Reported | CliError::Io(_) => 1,
            CliError::Core(CoreError::Fellow(e)) => fellow(e),
            CliError::Core(CoreError::NotFound(_) | CoreError::Ambiguous { .. }) => 5,
            CliError::Core(CoreError::Credentials(_)) => 3,
            CliError::Core(CoreError::Config { .. }) => 1,
            CliError::Profile(e) => match e {
                ProfileError::Fellow(e) => fellow(e),
                ProfileError::Validation(_) | ProfileError::Parse(_) => 4,
                ProfileError::Store(StoreError::Parse { .. } | StoreError::InvalidName(_)) => 4,
                ProfileError::Store(StoreError::NotFound(_))
                | ProfileError::NotFound(_)
                | ProfileError::Ambiguous { .. }
                | ProfileError::NotPushed(_) => 5,
                ProfileError::Conflict(_) => 6,
                ProfileError::Store(StoreError::Io { .. }) => 1,
            },
            CliError::Schedule(e) => match e {
                ScheduleError::Fellow(e) => fellow(e),
                ScheduleError::Validation(_) | ScheduleError::Parse { .. } => 4,
                ScheduleError::NotFound(_) | ScheduleError::Ambiguous { .. } => 5,
            },
        }
    }
}

/// Short description of a device for output.
pub(crate) fn device_name(d: &Device) -> String {
    d.display_name.clone().unwrap_or_else(|| "-".into())
}

pub(crate) fn draft_json(d: &ProfileDraft) -> serde_json::Value {
    serde_json::to_value(d).expect("draft serializes")
}
