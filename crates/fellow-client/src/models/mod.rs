//! Data types exchanged with the Fellow API.

mod device;
mod profile;
mod schedule;

pub use device::Device;
pub use profile::{Profile, ProfileDraft, SERVER_PROFILE_FIELDS};
pub use schedule::{Schedule, ScheduleDraft};
