use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::error::{FellowError, Result, ValidationIssue};
use crate::validation as v;

/// Fields the server manages. They are never sent on create/update and are
/// dropped from shared profiles before reuse.
pub const SERVER_PROFILE_FIELDS: &[&str] = &[
    "id",
    "createdAt",
    "deletedAt",
    "lastUsedTime",
    "sharedFrom",
    "isDefaultProfile",
    "instantBrew",
    "folder",
    "duration",
    "lastGBQuantity",
];

/// The user-editable part of a brew profile; this is what gets validated,
/// stored locally and sent to the brewer. Field names match the API (camelCase).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileDraft {
    /// Profile kind (`0` for regular profiles).
    pub profile_type: u32,
    /// Display title, at most 50 characters from a restricted character set.
    pub title: String,
    /// Brew ratio (1:x).
    pub ratio: f64,
    /// Whether a bloom phase runs first.
    pub bloom_enabled: bool,
    /// Bloom water ratio.
    pub bloom_ratio: f64,
    /// Bloom duration in seconds.
    pub bloom_duration: u32,
    /// Bloom water temperature in °C.
    pub bloom_temperature: f64,
    /// Whether single-serve pulses are enabled.
    pub ss_pulses_enabled: bool,
    /// Number of single-serve pulses.
    pub ss_pulses_number: u32,
    /// Seconds between single-serve pulses.
    pub ss_pulses_interval: u32,
    /// Temperature of each single-serve pulse in °C.
    pub ss_pulse_temperatures: Vec<f64>,
    /// Whether batch pulses are enabled.
    pub batch_pulses_enabled: bool,
    /// Number of batch pulses.
    pub batch_pulses_number: u32,
    /// Seconds between batch pulses.
    pub batch_pulses_interval: u32,
    /// Temperature of each batch pulse in °C.
    pub batch_pulse_temperatures: Vec<f64>,
}

impl ProfileDraft {
    /// Checks every field and returns all problems found (empty means valid).
    pub fn issues(&self) -> Vec<ValidationIssue> {
        let mut out = Vec::new();
        let mut bad = |field: &str, msg: String| out.push(ValidationIssue::new(field, msg));

        if self.title.is_empty() {
            bad("title", "must not be empty".into());
        } else if self.title.chars().count() > v::TITLE_MAX_LEN {
            bad(
                "title",
                format!("must be at most {} characters", v::TITLE_MAX_LEN),
            );
        }
        if let Some(c) = self.title.chars().find(|c| !v::is_valid_title_char(*c)) {
            bad(
                "title",
                format!(
                    "contains unsupported character {c:?}; allowed: letters, digits, space and {}",
                    v::TITLE_PUNCTUATION
                ),
            );
        }
        if !v::RATIO.contains(self.ratio) {
            bad(
                "ratio",
                format!("{} is not one of {}", self.ratio, v::RATIO.describe()),
            );
        }
        if !v::BLOOM_RATIO.contains(self.bloom_ratio) {
            bad(
                "bloomRatio",
                format!(
                    "{} is not one of {}",
                    self.bloom_ratio,
                    v::BLOOM_RATIO.describe()
                ),
            );
        }
        let mut range = |field: &str, value: u32, allowed: &std::ops::RangeInclusive<u32>| {
            if !allowed.contains(&value) {
                bad(
                    field,
                    format!("{value} is outside {}–{}", allowed.start(), allowed.end()),
                );
            }
        };
        range("bloomDuration", self.bloom_duration, &v::BLOOM_DURATION);
        range("ssPulsesNumber", self.ss_pulses_number, &v::PULSES_NUMBER);
        range(
            "ssPulsesInterval",
            self.ss_pulses_interval,
            &v::PULSES_INTERVAL,
        );
        range(
            "batchPulsesNumber",
            self.batch_pulses_number,
            &v::PULSES_NUMBER,
        );
        range(
            "batchPulsesInterval",
            self.batch_pulses_interval,
            &v::PULSES_INTERVAL,
        );

        let mut temp = |field: &str, t: f64| {
            if !v::TEMPERATURE.contains(t) {
                bad(
                    field,
                    format!("{t} is not one of {} °C", v::TEMPERATURE.describe()),
                );
            }
        };
        temp("bloomTemperature", self.bloom_temperature);
        for t in &self.ss_pulse_temperatures {
            temp("ssPulseTemperatures", *t);
        }
        for t in &self.batch_pulse_temperatures {
            temp("batchPulseTemperatures", *t);
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

/// A profile as returned by the server, including server-managed fields.
///
/// Only `id` and `title` are modelled; everything else is kept in `extra`.
/// Use [`Profile::draft`] to obtain the editable, validated part.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    /// Server-assigned id (`p<number>`).
    #[serde(default)]
    pub id: String,
    /// Display title.
    #[serde(default)]
    pub title: String,
    /// All other fields, preserved verbatim.
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Profile {
    /// Extracts the editable fields. Server-managed fields are ignored.
    pub fn draft(&self) -> Result<ProfileDraft> {
        let mut map = self.extra.clone();
        map.insert("title".into(), Value::String(self.title.clone()));
        serde_json::from_value(Value::Object(map))
            .map_err(|e| FellowError::Decode(format!("profile {}: {e}", self.id)))
    }

    /// Builds a server-shaped profile from a draft; used by fakes and tests.
    pub fn from_draft(id: impl Into<String>, draft: &ProfileDraft) -> Self {
        let Value::Object(mut map) = serde_json::to_value(draft).expect("draft serializes") else {
            unreachable!("ProfileDraft serializes to an object")
        };
        map.remove("title");
        Self {
            id: id.into(),
            title: draft.title.clone(),
            extra: map,
        }
    }
}
