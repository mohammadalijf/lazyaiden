use async_trait::async_trait;
use serde_json::Value;

use crate::error::{FellowError, Result};
use crate::models::{Device, Profile, ProfileDraft, Schedule, ScheduleDraft};

/// The Fellow protocol: everything the rest of the workspace may ask of the cloud.
///
/// Implementations are bound to one account. Device-scoped operations act on
/// the *selected brewer*: it is chosen explicitly with
/// [`select_brewer`](Self::select_brewer), or automatically when the account
/// has exactly one brewer. With several brewers and no selection, device-scoped
/// calls fail with [`FellowError::BrewerNotSelected`].
#[async_trait]
pub trait FellowApi: Send + Sync {
    /// Verifies the credentials by authenticating against the server.
    async fn login(&self) -> Result<()>;

    /// Lists all brewers on the account.
    async fn devices(&self) -> Result<Vec<Device>>;

    /// Chooses the brewer that device-scoped calls act on.
    fn select_brewer(&self, id: &str);

    /// The currently selected brewer id, if any (does not auto-select).
    fn selected_brewer(&self) -> Option<String>;

    /// The brewer device-scoped calls will act on, auto-selecting the only one.
    ///
    /// Errors with [`FellowError::NoDevices`] or [`FellowError::BrewerNotSelected`].
    async fn active_brewer(&self) -> Result<String>;

    /// Applies a single device setting (`PATCH /devices/{id}`); returns the response body.
    async fn adjust_setting(&self, setting: &str, value: Value) -> Result<Value>;

    /// Lists the selected brewer's profiles.
    async fn profiles(&self) -> Result<Vec<Profile>>;

    /// Creates a profile after validating it; returns the stored profile.
    async fn create_profile(&self, draft: &ProfileDraft) -> Result<Profile>;

    /// Replaces the fields of an existing profile after validating them.
    async fn update_profile(&self, profile_id: &str, draft: &ProfileDraft) -> Result<Profile>;

    /// Deletes a profile.
    async fn delete_profile(&self, profile_id: &str) -> Result<()>;

    /// Creates a share link for a profile.
    async fn share_profile(&self, profile_id: &str) -> Result<String>;

    /// Fetches a shared profile from a share link or bare brew id.
    ///
    /// The result still has server-managed fields; [`Profile::draft`] drops them.
    async fn fetch_shared_profile(&self, link_or_id: &str) -> Result<Profile>;

    /// Lists the selected brewer's schedules.
    async fn schedules(&self) -> Result<Vec<Schedule>>;

    /// Creates a schedule after validating it.
    async fn create_schedule(&self, draft: &ScheduleDraft) -> Result<Schedule>;

    /// Enables or disables a schedule.
    async fn toggle_schedule(&self, schedule_id: &str, enabled: bool) -> Result<()>;

    /// Deletes a schedule.
    async fn delete_schedule(&self, schedule_id: &str) -> Result<()>;
}

/// Extracts the brew id from a share link such as `https://fellowproducts.com/p/abc123`
/// or from a bare id.
pub fn brew_id_from_link(link: &str) -> Result<String> {
    let id = link
        .trim()
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or_default();
    if id.is_empty() || !id.bytes().all(|b| b.is_ascii_alphanumeric()) {
        return Err(FellowError::Decode(format!(
            "invalid profile link or id: {link:?}"
        )));
    }
    Ok(id.to_owned())
}
