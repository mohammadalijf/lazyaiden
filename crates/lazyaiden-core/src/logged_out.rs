use async_trait::async_trait;
use fellow_client::{
    Device, FellowApi, FellowError, Profile, ProfileDraft, Result, Schedule, ScheduleDraft,
};
use serde_json::Value;

/// Message shown whenever a remote operation is attempted without credentials.
pub const NOT_LOGGED_IN: &str =
    "not logged in: run `lazyaiden-cli login` or set FELLOW_EMAIL and FELLOW_PASSWORD";

/// A [`FellowApi`] that fails every remote call with an authentication error.
///
/// [`Aiden::connect`](crate::Aiden::connect) uses it when no credentials are
/// available, so purely local features (editing, importing and exporting
/// profiles) keep working while remote ones explain how to log in.
#[derive(Debug, Default, Clone)]
pub struct LoggedOut {
    reason: Option<String>,
}

impl LoggedOut {
    /// Logged out because credentials could not be read (e.g. no keychain available).
    pub fn because(reason: impl Into<String>) -> Self {
        Self {
            reason: Some(reason.into()),
        }
    }

    fn deny<T>(&self) -> Result<T> {
        Err(FellowError::Auth(match &self.reason {
            Some(r) => format!("{NOT_LOGGED_IN} ({r})"),
            None => NOT_LOGGED_IN.into(),
        }))
    }
}

#[async_trait]
impl FellowApi for LoggedOut {
    async fn login(&self) -> Result<()> {
        self.deny()
    }
    async fn devices(&self) -> Result<Vec<Device>> {
        self.deny()
    }
    fn select_brewer(&self, _: &str) {}
    fn selected_brewer(&self) -> Option<String> {
        None
    }
    async fn active_brewer(&self) -> Result<String> {
        self.deny()
    }
    async fn adjust_setting(&self, _: &str, _: Value) -> Result<Value> {
        self.deny()
    }
    async fn profiles(&self) -> Result<Vec<Profile>> {
        self.deny()
    }
    async fn create_profile(&self, _: &ProfileDraft) -> Result<Profile> {
        self.deny()
    }
    async fn update_profile(&self, _: &str, _: &ProfileDraft) -> Result<Profile> {
        self.deny()
    }
    async fn delete_profile(&self, _: &str) -> Result<()> {
        self.deny()
    }
    async fn share_profile(&self, _: &str) -> Result<String> {
        self.deny()
    }
    async fn fetch_shared_profile(&self, _: &str) -> Result<Profile> {
        self.deny()
    }
    async fn schedules(&self) -> Result<Vec<Schedule>> {
        self.deny()
    }
    async fn create_schedule(&self, _: &ScheduleDraft) -> Result<Schedule> {
        self.deny()
    }
    async fn toggle_schedule(&self, _: &str, _: bool) -> Result<()> {
        self.deny()
    }
    async fn delete_schedule(&self, _: &str) -> Result<()> {
        self.deny()
    }
}
