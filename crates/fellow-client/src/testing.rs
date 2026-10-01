//! An in-memory [`FellowApi`] for tests and demos (feature `testing`).

use std::collections::HashMap;
use std::sync::Mutex;

use async_trait::async_trait;
use serde_json::{Map, Value};

use crate::api::{FellowApi, brew_id_from_link};
use crate::error::{FellowError, Result};
use crate::models::{Device, Profile, ProfileDraft, Schedule, ScheduleDraft};

#[derive(Default)]
struct State {
    devices: Vec<Device>,
    selected: Option<String>,
    profiles: HashMap<String, Vec<Profile>>,
    schedules: HashMap<String, Vec<Schedule>>,
    shared: HashMap<String, Profile>,
    next_id: u64,
    login_calls: u32,
}

/// A fake Fellow backend that keeps everything in memory.
///
/// Behaviour mirrors the real API where the references document it: ids are
/// server-assigned, unknown ids give [`FellowError::NotFound`], and a schedule
/// that references a missing profile is rejected with a 400.
pub struct InMemoryFellow {
    state: Mutex<State>,
}

impl Default for InMemoryFellow {
    fn default() -> Self {
        Self::with_brewers(&[("b1", "Aiden")])
    }
}

impl InMemoryFellow {
    /// One brewer, `b1`, named "Aiden".
    pub fn new() -> Self {
        Self::default()
    }

    /// An account with the given `(id, display name)` brewers.
    pub fn with_brewers(brewers: &[(&str, &str)]) -> Self {
        let devices = brewers
            .iter()
            .map(|(id, name)| Device {
                id: (*id).into(),
                display_name: Some((*name).into()),
                extra: Map::new(),
            })
            .collect();
        Self {
            state: Mutex::new(State {
                devices,
                next_id: 1,
                ..State::default()
            }),
        }
    }

    /// Makes `draft` fetchable through `fetch_shared_profile(brew_id)`.
    pub fn seed_shared(&self, brew_id: &str, draft: &ProfileDraft) {
        let mut st = self.state.lock().unwrap();
        let mut profile = Profile::from_draft("shared-origin", draft);
        // Shared profiles carry server-managed fields; keep that realistic.
        profile
            .extra
            .insert("sharedFrom".into(), Value::String(brew_id.into()));
        profile.extra.insert(
            "createdAt".into(),
            Value::String("2024-01-01T00:00:00Z".into()),
        );
        st.shared.insert(brew_id.into(), profile);
    }

    /// How many times `login` was called.
    pub fn login_calls(&self) -> u32 {
        self.state.lock().unwrap().login_calls
    }

    fn brewer(st: &State) -> Result<String> {
        if let Some(id) = &st.selected {
            return Ok(id.clone());
        }
        match st.devices.len() {
            0 => Err(FellowError::NoDevices),
            1 => Ok(st.devices[0].id.clone()),
            _ => Err(FellowError::BrewerNotSelected(st.devices.clone())),
        }
    }

    fn known_brewer(st: &State) -> Result<String> {
        let id = Self::brewer(st)?;
        if st.devices.iter().any(|d| d.id == id) {
            Ok(id)
        } else {
            Err(FellowError::NotFound(format!("device {id}")))
        }
    }

    fn fresh_id(st: &mut State, prefix: &str) -> String {
        let id = format!("{prefix}{}", st.next_id);
        st.next_id += 1;
        id
    }
}

#[async_trait]
impl FellowApi for InMemoryFellow {
    async fn login(&self) -> Result<()> {
        self.state.lock().unwrap().login_calls += 1;
        Ok(())
    }

    async fn devices(&self) -> Result<Vec<Device>> {
        Ok(self.state.lock().unwrap().devices.clone())
    }

    fn select_brewer(&self, id: &str) {
        self.state.lock().unwrap().selected = Some(id.into());
    }

    fn selected_brewer(&self) -> Option<String> {
        let st = self.state.lock().unwrap();
        st.selected.clone().or_else(|| match st.devices.as_slice() {
            [only] => Some(only.id.clone()),
            _ => None,
        })
    }

    async fn active_brewer(&self) -> Result<String> {
        Self::brewer(&self.state.lock().unwrap())
    }

    async fn adjust_setting(&self, setting: &str, value: Value) -> Result<Value> {
        let mut st = self.state.lock().unwrap();
        let id = Self::known_brewer(&st)?;
        let device = st.devices.iter_mut().find(|d| d.id == id).expect("known");
        device.extra.insert(setting.into(), value);
        Ok(Value::Null)
    }

    async fn profiles(&self) -> Result<Vec<Profile>> {
        let st = self.state.lock().unwrap();
        let id = Self::known_brewer(&st)?;
        Ok(st.profiles.get(&id).cloned().unwrap_or_default())
    }

    async fn create_profile(&self, draft: &ProfileDraft) -> Result<Profile> {
        draft.validate()?;
        let mut st = self.state.lock().unwrap();
        let brewer = Self::known_brewer(&st)?;
        let profile = Profile::from_draft(Self::fresh_id(&mut st, "p"), draft);
        st.profiles.entry(brewer).or_default().push(profile.clone());
        Ok(profile)
    }

    async fn update_profile(&self, profile_id: &str, draft: &ProfileDraft) -> Result<Profile> {
        draft.validate()?;
        let mut st = self.state.lock().unwrap();
        let brewer = Self::known_brewer(&st)?;
        let slot = st
            .profiles
            .entry(brewer)
            .or_default()
            .iter_mut()
            .find(|p| p.id == profile_id)
            .ok_or_else(|| FellowError::NotFound(format!("profile {profile_id}")))?;
        *slot = Profile::from_draft(profile_id, draft);
        Ok(slot.clone())
    }

    async fn delete_profile(&self, profile_id: &str) -> Result<()> {
        let mut st = self.state.lock().unwrap();
        let brewer = Self::known_brewer(&st)?;
        let list = st.profiles.entry(brewer).or_default();
        let before = list.len();
        list.retain(|p| p.id != profile_id);
        if list.len() == before {
            return Err(FellowError::NotFound(format!("profile {profile_id}")));
        }
        Ok(())
    }

    async fn share_profile(&self, profile_id: &str) -> Result<String> {
        let mut st = self.state.lock().unwrap();
        let brewer = Self::known_brewer(&st)?;
        let profile = st
            .profiles
            .entry(brewer)
            .or_default()
            .iter()
            .find(|p| p.id == profile_id)
            .cloned()
            .ok_or_else(|| FellowError::NotFound(format!("profile {profile_id}")))?;
        let brew_id = format!("share{}", st.next_id);
        st.next_id += 1;
        st.shared.insert(brew_id.clone(), profile);
        Ok(format!("https://fellowproducts.com/p/{brew_id}"))
    }

    async fn fetch_shared_profile(&self, link_or_id: &str) -> Result<Profile> {
        let id = brew_id_from_link(link_or_id)?;
        self.state
            .lock()
            .unwrap()
            .shared
            .get(&id)
            .cloned()
            .ok_or_else(|| FellowError::NotFound(format!("shared profile {id}")))
    }

    async fn schedules(&self) -> Result<Vec<Schedule>> {
        let st = self.state.lock().unwrap();
        let id = Self::known_brewer(&st)?;
        Ok(st.schedules.get(&id).cloned().unwrap_or_default())
    }

    async fn create_schedule(&self, draft: &ScheduleDraft) -> Result<Schedule> {
        draft.validate()?;
        let mut st = self.state.lock().unwrap();
        let brewer = Self::known_brewer(&st)?;
        let profile_exists = st
            .profiles
            .get(&brewer)
            .is_some_and(|ps| ps.iter().any(|p| p.id == draft.profile_id));
        if !profile_exists {
            return Err(FellowError::Api {
                status: 400,
                body: "Profile could not be found".into(),
            });
        }
        let schedule = Schedule {
            id: Self::fresh_id(&mut st, "s"),
            draft: draft.clone(),
            extra: Map::new(),
        };
        st.schedules
            .entry(brewer)
            .or_default()
            .push(schedule.clone());
        Ok(schedule)
    }

    async fn toggle_schedule(&self, schedule_id: &str, enabled: bool) -> Result<()> {
        let mut st = self.state.lock().unwrap();
        let brewer = Self::known_brewer(&st)?;
        let s = st
            .schedules
            .entry(brewer)
            .or_default()
            .iter_mut()
            .find(|s| s.id == schedule_id)
            .ok_or_else(|| FellowError::NotFound(format!("schedule {schedule_id}")))?;
        s.draft.enabled = enabled;
        Ok(())
    }

    async fn delete_schedule(&self, schedule_id: &str) -> Result<()> {
        let mut st = self.state.lock().unwrap();
        let brewer = Self::known_brewer(&st)?;
        let list = st.schedules.entry(brewer).or_default();
        let before = list.len();
        list.retain(|s| s.id != schedule_id);
        if list.len() == before {
            return Err(FellowError::NotFound(format!("schedule {schedule_id}")));
        }
        Ok(())
    }
}
