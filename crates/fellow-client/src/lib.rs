//! Client library for the (unofficial) Fellow Aiden cloud API.
//!
//! The crate exposes a *protocol* — the [`FellowApi`] trait — and a *default
//! implementation* of it, [`HttpFellowClient`], which talks to Fellow's
//! mobile-app backend over HTTPS. Business-logic crates depend on the trait
//! only, so they can be exercised against [`testing::InMemoryFellow`] (feature
//! `testing`) without any network access.
//!
//! ```
//! use fellow_client::{Credentials, HttpClientOptions, HttpFellowClient, FellowApi};
//!
//! # async fn demo() -> Result<(), fellow_client::FellowError> {
//! let client = HttpFellowClient::new(
//!     Credentials::new("me@example.com", "secret"),
//!     HttpClientOptions::default(),
//! )?;
//! for profile in client.profiles().await? {
//!     println!("{} ({})", profile.title, profile.id);
//! }
//! # Ok(()) }
//! ```
//!
//! The API is reverse-engineered; response models are deliberately lenient and
//! keep unknown fields so new server fields never break decoding.

#![warn(missing_docs)]

mod api;
mod error;
mod http;
pub mod models;
#[cfg(feature = "testing")]
pub mod testing;
pub mod validation;

pub use api::{FellowApi, brew_id_from_link};
pub use error::{FellowError, Result, ValidationIssue};
pub use http::{Credentials, HttpClientOptions, HttpFellowClient};
pub use models::{Device, Profile, ProfileDraft, Schedule, ScheduleDraft};
