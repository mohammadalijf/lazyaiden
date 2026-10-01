//! Plain data the UI shows. No terminal or network types here.

use lazyaiden_core::profiles::{LoadProblem, LocalProfile, SyncState};
use lazyaiden_core::{Device, Profile};
use lazyaiden_core::{FellowError, schedules::ScheduleView};

/// The three left-hand panels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    /// Local and remote profiles.
    Profiles,
    /// Brew schedules.
    Schedules,
    /// Brewer selection.
    Device,
}

impl Panel {
    /// All panels in display order.
    pub const ALL: [Panel; 3] = [Panel::Profiles, Panel::Schedules, Panel::Device];

    /// Panel title.
    pub fn title(self) -> &'static str {
        match self {
            Panel::Profiles => "Profiles",
            Panel::Schedules => "Schedules",
            Panel::Device => "Device",
        }
    }

    /// The next panel, wrapping.
    pub fn next(self) -> Panel {
        Panel::ALL[(self as usize + 1) % 3]
    }

    /// The previous panel, wrapping.
    pub fn prev(self) -> Panel {
        Panel::ALL[(self as usize + 2) % 3]
    }
}

/// A scrollable list with one selected row.
#[derive(Debug, Clone, PartialEq)]
pub struct List<T> {
    /// Items in display order.
    pub items: Vec<T>,
    /// Index of the selected item (always valid when `items` is non-empty).
    pub selected: usize,
}

impl<T> Default for List<T> {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            selected: 0,
        }
    }
}

impl<T> List<T> {
    /// A list over `items` with the first row selected.
    pub fn new(items: Vec<T>) -> Self {
        Self { items, selected: 0 }
    }

    /// The selected item.
    pub fn current(&self) -> Option<&T> {
        self.items.get(self.selected)
    }

    /// Moves the selection by `delta`, clamped to the list.
    pub fn step(&mut self, delta: isize) {
        if self.items.is_empty() {
            return;
        }
        let max = self.items.len() - 1;
        self.selected = self.selected.saturating_add_signed(delta).min(max);
    }

    /// Selects the first row.
    pub fn first(&mut self) {
        self.selected = 0;
    }

    /// Selects the last row.
    pub fn last(&mut self) {
        self.selected = self.items.len().saturating_sub(1);
    }

    /// Replaces the items, keeping the selection index in range.
    pub fn replace(&mut self, items: Vec<T>) {
        self.items = items;
        self.selected = self.selected.min(self.items.len().saturating_sub(1));
    }
}

/// One row of the profiles panel.
#[derive(Debug, Clone, PartialEq)]
pub struct ProfileRow {
    /// Local name; `None` for remote-only profiles.
    pub name: Option<String>,
    /// Display title.
    pub title: String,
    /// Sync state; `None` when the brewer is unreachable so it is unknown.
    pub state: Option<SyncState>,
    /// Remote id when known.
    pub remote_id: Option<String>,
    /// The local profile, if any.
    pub local: Option<LocalProfile>,
    /// The remote profile, if any.
    pub remote: Option<Profile>,
}

/// Who made a profile on the brewer, judged by its remote id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// Shipped by Fellow (`d<digits>` or `plocal<digits>`).
    Fellow,
    /// Created by the user (`p<digits>`).
    Custom,
}

impl Origin {
    /// Classifies a remote profile id; `None` when it matches no known pattern.
    pub fn of(remote_id: &str) -> Option<Origin> {
        let digits = |s: Option<&str>| {
            s.is_some_and(|d| !d.is_empty() && d.bytes().all(|b| b.is_ascii_digit()))
        };
        if digits(remote_id.strip_prefix("plocal")) || digits(remote_id.strip_prefix('d')) {
            Some(Origin::Fellow)
        } else if digits(remote_id.strip_prefix('p')) {
            Some(Origin::Custom)
        } else {
            None
        }
    }
}

impl ProfileRow {
    /// Where the brewer copy came from; `None` for local-only rows.
    pub fn origin(&self) -> Option<Origin> {
        self.remote_id.as_deref().and_then(Origin::of)
    }
}

/// A device and whether it is the active one.
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceRow {
    /// The brewer.
    pub device: Device,
    /// Whether device-scoped calls act on it.
    pub active: bool,
}

/// An error shown to the user, with the hints the UI can act on.
#[derive(Debug, Clone, PartialEq)]
pub struct UiError {
    /// Message for the status line.
    pub message: String,
    /// Set when the account has several brewers and none is chosen.
    pub brewer_choices: Option<Vec<Device>>,
}

impl UiError {
    /// An error with no special handling.
    pub fn plain(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            brewer_choices: None,
        }
    }

    /// Classifies any error that can come out of the services.
    pub fn from_error(e: &(dyn std::error::Error + 'static)) -> Self {
        match fellow_error(e) {
            Some(FellowError::BrewerNotSelected(d)) => Self {
                message: "choose a brewer to continue".into(),
                brewer_choices: Some(d.clone()),
            },
            _ => Self::plain(e.to_string()),
        }
    }
}

/// Finds the underlying API error. `#[error(transparent)]` wrappers hide it from
/// `source()`, so the known service error types are unwrapped explicitly.
fn fellow_error<'a>(e: &'a (dyn std::error::Error + 'static)) -> Option<&'a FellowError> {
    use lazyaiden_core::CoreError;
    use lazyaiden_core::profiles::ProfileError;
    use lazyaiden_core::schedules::ScheduleError;
    if let Some(f) = e.downcast_ref::<FellowError>() {
        return Some(f);
    }
    match (
        e.downcast_ref::<ProfileError>(),
        e.downcast_ref::<ScheduleError>(),
        e.downcast_ref::<CoreError>(),
    ) {
        (Some(ProfileError::Fellow(f)), _, _)
        | (_, Some(ScheduleError::Fellow(f)), _)
        | (_, _, Some(CoreError::Fellow(f))) => Some(f),
        _ => None,
    }
}

/// Everything the UI needs, gathered by one refresh.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Snapshot {
    /// Profile rows (with sync state when the brewer was reachable).
    pub profiles: Vec<ProfileRow>,
    /// Whether the rows carry remote state.
    pub remote_ok: bool,
    /// Local files that could not be read.
    pub problems: Vec<LoadProblem>,
    /// Schedules, if they could be loaded.
    pub schedules: Option<Vec<ScheduleView>>,
    /// Brewers, if they could be loaded.
    pub devices: Option<Vec<DeviceRow>>,
    /// Errors from the parts that failed.
    pub errors: Vec<UiError>,
}

/// A transient message in the status line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    /// Text.
    pub text: String,
    /// Whether to style it as an error.
    pub error: bool,
}
