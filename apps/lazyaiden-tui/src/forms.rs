//! Input forms and prompts. Pure state machines: keys in, validation out.

use lazyaiden_core::schedules::{Days, NewSchedule, TimeOfDay};
use lazyaiden_core::{ProfileDraft, ScheduleDraft, ValidationIssue};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde_json::{Map, Number, Value};

/// What a key press did to a form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormOutcome {
    /// Keep editing.
    Continue,
    /// The user asked to save (Ctrl-S or Enter on the last field).
    Submit,
    /// The user cancelled (Esc).
    Cancel,
}

/// How a field's text is interpreted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Free text.
    Text,
    /// Whole number.
    Int,
    /// Decimal number.
    Float,
    /// `true` / `false`, toggled with Space.
    Bool,
    /// Comma separated decimals.
    FloatList,
}

/// One editable line of a form.
#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    /// API field name, also used to attach validation messages.
    pub key: &'static str,
    /// Label shown to the user.
    pub label: &'static str,
    /// Interpretation of the text.
    pub kind: Kind,
    /// Current text.
    pub value: String,
}

const PROFILE_FIELDS: [(&str, &str, Kind); 14] = [
    ("title", "Title", Kind::Text),
    ("ratio", "Ratio (1:x)", Kind::Float),
    ("bloomEnabled", "Bloom", Kind::Bool),
    ("bloomRatio", "Bloom ratio", Kind::Float),
    ("bloomDuration", "Bloom duration (s)", Kind::Int),
    ("bloomTemperature", "Bloom temp (°C)", Kind::Float),
    ("ssPulsesEnabled", "Single-serve pulses", Kind::Bool),
    ("ssPulsesNumber", "  count", Kind::Int),
    ("ssPulsesInterval", "  interval (s)", Kind::Int),
    (
        "ssPulseTemperatures",
        "  temps (°C, comma list)",
        Kind::FloatList,
    ),
    ("batchPulsesEnabled", "Batch pulses", Kind::Bool),
    ("batchPulsesNumber", "  count", Kind::Int),
    ("batchPulsesInterval", "  interval (s)", Kind::Int),
    (
        "batchPulseTemperatures",
        "  temps (°C, comma list)",
        Kind::FloatList,
    ),
];

fn num(n: f64) -> String {
    format!("{n}")
}

fn list(v: &[f64]) -> String {
    v.iter().map(|n| num(*n)).collect::<Vec<_>>().join(", ")
}

/// Applies a key to a text value. Returns `true` if the key was consumed as editing.
fn edit_text(value: &mut String, key: KeyEvent) -> bool {
    match key.code {
        KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            value.push(c);
            true
        }
        KeyCode::Backspace => {
            value.pop();
            true
        }
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            value.clear();
            true
        }
        _ => false,
    }
}

fn is_ctrl(key: KeyEvent, c: char) -> bool {
    key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char(c)
}

/// Form for creating or editing a local profile.
#[derive(Debug, Clone, PartialEq)]
pub struct ProfileForm {
    /// Local name when editing an existing profile; `None` creates a new one.
    pub name: Option<String>,
    /// The editable fields.
    pub fields: Vec<Field>,
    /// Index of the focused field.
    pub focus: usize,
    /// Current validation problems (kept up to date after every edit).
    pub issues: Vec<ValidationIssue>,
    profile_type: u32,
}

impl ProfileForm {
    /// A form pre-filled from `draft`.
    pub fn new(name: Option<String>, draft: &ProfileDraft) -> Self {
        let value = |key: &str| -> String {
            match key {
                "title" => draft.title.clone(),
                "ratio" => num(draft.ratio),
                "bloomEnabled" => draft.bloom_enabled.to_string(),
                "bloomRatio" => num(draft.bloom_ratio),
                "bloomDuration" => draft.bloom_duration.to_string(),
                "bloomTemperature" => num(draft.bloom_temperature),
                "ssPulsesEnabled" => draft.ss_pulses_enabled.to_string(),
                "ssPulsesNumber" => draft.ss_pulses_number.to_string(),
                "ssPulsesInterval" => draft.ss_pulses_interval.to_string(),
                "ssPulseTemperatures" => list(&draft.ss_pulse_temperatures),
                "batchPulsesEnabled" => draft.batch_pulses_enabled.to_string(),
                "batchPulsesNumber" => draft.batch_pulses_number.to_string(),
                "batchPulsesInterval" => draft.batch_pulses_interval.to_string(),
                "batchPulseTemperatures" => list(&draft.batch_pulse_temperatures),
                other => unreachable!("unknown profile field {other}"),
            }
        };
        let fields = PROFILE_FIELDS
            .iter()
            .map(|(key, label, kind)| Field {
                key,
                label,
                kind: *kind,
                value: value(key),
            })
            .collect();
        let mut form = Self {
            name,
            fields,
            focus: 0,
            issues: Vec::new(),
            profile_type: draft.profile_type,
        };
        form.revalidate();
        form
    }

    /// Parses the fields into a draft, or explains what is wrong. Includes the
    /// brewer's range rules, so `Ok` means the profile can be pushed.
    pub fn draft(&self) -> Result<ProfileDraft, Vec<ValidationIssue>> {
        let mut map = Map::new();
        map.insert("profileType".into(), self.profile_type.into());
        let mut issues = Vec::new();
        for f in &self.fields {
            let text = f.value.trim();
            let parsed: Result<Value, &str> = match f.kind {
                Kind::Text => Ok(Value::String(f.value.clone())),
                Kind::Bool => Ok(Value::Bool(text == "true")),
                Kind::Int => text
                    .parse::<u32>()
                    .map(Value::from)
                    .map_err(|_| "must be a whole number"),
                Kind::Float => text
                    .parse::<f64>()
                    .ok()
                    .and_then(Number::from_f64)
                    .map(Value::Number)
                    .ok_or("must be a number"),
                Kind::FloatList => text
                    .split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(|s| {
                        s.parse::<f64>()
                            .ok()
                            .and_then(Number::from_f64)
                            .map(Value::Number)
                    })
                    .collect::<Option<Vec<_>>>()
                    .map(Value::Array)
                    .ok_or("must be numbers separated by commas"),
            };
            match parsed {
                Ok(v) => {
                    map.insert(f.key.into(), v);
                }
                Err(msg) => issues.push(ValidationIssue::new(f.key, msg)),
            }
        }
        if !issues.is_empty() {
            return Err(issues);
        }
        let draft: ProfileDraft = serde_json::from_value(Value::Object(map))
            .map_err(|e| vec![ValidationIssue::new("form", e.to_string())])?;
        let range = draft.issues();
        if range.is_empty() {
            Ok(draft)
        } else {
            Err(range)
        }
    }

    fn revalidate(&mut self) {
        self.issues = self.draft().err().unwrap_or_default();
    }

    /// The first problem attached to field `key`.
    pub fn issue_for(&self, key: &str) -> Option<&ValidationIssue> {
        self.issues.iter().find(|i| i.field == key)
    }

    /// Applies a key press.
    pub fn handle_key(&mut self, key: KeyEvent) -> FormOutcome {
        if key.code == KeyCode::Esc {
            return FormOutcome::Cancel;
        }
        if is_ctrl(key, 's') {
            return FormOutcome::Submit;
        }
        let last = self.fields.len() - 1;
        match key.code {
            KeyCode::Tab | KeyCode::Down => self.focus = (self.focus + 1).min(last),
            KeyCode::BackTab | KeyCode::Up => self.focus = self.focus.saturating_sub(1),
            KeyCode::Enter if self.focus == last => return FormOutcome::Submit,
            KeyCode::Enter => self.focus += 1,
            _ => {
                let field = &mut self.fields[self.focus];
                if field.kind == Kind::Bool {
                    if matches!(
                        key.code,
                        KeyCode::Char(' ') | KeyCode::Left | KeyCode::Right
                    ) {
                        field.value = (field.value != "true").to_string();
                    }
                } else if edit_text(&mut field.value, key) {
                    // revalidated below
                }
            }
        }
        self.revalidate();
        FormOutcome::Continue
    }
}

/// Form for creating a schedule.
#[derive(Debug, Clone, PartialEq)]
pub struct ScheduleForm {
    /// Remote profiles to choose from, as `(id, title)`.
    pub profiles: Vec<(String, String)>,
    /// Index into `profiles`.
    pub profile: usize,
    /// Weekdays text (`mon-fri`, `weekends`, ...).
    pub days: String,
    /// Start time text (`07:30`, `7:30am`).
    pub time: String,
    /// Water in millilitres, as text.
    pub water: String,
    /// Whether the schedule starts enabled.
    pub enabled: bool,
    /// Focused row: 0 profile, 1 days, 2 time, 3 water, 4 enabled.
    pub focus: usize,
    /// Current validation problems.
    pub issues: Vec<ValidationIssue>,
}

impl ScheduleForm {
    /// Number of rows.
    pub const ROWS: usize = 5;

    /// A form with sensible defaults (weekdays, 07:30, 500 ml).
    pub fn new(profiles: Vec<(String, String)>) -> Self {
        let mut f = Self {
            profiles,
            profile: 0,
            days: "mon-fri".into(),
            time: "07:30".into(),
            water: "500".into(),
            enabled: true,
            focus: 0,
            issues: Vec::new(),
        };
        f.revalidate();
        f
    }

    /// Parses the form into a schedule request, or explains what is wrong.
    pub fn new_schedule(&self) -> Result<NewSchedule, Vec<ValidationIssue>> {
        let mut issues = Vec::new();
        let days = self
            .days
            .parse::<Days>()
            .map_err(|_| {
                issues.push(ValidationIssue::new(
                    "days",
                    "use e.g. mon-fri, sat,sun, weekdays, daily",
                ))
            })
            .ok();
        let time = self
            .time
            .parse::<TimeOfDay>()
            .map_err(|_| {
                issues.push(ValidationIssue::new(
                    "time",
                    "use 24h (07:30) or 12h (7:30am)",
                ))
            })
            .ok();
        let water = self
            .water
            .trim()
            .parse::<u32>()
            .map_err(|_| {
                issues.push(ValidationIssue::new(
                    "water",
                    "must be a whole number of ml",
                ))
            })
            .ok();
        let profile = self.profiles.get(self.profile).map(|(id, _)| id.clone());
        if profile.is_none() {
            issues.push(ValidationIssue::new(
                "profile",
                "no profiles on the brewer; push one first",
            ));
        }
        if let (Some(days), Some(time), Some(water), Some(profile)) =
            (days, time, water, profile.clone())
        {
            if days.is_empty() {
                issues.push(ValidationIssue::new("days", "select at least one weekday"));
            }
            let draft = ScheduleDraft {
                days: days.0,
                second_from_start_of_the_day: time.seconds(),
                enabled: self.enabled,
                amount_of_water: water,
                profile_id: profile.clone(),
            };
            for i in draft.issues() {
                let field = if i.field == "amountOfWater" {
                    "water"
                } else {
                    "profile"
                };
                issues.push(ValidationIssue::new(field, i.message));
            }
            if issues.is_empty() {
                return Ok(NewSchedule {
                    days,
                    time,
                    water_ml: water,
                    profile,
                    enabled: self.enabled,
                });
            }
        }
        Err(issues)
    }

    fn revalidate(&mut self) {
        self.issues = self.new_schedule().err().unwrap_or_default();
    }

    /// The first problem attached to `field` (`profile`, `days`, `time`, `water`).
    pub fn issue_for(&self, field: &str) -> Option<&ValidationIssue> {
        self.issues.iter().find(|i| i.field == field)
    }

    /// Applies a key press.
    pub fn handle_key(&mut self, key: KeyEvent) -> FormOutcome {
        if key.code == KeyCode::Esc {
            return FormOutcome::Cancel;
        }
        if is_ctrl(key, 's') {
            return FormOutcome::Submit;
        }
        match key.code {
            KeyCode::Tab | KeyCode::Down => self.focus = (self.focus + 1).min(Self::ROWS - 1),
            KeyCode::BackTab | KeyCode::Up => self.focus = self.focus.saturating_sub(1),
            KeyCode::Enter if self.focus == Self::ROWS - 1 => return FormOutcome::Submit,
            KeyCode::Enter => self.focus += 1,
            _ => match self.focus {
                0 => {
                    let n = self.profiles.len().max(1);
                    match key.code {
                        KeyCode::Right | KeyCode::Char(' ') => {
                            self.profile = (self.profile + 1) % n
                        }
                        KeyCode::Left => self.profile = (self.profile + n - 1) % n,
                        _ => {}
                    }
                }
                1 => {
                    edit_text(&mut self.days, key);
                }
                2 => {
                    edit_text(&mut self.time, key);
                }
                3 => {
                    edit_text(&mut self.water, key);
                }
                _ => {
                    if matches!(
                        key.code,
                        KeyCode::Char(' ') | KeyCode::Left | KeyCode::Right
                    ) {
                        self.enabled = !self.enabled;
                    }
                }
            },
        }
        self.revalidate();
        FormOutcome::Continue
    }
}

/// What a single-line prompt is asking for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromptKind {
    /// Title of a new profile.
    NewProfile,
    /// Path of a file to import.
    Import,
    /// Path to export the named profile to.
    Export(String),
    /// A share link to import.
    ImportLink,
}

/// A single-line text prompt.
#[derive(Debug, Clone, PartialEq)]
pub struct Prompt {
    /// What the answer is used for.
    pub kind: PromptKind,
    /// Question shown above the input.
    pub label: String,
    /// Current answer.
    pub value: String,
}

impl Prompt {
    /// A prompt with an initial answer.
    pub fn new(kind: PromptKind, label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            kind,
            label: label.into(),
            value: value.into(),
        }
    }

    /// Applies a key press; `Submit` only when the answer is non-empty.
    pub fn handle_key(&mut self, key: KeyEvent) -> FormOutcome {
        match key.code {
            KeyCode::Esc => FormOutcome::Cancel,
            KeyCode::Enter if !self.value.trim().is_empty() => FormOutcome::Submit,
            _ => {
                edit_text(&mut self.value, key);
                FormOutcome::Continue
            }
        }
    }
}
