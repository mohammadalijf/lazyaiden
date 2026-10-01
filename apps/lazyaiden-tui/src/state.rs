//! Application state and the pure reducer.
//!
//! `update(app, event)` mutates the state and returns the [`Effect`]s to run.
//! It performs no I/O, so every behaviour is unit-testable; `effects::perform`
//! executes effects against the services and answers with [`Msg`]s.

use lazyaiden_core::profiles::{SyncState, template};
use lazyaiden_core::schedules::NewSchedule;
use lazyaiden_core::{Device, ProfileDraft};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::forms::{FormOutcome, ProfileForm, Prompt, PromptKind, ScheduleForm};
use crate::model::{DeviceRow, List, Message, Origin, Panel, ProfileRow, Snapshot, UiError};
use lazyaiden_core::profiles::LoadProblem;
use lazyaiden_core::schedules::ScheduleView;

/// Work the reducer asks the runtime to do. Variant fields name the target profile/schedule/path.
#[derive(Debug, Clone, PartialEq)]
#[allow(missing_docs)]
pub enum Effect {
    /// Reload everything.
    Refresh,
    /// Create a local profile from the template, then open it for editing.
    NewProfile { title: String },
    /// Validate and store a profile from the form.
    SaveProfile {
        name: Option<String>,
        draft: ProfileDraft,
    },
    /// Import a YAML/JSON file.
    Import { path: String },
    /// Export a local profile to a file.
    Export { name: String, path: String },
    /// Open the profile's file in `$EDITOR` (the runtime suspends the terminal).
    OpenEditor { name: String },
    /// Re-read and validate a profile file after external editing.
    Validate { name: String },
    /// Push one profile.
    Push { name: String },
    /// Push every local profile.
    PushAll,
    /// Pull one remote profile (by id).
    Pull { remote_id: String },
    /// Pull every remote profile.
    PullAll,
    /// Delete the local file only.
    DeleteLocal { name: String },
    /// Delete the remote copy (by local name or remote id), keeping any local file.
    DeleteRemote { query: String },
    /// Delete the remote copy and the local file.
    DeleteBoth { name: String },
    /// Create a share link.
    Share { name: String },
    /// Import from a share link.
    ImportLink { link: String },
    /// Create a schedule.
    CreateSchedule(NewSchedule),
    /// Flip a schedule's enabled flag.
    ToggleSchedule { id: String },
    /// Delete a schedule.
    DeleteSchedule { id: String },
    /// Choose the active brewer (id).
    SelectBrewer { id: String },
    /// Put text on the system clipboard (the runtime owns the clipboard).
    Copy { text: String },
}

impl Effect {
    /// What the status line says while the effect runs.
    pub fn label(&self) -> String {
        match self {
            Effect::Refresh => "refreshing".into(),
            Effect::NewProfile { title } => format!("creating {title}"),
            Effect::SaveProfile { .. } => "saving".into(),
            Effect::Import { path } => format!("importing {path}"),
            Effect::Export { name, .. } => format!("exporting {name}"),
            Effect::OpenEditor { name } => format!("editing {name}"),
            Effect::Validate { name } => format!("validating {name}"),
            Effect::Push { name } => format!("pushing {name}"),
            Effect::PushAll => "pushing all profiles".into(),
            Effect::Pull { remote_id } => format!("pulling {remote_id}"),
            Effect::PullAll => "pulling all profiles".into(),
            Effect::DeleteLocal { name } => format!("deleting {name}"),
            Effect::DeleteRemote { query } => format!("deleting {query} on the brewer"),
            Effect::DeleteBoth { name } => format!("deleting {name}"),
            Effect::Share { name } => format!("creating a share link for {name}"),
            Effect::ImportLink { .. } => "importing the shared profile".into(),
            Effect::CreateSchedule(_) => "creating the schedule".into(),
            Effect::ToggleSchedule { id } => format!("toggling schedule {id}"),
            Effect::DeleteSchedule { id } => format!("deleting schedule {id}"),
            Effect::SelectBrewer { .. } => "switching brewer".into(),
            Effect::Copy { .. } => "copying to the clipboard".into(),
        }
    }
}

/// Results coming back from the runtime.
#[derive(Debug, Clone, PartialEq)]
#[allow(missing_docs)]
pub enum Msg {
    /// A refresh finished.
    Refreshed(Box<Snapshot>),
    /// An action succeeded; show `message` and refresh.
    Done { message: String },
    /// A new local profile exists; open it in the form.
    Created { name: String, draft: ProfileDraft },
    /// An action failed.
    Failed(UiError),
    /// The external editor returned.
    EditorClosed { name: String, ok: bool },
    /// A share link was created; copy it.
    Shared { link: String },
    /// A clipboard copy finished; `error` says why it failed.
    Copied { text: String, error: Option<String> },
}

/// Everything that can happen to the app.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// A key press.
    Key(KeyEvent),
    /// A result from an effect.
    Msg(Msg),
    /// Animation tick, sent while effects are in flight.
    Tick,
}

/// What a confirmation dialog will do on "yes".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfirmKind {
    /// Delete a local file only.
    DeleteLocal(String),
    /// Delete a remote-only profile.
    DeleteRemote(String),
    /// Delete a linked profile; `l` keeps the remote copy.
    DeleteBoth(String),
    /// Delete a schedule.
    DeleteSchedule(String),
    /// Push every local profile.
    PushAll,
    /// Pull every remote profile (overwrites linked local files).
    PullAll,
    /// Pull a profile whose local copy has unpushed changes.
    PullOverwrite(String),
}

/// A yes/no dialog.
#[derive(Debug, Clone, PartialEq)]
pub struct Confirm {
    /// Action on "yes".
    pub kind: ConfirmKind,
    /// Question shown to the user.
    pub text: String,
}

/// What the keyboard is currently driving.
#[derive(Debug, Clone, PartialEq)]
pub enum Mode {
    /// Browsing the panels.
    Normal,
    /// The key reference overlay.
    Help,
    /// Editing a profile.
    Profile(ProfileForm),
    /// Creating a schedule.
    Schedule(ScheduleForm),
    /// A one-line question.
    Prompt(Prompt),
    /// A yes/no question.
    Confirm(Confirm),
    /// Choosing a brewer.
    Brewer(List<DeviceRow>),
}

/// The whole UI state.
#[derive(Debug, Clone, PartialEq)]
pub struct App {
    /// Focused panel.
    pub focus: Panel,
    /// Visible profile rows (Fellow defaults filtered out unless `show_fellow`).
    pub profiles: List<ProfileRow>,
    /// Every profile row from the last refresh.
    pub all_profiles: Vec<ProfileRow>,
    /// Whether profiles shipped by Fellow are listed.
    pub show_fellow: bool,
    /// Unreadable local files.
    pub problems: Vec<LoadProblem>,
    /// Schedules.
    pub schedules: List<ScheduleView>,
    /// Brewers.
    pub devices: List<DeviceRow>,
    /// Whether the profile rows carry remote state.
    pub remote_ok: bool,
    /// Keyboard mode.
    pub mode: Mode,
    /// Status line message.
    pub message: Option<Message>,
    /// Effects in flight.
    pub busy: usize,
    /// What the most recently started effect is doing; cleared when idle.
    pub activity: Option<String>,
    /// Spinner frame, advanced by [`Event::Tick`].
    pub spinner: usize,
    /// Set when the app should exit.
    pub quit: bool,
    /// Whether the first refresh has completed.
    pub loaded: bool,
}

impl Default for App {
    fn default() -> Self {
        Self {
            focus: Panel::Profiles,
            profiles: List::default(),
            all_profiles: Vec::new(),
            show_fellow: false,
            problems: Vec::new(),
            schedules: List::default(),
            devices: List::default(),
            remote_ok: false,
            mode: Mode::Normal,
            message: None,
            busy: 0,
            activity: None,
            spinner: 0,
            quit: false,
            loaded: false,
        }
    }
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

impl App {
    /// The effects to run at startup.
    pub fn start(&mut self) -> Vec<Effect> {
        let effects = vec![Effect::Refresh];
        self.track(&effects);
        effects
    }

    fn track(&mut self, effects: &[Effect]) {
        self.busy += effects.len();
        if let Some(last) = effects.last() {
            self.activity = Some(last.label());
        }
        if self.busy == 0 {
            self.activity = None;
        }
    }

    /// Number of Fellow profiles currently hidden.
    pub fn hidden_fellow(&self) -> usize {
        if self.show_fellow {
            return 0;
        }
        self.all_profiles
            .iter()
            .filter(|r| r.origin() == Some(Origin::Fellow))
            .count()
    }

    /// Rebuilds the visible rows from `all_profiles`, keeping the selected profile when it is still shown.
    fn filter_profiles(&mut self) {
        let keep = self
            .profiles
            .current()
            .map(|r| (r.name.clone(), r.remote_id.clone()));
        let show = self.show_fellow;
        let rows: Vec<ProfileRow> = self
            .all_profiles
            .iter()
            .filter(|r| show || r.origin() != Some(Origin::Fellow))
            .cloned()
            .collect();
        let same = |r: &ProfileRow| match &keep {
            Some((Some(name), _)) => r.name.as_ref() == Some(name),
            Some((None, Some(id))) => r.remote_id.as_ref() == Some(id),
            _ => false,
        };
        let found = rows.iter().position(same);
        self.profiles.replace(rows);
        if let Some(i) = found {
            self.profiles.selected = i;
        }
    }

    /// The selected profile row.
    pub fn selected_profile(&self) -> Option<&ProfileRow> {
        self.profiles.current()
    }

    fn say(&mut self, text: impl Into<String>) {
        self.message = Some(Message {
            text: text.into(),
            error: false,
        });
    }

    fn fail(&mut self, text: impl Into<String>) {
        self.message = Some(Message {
            text: text.into(),
            error: true,
        });
    }

    fn open_picker(&mut self, devices: Vec<DeviceRow>) {
        let selected = devices.iter().position(|d| d.active).unwrap_or(0);
        self.mode = Mode::Brewer(List {
            items: devices,
            selected,
        });
    }

    fn apply(&mut self, s: Snapshot) -> Vec<Effect> {
        self.loaded = true;
        self.remote_ok = s.remote_ok;
        self.problems = s.problems;
        self.all_profiles = s.profiles;
        self.filter_profiles();
        if let Some(sc) = s.schedules {
            self.schedules.replace(sc);
        }
        if let Some(d) = s.devices {
            let active = d.iter().position(|r| r.active);
            self.devices.replace(d);
            if let Some(i) = active {
                self.devices.selected = i;
            }
        }
        let choices = s.errors.iter().find_map(|e| e.brewer_choices.clone());
        if let Some(first) = s.errors.first() {
            self.fail(first.message.clone());
        }
        if let (Some(devices), Mode::Normal) = (choices, &self.mode) {
            self.open_picker(
                devices
                    .into_iter()
                    .map(|device: Device| DeviceRow {
                        device,
                        active: false,
                    })
                    .collect(),
            );
        }
        Vec::new()
    }

    /// Applies one event and returns the work it triggers.
    pub fn update(&mut self, event: Event) -> Vec<Effect> {
        let effects = match event {
            Event::Key(k) => self.on_key(k),
            Event::Msg(m) => self.on_msg(m),
            Event::Tick => {
                self.spinner = self.spinner.wrapping_add(1);
                Vec::new()
            }
        };
        self.track(&effects);
        effects
    }

    fn on_msg(&mut self, msg: Msg) -> Vec<Effect> {
        self.busy = self.busy.saturating_sub(1);
        match msg {
            Msg::Refreshed(s) => self.apply(*s),
            Msg::Done { message } => {
                self.say(message);
                vec![Effect::Refresh]
            }
            Msg::Created { name, draft } => {
                self.say(format!("created {name}; fill in the details"));
                self.mode = Mode::Profile(ProfileForm::new(Some(name), &draft));
                vec![Effect::Refresh]
            }
            Msg::Failed(e) => {
                self.fail(e.message);
                if let (Some(devices), Mode::Normal) = (e.brewer_choices, &self.mode) {
                    self.open_picker(
                        devices
                            .into_iter()
                            .map(|device| DeviceRow {
                                device,
                                active: false,
                            })
                            .collect(),
                    );
                }
                // A failed action may still have changed things (e.g. a partial push).
                vec![Effect::Refresh]
            }
            Msg::Shared { link } => {
                self.say(format!("share link: {link}"));
                vec![Effect::Copy { text: link }]
            }
            Msg::Copied { text, error: None } => {
                self.say(format!("copied to clipboard: {text}"));
                Vec::new()
            }
            Msg::Copied {
                text,
                error: Some(e),
            } => {
                self.say(format!("share link: {text} (could not copy: {e})"));
                Vec::new()
            }
            Msg::EditorClosed { name, ok } => {
                if ok {
                    vec![Effect::Validate { name }]
                } else {
                    self.fail("the editor exited with an error");
                    Vec::new()
                }
            }
        }
    }

    fn on_key(&mut self, k: KeyEvent) -> Vec<Effect> {
        if k.modifiers.contains(KeyModifiers::CONTROL) && k.code == KeyCode::Char('c') {
            self.quit = true;
            return Vec::new();
        }
        self.message = None;
        match std::mem::replace(&mut self.mode, Mode::Normal) {
            Mode::Normal => self.on_normal_key(k),
            Mode::Help => Vec::new(),
            Mode::Profile(mut form) => match form.handle_key(k) {
                FormOutcome::Continue => {
                    self.mode = Mode::Profile(form);
                    Vec::new()
                }
                FormOutcome::Cancel => Vec::new(),
                FormOutcome::Submit => match form.draft() {
                    Ok(draft) => vec![Effect::SaveProfile {
                        name: form.name,
                        draft,
                    }],
                    Err(_) => {
                        self.fail("fix the highlighted fields first");
                        self.mode = Mode::Profile(form);
                        Vec::new()
                    }
                },
            },
            Mode::Schedule(mut form) => match form.handle_key(k) {
                FormOutcome::Continue => {
                    self.mode = Mode::Schedule(form);
                    Vec::new()
                }
                FormOutcome::Cancel => Vec::new(),
                FormOutcome::Submit => match form.new_schedule() {
                    Ok(new) => vec![Effect::CreateSchedule(new)],
                    Err(_) => {
                        self.fail("fix the highlighted fields first");
                        self.mode = Mode::Schedule(form);
                        Vec::new()
                    }
                },
            },
            Mode::Prompt(mut p) => match p.handle_key(k) {
                FormOutcome::Continue => {
                    self.mode = Mode::Prompt(p);
                    Vec::new()
                }
                FormOutcome::Cancel => Vec::new(),
                FormOutcome::Submit => {
                    let v = p.value.trim().to_owned();
                    vec![match p.kind {
                        PromptKind::NewProfile => Effect::NewProfile { title: v },
                        PromptKind::Import => Effect::Import { path: v },
                        PromptKind::Export(name) => Effect::Export { name, path: v },
                        PromptKind::ImportLink => Effect::ImportLink { link: v },
                    }]
                }
            },
            Mode::Confirm(c) => self.on_confirm_key(c, k),
            Mode::Brewer(mut list) => match k.code {
                KeyCode::Esc | KeyCode::Char('q') => Vec::new(),
                KeyCode::Down | KeyCode::Char('j') => {
                    list.step(1);
                    self.mode = Mode::Brewer(list);
                    Vec::new()
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    list.step(-1);
                    self.mode = Mode::Brewer(list);
                    Vec::new()
                }
                KeyCode::Enter => match list.current() {
                    Some(row) => vec![Effect::SelectBrewer {
                        id: row.device.id.clone(),
                    }],
                    None => Vec::new(),
                },
                _ => {
                    self.mode = Mode::Brewer(list);
                    Vec::new()
                }
            },
        }
    }

    fn on_confirm_key(&mut self, c: Confirm, k: KeyEvent) -> Vec<Effect> {
        let yes = matches!(k.code, KeyCode::Char('y') | KeyCode::Enter);
        match (&c.kind, k.code) {
            (ConfirmKind::DeleteBoth(name), KeyCode::Char('l')) => {
                return vec![Effect::DeleteLocal { name: name.clone() }];
            }
            (ConfirmKind::DeleteBoth(name), KeyCode::Char('b')) => {
                return vec![Effect::DeleteBoth { name: name.clone() }];
            }
            (_, KeyCode::Char('n') | KeyCode::Esc | KeyCode::Char('q')) => return Vec::new(),
            _ => {}
        }
        if !yes {
            self.mode = Mode::Confirm(c);
            return Vec::new();
        }
        vec![match c.kind {
            ConfirmKind::DeleteLocal(name) => Effect::DeleteLocal { name },
            ConfirmKind::DeleteRemote(id) => Effect::DeleteRemote { query: id },
            ConfirmKind::DeleteBoth(name) => Effect::DeleteBoth { name },
            ConfirmKind::DeleteSchedule(id) => Effect::DeleteSchedule { id },
            ConfirmKind::PushAll => Effect::PushAll,
            ConfirmKind::PullAll => Effect::PullAll,
            ConfirmKind::PullOverwrite(id) => Effect::Pull { remote_id: id },
        }]
    }

    fn confirm(&mut self, kind: ConfirmKind, text: impl Into<String>) {
        self.mode = Mode::Confirm(Confirm {
            kind,
            text: text.into(),
        });
    }

    fn on_normal_key(&mut self, k: KeyEvent) -> Vec<Effect> {
        match k.code {
            KeyCode::Char('q') => self.quit = true,
            KeyCode::Char('?') => self.mode = Mode::Help,
            KeyCode::Tab => self.focus = self.focus.next(),
            KeyCode::BackTab => self.focus = self.focus.prev(),
            KeyCode::Char('1') => self.focus = Panel::Profiles,
            KeyCode::Char('2') => self.focus = Panel::Schedules,
            KeyCode::Char('3') => self.focus = Panel::Device,
            KeyCode::Down | KeyCode::Char('j') => self.step(1),
            KeyCode::Up | KeyCode::Char('k') => self.step(-1),
            KeyCode::Home | KeyCode::Char('g') => self.edge(true),
            KeyCode::End | KeyCode::Char('G') => self.edge(false),
            KeyCode::Char('r') => return vec![Effect::Refresh],
            KeyCode::Char('b') => return self.pick_brewer(),
            _ => {
                return match self.focus {
                    Panel::Profiles => self.profile_key(k),
                    Panel::Schedules => self.schedule_key(k),
                    Panel::Device => self.device_key(k),
                };
            }
        }
        Vec::new()
    }

    fn step(&mut self, delta: isize) {
        match self.focus {
            Panel::Profiles => self.profiles.step(delta),
            Panel::Schedules => self.schedules.step(delta),
            Panel::Device => self.devices.step(delta),
        }
    }

    fn edge(&mut self, first: bool) {
        macro_rules! go {
            ($l:expr) => {
                if first { $l.first() } else { $l.last() }
            };
        }
        match self.focus {
            Panel::Profiles => go!(self.profiles),
            Panel::Schedules => go!(self.schedules),
            Panel::Device => go!(self.devices),
        }
    }

    fn pick_brewer(&mut self) -> Vec<Effect> {
        if self.devices.items.is_empty() {
            self.fail("no brewers loaded yet (press r to refresh)");
        } else {
            self.mode = Mode::Brewer(self.devices.clone());
        }
        Vec::new()
    }

    fn device_key(&mut self, k: KeyEvent) -> Vec<Effect> {
        if k.code == KeyCode::Enter {
            return self.pick_brewer();
        }
        Vec::new()
    }

    fn schedule_key(&mut self, k: KeyEvent) -> Vec<Effect> {
        match k.code {
            KeyCode::Char('n') => {
                let choices: Vec<(String, String)> = self
                    .all_profiles
                    .iter()
                    .filter_map(|r| {
                        r.remote_id
                            .clone()
                            .filter(|_| r.remote.is_some())
                            .map(|id| (id, r.title.clone()))
                    })
                    .collect();
                if choices.is_empty() {
                    self.fail("no profiles on the brewer yet; push one first (p in Profiles)");
                } else {
                    self.mode = Mode::Schedule(ScheduleForm::new(choices));
                }
                Vec::new()
            }
            KeyCode::Char('t') | KeyCode::Enter => match self.schedules.current() {
                Some(s) => vec![Effect::ToggleSchedule {
                    id: s.schedule.id.clone(),
                }],
                None => Vec::new(),
            },
            KeyCode::Char('d') => {
                if let Some(s) = self.schedules.current() {
                    let id = s.schedule.id.clone();
                    self.confirm(
                        ConfirmKind::DeleteSchedule(id.clone()),
                        format!("Delete schedule {id}? [y/n]"),
                    );
                }
                Vec::new()
            }
            _ => Vec::new(),
        }
    }

    fn profile_key(&mut self, k: KeyEvent) -> Vec<Effect> {
        let row = self.selected_profile().cloned();
        match k.code {
            KeyCode::Char('n') => {
                self.mode = Mode::Prompt(Prompt::new(
                    PromptKind::NewProfile,
                    "Title of the new profile",
                    "",
                ));
            }
            KeyCode::Char('i') => {
                self.mode = Mode::Prompt(Prompt::new(
                    PromptKind::Import,
                    "File to import (YAML or JSON)",
                    "",
                ));
            }
            KeyCode::Char('L') => {
                self.mode = Mode::Prompt(Prompt::new(
                    PromptKind::ImportLink,
                    "Share link or brew id",
                    "",
                ));
            }
            KeyCode::Char('f') => {
                self.show_fellow = !self.show_fellow;
                self.filter_profiles();
                self.say(if self.show_fellow {
                    "showing Fellow profiles"
                } else {
                    "hiding Fellow profiles"
                });
            }
            KeyCode::Char('U') => self.confirm(
                ConfirmKind::PushAll,
                "Push every local profile to the brewer? [y/n]",
            ),
            KeyCode::Char('D') => {
                self.confirm(
                    ConfirmKind::PullAll,
                    "Pull every remote profile (overwrites linked local files)? [y/n]",
                );
            }
            KeyCode::Char('e') | KeyCode::Enter => {
                match row.as_ref().and_then(|r| r.local.as_ref()) {
                    Some(l) => {
                        self.mode = Mode::Profile(ProfileForm::new(Some(l.name.clone()), &l.draft))
                    }
                    None => self.fail("no local copy; pull it first (P)"),
                }
            }
            KeyCode::Char('E') => match row.as_ref().and_then(|r| r.name.clone()) {
                Some(name) => return vec![Effect::OpenEditor { name }],
                None => self.fail("no local copy; pull it first (P)"),
            },
            KeyCode::Char('x') => match row.as_ref().and_then(|r| r.name.clone()) {
                Some(name) => {
                    let default = format!("{name}.yaml");
                    self.mode = Mode::Prompt(Prompt::new(
                        PromptKind::Export(name),
                        "Export to file",
                        default,
                    ));
                }
                None => self.fail("no local copy to export"),
            },
            KeyCode::Char('p') => match row.as_ref().and_then(|r| r.name.clone()) {
                Some(name) => return vec![Effect::Push { name }],
                None => self.fail("nothing local to push"),
            },
            KeyCode::Char('P') => match row {
                Some(ProfileRow {
                    state: Some(SyncState::Modified),
                    remote_id: Some(id),
                    title,
                    ..
                }) => {
                    self.confirm(
                        ConfirmKind::PullOverwrite(id),
                        format!("{title:?} has local changes; overwrite them with the remote version? [y/n]"),
                    );
                }
                Some(ProfileRow {
                    remote_id: Some(id),
                    remote: Some(_),
                    ..
                }) => {
                    return vec![Effect::Pull { remote_id: id }];
                }
                _ => self.fail("this profile is not on the brewer"),
            },
            KeyCode::Char('s') => match row.as_ref().and_then(|r| r.name.clone()) {
                Some(name) => return vec![Effect::Share { name }],
                None => self.fail("nothing to share"),
            },
            KeyCode::Char('d') => {
                if let Some(r) = row {
                    match (&r.name, &r.remote_id, r.state) {
                        (Some(name), Some(_), Some(SyncState::InSync | SyncState::Modified)) => {
                            self.confirm(
                                ConfirmKind::DeleteBoth(name.clone()),
                                format!(
                                    "Delete {:?}: [b]oth local and remote, [l]ocal only, [n]o",
                                    r.title
                                ),
                            )
                        }
                        (Some(name), _, _) => self.confirm(
                            ConfirmKind::DeleteLocal(name.clone()),
                            format!("Delete local file {name}? [y/n]"),
                        ),
                        (None, Some(id), _) => self.confirm(
                            ConfirmKind::DeleteRemote(id.clone()),
                            format!("Delete {:?} from the brewer? [y/n]", r.title),
                        ),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
        Vec::new()
    }
}

/// A fresh draft for a newly created profile title.
pub fn new_profile_draft(title: &str) -> ProfileDraft {
    template(title)
}

/// Convenience used by tests and the runtime.
pub fn press(code: KeyCode) -> Event {
    Event::Key(key(code))
}
