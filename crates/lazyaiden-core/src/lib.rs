//! Composition root of Aiden.
//!
//! This crate is the only place that knows how to turn *configuration* and
//! *credentials* into working services: it builds the default
//! [`HttpFellowClient`](fellow_client::HttpFellowClient) and the on-disk profile
//! store, then hands them (as trait objects) to the business-logic crates.
//! Applications obtain an [`Aiden`] and never touch the client directly.
//!
//! * [`Config`] — `flag > environment > config file > default`.
//! * [`CredentialStore`] — environment variables first, OS keychain second.
//! * [`DeviceService`] — listing and choosing the brewer to act on.
//! * [`Aiden`] — the wired-up services.

#![warn(missing_docs)]

mod aiden;
mod config;
mod credentials;
mod devices;
mod error;
mod logged_out;

pub use aiden::{Aiden, login, logout};
pub use config::{Config, ConfigFile, Env, Overrides, process_env};
pub use credentials::{
    CredentialStore, Credentials, EnvCredentials, KeyringCredentials, LayeredCredentials,
    MemoryCredentials,
};
pub use devices::{DeviceService, DeviceView};
pub use error::{CoreError, Result};
pub use logged_out::{LoggedOut, NOT_LOGGED_IN};

// Re-exported so apps depend on the core crate only for these types.
pub use fellow_client::{
    Device, FellowError, Profile, ProfileDraft, Schedule, ScheduleDraft, ValidationIssue,
};
pub use lazyaiden_profiles as profiles;
pub use lazyaiden_schedules as schedules;
