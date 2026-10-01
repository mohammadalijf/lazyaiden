use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::error::{FellowError, Result, ValidationIssue};
use crate::validation as v;

/// The user-editable part of a brew schedule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleDraft {
    /// Which weekdays the schedule runs on, Sunday first.
    pub days: [bool; 7],
    /// Start time as seconds since local midnight (0–86399).
    pub second_from_start_of_the_day: u32,
    /// Whether the schedule is active.
    pub enabled: bool,
    /// Water amount in millilitres (150–1500).
    pub amount_of_water: u32,
    /// Profile to brew: `p<number>` (cloud) or `plocal<number>` (on-device).
    pub profile_id: String,
}

impl ScheduleDraft {
    /// Checks every field and returns all problems found (empty means valid).
    pub fn issues(&self) -> Vec<ValidationIssue> {
        let mut out = Vec::new();
        if self.second_from_start_of_the_day > v::MAX_SECOND_OF_DAY {
            out.push(ValidationIssue::new(
                "secondFromStartOfTheDay",
                format!(
                    "{} is outside 0–{}",
                    self.second_from_start_of_the_day,
                    v::MAX_SECOND_OF_DAY
                ),
            ));
        }
        if !v::WATER_ML.contains(&self.amount_of_water) {
            out.push(ValidationIssue::new(
                "amountOfWater",
                format!(
                    "{} is outside {}–{} ml",
                    self.amount_of_water,
                    v::WATER_ML.start(),
                    v::WATER_ML.end()
                ),
            ));
        }
        if !v::is_valid_profile_id(&self.profile_id) {
            out.push(ValidationIssue::new(
                "profileId",
                format!(
                    "{:?} must match p<number> or plocal<number>",
                    self.profile_id
                ),
            ));
        }
        out
    }

    /// Like [`issues`](Self::issues) but as a `Result`.
    pub fn validate(&self) -> Result<()> {
        let issues = self.issues();
        if issues.is_empty() {
            Ok(())
        } else {
            Err(FellowError::Validation(issues))
        }
    }
}

/// A schedule as returned by the server.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Schedule {
    /// Server-assigned id.
    pub id: String,
    /// The editable fields.
    #[serde(flatten)]
    pub draft: ScheduleDraft,
    /// Any additional server fields, preserved verbatim.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}
