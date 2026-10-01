use std::sync::Arc;

use fellow_client::{FellowApi, Profile, Schedule, ScheduleDraft, ValidationIssue};

use crate::error::{Result, ScheduleError};
use crate::time::{Days, TimeOfDay};

/// A schedule joined with the title of the profile it brews.
#[derive(Debug, Clone, PartialEq)]
pub struct ScheduleView {
    /// The schedule as stored on the brewer.
    pub schedule: Schedule,
    /// Title of the referenced profile; `None` if it no longer exists.
    pub profile_title: Option<String>,
}

impl ScheduleView {
    /// The start time as a [`TimeOfDay`], if the stored value is in range.
    pub fn time(&self) -> Option<TimeOfDay> {
        TimeOfDay::from_seconds(self.schedule.draft.second_from_start_of_the_day)
    }

    /// The weekdays.
    pub fn days(&self) -> Days {
        Days(self.schedule.draft.days)
    }
}

/// Input for [`ScheduleService::create`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewSchedule {
    /// Weekdays to run on (at least one).
    pub days: Days,
    /// Start time.
    pub time: TimeOfDay,
    /// Water in millilitres (150–1500).
    pub water_ml: u32,
    /// Remote profile id (`p12`) or exact title.
    pub profile: String,
    /// Whether the schedule starts enabled.
    pub enabled: bool,
}

/// Business logic for brew schedules.
pub struct ScheduleService {
    api: Arc<dyn FellowApi>,
}

fn resolve_profile<'a>(profiles: &'a [Profile], query: &str) -> Result<&'a Profile> {
    if let Some(p) = profiles.iter().find(|p| p.id == query) {
        return Ok(p);
    }
    let matches: Vec<&Profile> = profiles
        .iter()
        .filter(|p| p.title.eq_ignore_ascii_case(query))
        .collect();
    match matches.as_slice() {
        [] => Err(ScheduleError::NotFound(format!("profile {query:?}"))),
        [one] => Ok(one),
        many => Err(ScheduleError::Ambiguous {
            query: query.into(),
            matches: many.iter().map(|p| p.id.clone()).collect(),
        }),
    }
}

impl ScheduleService {
    /// Creates the service around an injected brewer protocol.
    pub fn new(api: Arc<dyn FellowApi>) -> Self {
        Self { api }
    }

    /// Schedules of the selected brewer with profile titles resolved.
    pub async fn list(&self) -> Result<Vec<ScheduleView>> {
        let schedules = self.api.schedules().await?;
        let profiles = self.api.profiles().await?;
        Ok(schedules
            .into_iter()
            .map(|schedule| {
                let profile_title = profiles
                    .iter()
                    .find(|p| p.id == schedule.draft.profile_id)
                    .map(|p| p.title.clone());
                ScheduleView {
                    schedule,
                    profile_title,
                }
            })
            .collect())
    }

    /// Creates a schedule after resolving the profile and validating the input.
    pub async fn create(&self, new: NewSchedule) -> Result<Schedule> {
        if new.days.is_empty() {
            return Err(ScheduleError::Validation(vec![ValidationIssue::new(
                "days",
                "select at least one weekday",
            )]));
        }
        let profiles = self.api.profiles().await?;
        let profile = resolve_profile(&profiles, &new.profile)?;
        let draft = ScheduleDraft {
            days: new.days.0,
            second_from_start_of_the_day: new.time.seconds(),
            enabled: new.enabled,
            amount_of_water: new.water_ml,
            profile_id: profile.id.clone(),
        };
        let issues = draft.issues();
        if !issues.is_empty() {
            return Err(ScheduleError::Validation(issues));
        }
        Ok(self.api.create_schedule(&draft).await?)
    }

    async fn current(&self, id: &str) -> Result<Schedule> {
        let schedules = self.api.schedules().await?;
        schedules
            .into_iter()
            .find(|s| s.id == id)
            .ok_or_else(|| ScheduleError::NotFound(format!("schedule {id:?}")))
    }

    /// Enables or disables a schedule.
    pub async fn set_enabled(&self, id: &str, enabled: bool) -> Result<()> {
        self.current(id).await?;
        Ok(self.api.toggle_schedule(id, enabled).await?)
    }

    /// Flips a schedule's enabled flag and returns the new value.
    pub async fn toggle(&self, id: &str) -> Result<bool> {
        let enabled = !self.current(id).await?.draft.enabled;
        self.api.toggle_schedule(id, enabled).await?;
        Ok(enabled)
    }

    /// Deletes a schedule.
    pub async fn delete(&self, id: &str) -> Result<()> {
        self.current(id).await?;
        Ok(self.api.delete_schedule(id).await?)
    }
}
