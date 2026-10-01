//! Profile business logic.
//!
//! Profiles live locally as one YAML file each (see [`FsProfileStore`]), so they
//! can be edited with any editor, shared, or committed to a repository. They are
//! pushed to a brewer on demand through the [`FellowApi`](fellow_client::FellowApi)
//! protocol, which [`ProfileService`] receives by dependency injection.
//!
//! The link between a local file and its remote profile is kept in a sidecar
//! file (`.lazyaiden-links.yaml`) so the YAML profiles themselves stay pristine and
//! diff-friendly.

#![warn(missing_docs)]

mod error;
mod local;
mod service;
mod store;

pub use error::{ProfileError, Result, StoreError};
pub use local::{LoadProblem, LocalListing, LocalProfile, slugify, template};
pub use service::{
    FieldChange, ImportOptions, ProfileService, ProfileStatus, PullAction, PullOutcome, PushAction,
    PushOutcome, StatusReport, SyncState,
};
pub use store::{FsProfileStore, Links, ProfileStore};
