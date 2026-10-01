use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::sync::Arc;

use fellow_client::{FellowApi, Profile, ProfileDraft};
use serde_json::Value;

use crate::error::{ProfileError, Result, StoreError};
use crate::local::{LoadProblem, LocalListing, LocalProfile, slugify};
use crate::store::{Links, ProfileStore};

/// How a local profile relates to the selected brewer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncState {
    /// Exists locally only; never pushed to this brewer.
    LocalOnly,
    /// Linked and identical on both sides.
    InSync,
    /// Linked, but local and remote differ.
    Modified,
    /// Linked, but the remote profile no longer exists.
    RemoteMissing,
    /// Exists on the brewer only.
    RemoteOnly,
}

/// One row of the status report.
#[derive(Debug, Clone, PartialEq)]
pub struct ProfileStatus {
    /// Local name (file stem); `None` for [`SyncState::RemoteOnly`].
    pub name: Option<String>,
    /// Display title.
    pub title: String,
    /// Remote profile id when known.
    pub remote_id: Option<String>,
    /// The comparison result.
    pub state: SyncState,
    /// The local profile, when there is one.
    pub local: Option<LocalProfile>,
    /// The remote profile, when there is one.
    pub remote: Option<Profile>,
}

/// Local-vs-remote overview for the selected brewer.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StatusReport {
    /// Local profiles (sorted by name) followed by remote-only ones.
    pub entries: Vec<ProfileStatus>,
    /// Local files that could not be read.
    pub problems: Vec<LoadProblem>,
}

/// One differing field between local and remote.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldChange {
    /// API field name.
    pub field: String,
    /// Local value.
    pub local: Value,
    /// Remote value.
    pub remote: Value,
}

/// What a push did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PushAction {
    /// A new remote profile was created.
    Created,
    /// An existing remote profile was changed.
    Updated,
    /// Remote already matched; nothing sent.
    Unchanged,
}

/// Result of pushing one profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PushOutcome {
    /// Local name.
    pub name: String,
    /// Remote id now linked to it.
    pub remote_id: String,
    /// What happened.
    pub action: PushAction,
}

/// What a pull did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PullAction {
    /// A new local file was written.
    Created,
    /// The linked local file was overwritten.
    Updated,
    /// The linked local file already matched.
    Unchanged,
}

/// Result of pulling one profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullOutcome {
    /// Local name.
    pub name: String,
    /// Remote id.
    pub remote_id: String,
    /// What happened.
    pub action: PullAction,
}

/// Options for importing a profile document.
#[derive(Debug, Clone, Default)]
pub struct ImportOptions {
    /// Local name to use instead of one derived from the title.
    pub name: Option<String>,
    /// Replace an existing profile with the same name/title instead of failing.
    pub overwrite: bool,
}

/// Business logic for local and remote profiles.
///
/// The brewer protocol and the local store are injected; the service never
/// constructs either.
pub struct ProfileService {
    api: Arc<dyn FellowApi>,
    store: Arc<dyn ProfileStore>,
}

fn check(draft: &ProfileDraft) -> Result<()> {
    let issues = draft.issues();
    if issues.is_empty() {
        Ok(())
    } else {
        Err(ProfileError::Validation(issues))
    }
}

fn parse_yaml(text: &str) -> Result<ProfileDraft> {
    serde_yaml_ng::from_str(text).map_err(|e| {
        ProfileError::Parse(match e.location() {
            Some(l) => format!("line {}, column {}: {e}", l.line(), l.column()),
            None => e.to_string(),
        })
    })
}

fn find_remote<'a>(remote: &'a [Profile], query: &str) -> Result<&'a Profile> {
    if let Some(p) = remote.iter().find(|p| p.id == query) {
        return Ok(p);
    }
    let by_title: Vec<&Profile> = remote
        .iter()
        .filter(|p| p.title.eq_ignore_ascii_case(query))
        .collect();
    match by_title.as_slice() {
        [] => Err(ProfileError::NotFound(query.into())),
        [one] => Ok(one),
        many => Err(ProfileError::Ambiguous {
            query: query.into(),
            matches: many.iter().map(|p| p.id.clone()).collect(),
        }),
    }
}

impl ProfileService {
    /// Creates the service from its two collaborators.
    pub fn new(api: Arc<dyn FellowApi>, store: Arc<dyn ProfileStore>) -> Self {
        Self { api, store }
    }

    // ---- local -----------------------------------------------------------

    /// All local profiles plus any unreadable files.
    pub fn list_local(&self) -> Result<LocalListing> {
        Ok(self.store.list()?)
    }

    /// Finds a local profile by file name, else by case-insensitive title.
    pub fn get_local(&self, query: &str) -> Result<LocalProfile> {
        if self.store.exists(query) {
            return Ok(self.store.load(query)?);
        }
        let listing = self.store.list()?;
        let matches: Vec<&LocalProfile> = listing
            .profiles
            .iter()
            .filter(|p| p.draft.title.eq_ignore_ascii_case(query))
            .collect();
        match matches.as_slice() {
            [] => Err(ProfileError::NotFound(query.into())),
            [one] => Ok((*one).clone()),
            many => Err(ProfileError::Ambiguous {
                query: query.into(),
                matches: many.iter().map(|p| p.name.clone()).collect(),
            }),
        }
    }

    /// On-disk location of a local profile (by name or title), for opening an editor.
    pub fn path_of(&self, query: &str) -> Result<std::path::PathBuf> {
        let profile = self.get_local(query)?;
        self.store
            .path_of(&profile.name)
            .ok_or(ProfileError::NotFound(query.into()))
    }

    /// Re-reads a local profile and checks it against the brewer's rules; use after
    /// the file was edited by hand.
    pub fn validate_local(&self, query: &str) -> Result<LocalProfile> {
        let profile = self.get_local(query)?;
        check(&profile.draft)?;
        Ok(profile)
    }

    fn unique_name(&self, base: &str) -> String {
        if !self.store.exists(base) {
            return base.into();
        }
        (2..)
            .map(|n| format!("{base}-{n}"))
            .find(|c| !self.store.exists(c))
            .expect("infinite range")
    }

    /// Validates and stores a new profile under a name derived from its title.
    pub fn create_local(&self, draft: ProfileDraft) -> Result<LocalProfile> {
        check(&draft)?;
        let profile = LocalProfile {
            name: self.unique_name(&slugify(&draft.title)),
            draft,
        };
        self.store.save(&profile)?;
        Ok(profile)
    }

    /// Validates and replaces the data of an existing local profile.
    pub fn update_local(&self, name: &str, draft: ProfileDraft) -> Result<LocalProfile> {
        check(&draft)?;
        if !self.store.exists(name) {
            return Err(StoreError::NotFound(name.into()).into());
        }
        let profile = LocalProfile {
            name: name.into(),
            draft,
        };
        self.store.save(&profile)?;
        Ok(profile)
    }

    /// Deletes a local profile and forgets its remote links (the remote copy stays).
    pub fn delete_local(&self, query: &str) -> Result<()> {
        let profile = self.get_local(query)?;
        self.store.delete(&profile.name)?;
        let mut links = self.store.links()?;
        links.forget(&profile.name);
        self.store.save_links(&links)?;
        Ok(())
    }

    /// Imports a YAML (or JSON) profile document.
    pub fn import_str(&self, text: &str, options: &ImportOptions) -> Result<LocalProfile> {
        let draft = parse_yaml(text)?;
        check(&draft)?;
        let listing = self.store.list()?;
        let existing = match &options.name {
            Some(name) => self.store.exists(name).then(|| name.clone()),
            None => listing
                .profiles
                .iter()
                .find(|p| p.draft.title.eq_ignore_ascii_case(&draft.title))
                .map(|p| p.name.clone()),
        };
        let name = match existing {
            Some(name) if options.overwrite => name,
            Some(name) => {
                return Err(ProfileError::Conflict(format!(
                    "local profile {name:?} already exists; use overwrite to replace it"
                )));
            }
            None => match &options.name {
                Some(name) => name.clone(),
                None => self.unique_name(&slugify(&draft.title)),
            },
        };
        let profile = LocalProfile { name, draft };
        self.store.save(&profile)?;
        Ok(profile)
    }

    /// Imports a profile document from a file.
    pub fn import_file(&self, path: &Path, options: &ImportOptions) -> Result<LocalProfile> {
        let text = fs::read_to_string(path).map_err(|source| StoreError::Io {
            path: path.into(),
            source,
        })?;
        self.import_str(&text, options)
    }

    /// Renders a local profile as YAML (no local bookkeeping, ready to share).
    pub fn export_str(&self, query: &str) -> Result<String> {
        let profile = self.get_local(query)?;
        serde_yaml_ng::to_string(&profile.draft).map_err(|e| ProfileError::Parse(e.to_string()))
    }

    /// Writes a local profile as YAML to `path`.
    pub fn export_file(&self, query: &str, path: &Path) -> Result<()> {
        let text = self.export_str(query)?;
        fs::write(path, text).map_err(|source| StoreError::Io {
            path: path.into(),
            source,
        })?;
        Ok(())
    }

    // ---- remote ----------------------------------------------------------

    /// Profiles currently on the selected brewer.
    pub async fn list_remote(&self) -> Result<Vec<Profile>> {
        Ok(self.api.profiles().await?)
    }

    /// Compares every local profile with the selected brewer.
    pub async fn status(&self) -> Result<StatusReport> {
        let remote = self.api.profiles().await?;
        let brewer = self.api.active_brewer().await?;
        let listing = self.store.list()?;
        let links = self.store.links()?;

        let mut entries = Vec::new();
        let mut claimed: HashSet<String> = HashSet::new();
        let local_names = listing
            .problems
            .iter()
            .map(|p| &p.name)
            .chain(listing.profiles.iter().map(|p| &p.name));
        for name in local_names {
            if let Some(id) = links.remote_id(&brewer, name) {
                claimed.insert(id.to_owned());
            }
        }
        for local in &listing.profiles {
            let rid = links.remote_id(&brewer, &local.name).map(str::to_owned);
            let remote_p = rid
                .as_deref()
                .and_then(|id| remote.iter().find(|r| r.id == id));
            let state = match (&rid, remote_p) {
                (None, _) => SyncState::LocalOnly,
                (Some(_), None) => SyncState::RemoteMissing,
                (Some(_), Some(r)) => {
                    if r.draft().is_ok_and(|d| d == local.draft) {
                        SyncState::InSync
                    } else {
                        SyncState::Modified
                    }
                }
            };
            entries.push(ProfileStatus {
                name: Some(local.name.clone()),
                title: local.draft.title.clone(),
                remote_id: rid,
                state,
                local: Some(local.clone()),
                remote: remote_p.cloned(),
            });
        }
        for r in remote.iter().filter(|r| !claimed.contains(&r.id)) {
            entries.push(ProfileStatus {
                name: None,
                title: r.title.clone(),
                remote_id: Some(r.id.clone()),
                state: SyncState::RemoteOnly,
                local: None,
                remote: Some(r.clone()),
            });
        }
        Ok(StatusReport {
            entries,
            problems: listing.problems,
        })
    }

    /// Field-level differences between a linked local profile and its remote copy.
    pub async fn diff(&self, query: &str) -> Result<Vec<FieldChange>> {
        let local = self.get_local(query)?;
        let brewer = self.api.active_brewer().await?;
        let links = self.store.links()?;
        let rid = links
            .remote_id(&brewer, &local.name)
            .ok_or_else(|| ProfileError::NotPushed(local.name.clone()))?;
        let remote = self.api.profiles().await?;
        let remote = remote
            .iter()
            .find(|r| r.id == rid)
            .ok_or_else(|| ProfileError::NotFound(rid.to_owned()))?;
        let to_map = |d: &ProfileDraft| match serde_json::to_value(d) {
            Ok(Value::Object(m)) => m,
            _ => unreachable!("ProfileDraft serializes to an object"),
        };
        let (l, r) = (to_map(&local.draft), to_map(&remote.draft()?));
        Ok(l.into_iter()
            .filter_map(|(field, lv)| {
                let rv = r.get(&field).cloned().unwrap_or(Value::Null);
                (lv != rv).then_some(FieldChange {
                    field,
                    local: lv,
                    remote: rv,
                })
            })
            .collect())
    }

    async fn push_one(
        &self,
        brewer: &str,
        remote: &[Profile],
        local: &LocalProfile,
        links: &mut Links,
    ) -> Result<PushOutcome> {
        check(&local.draft)?;
        let linked = links
            .remote_id(brewer, &local.name)
            .and_then(|id| remote.iter().find(|r| r.id == id));
        let (remote_id, action) = match linked {
            Some(r) if r.draft().is_ok_and(|d| d == local.draft) => {
                (r.id.clone(), PushAction::Unchanged)
            }
            Some(r) => {
                self.api.update_profile(&r.id, &local.draft).await?;
                (r.id.clone(), PushAction::Updated)
            }
            None => {
                let created = self.api.create_profile(&local.draft).await?;
                (created.id, PushAction::Created)
            }
        };
        links.link(brewer, &local.name, &remote_id);
        self.store.save_links(links)?;
        Ok(PushOutcome {
            name: local.name.clone(),
            remote_id,
            action,
        })
    }

    /// Pushes one local profile (by name or title) to the selected brewer.
    pub async fn push(&self, query: &str) -> Result<PushOutcome> {
        let local = self.get_local(query)?;
        let brewer = self.api.active_brewer().await?;
        let remote = self.api.profiles().await?;
        let mut links = self.store.links()?;
        self.push_one(&brewer, &remote, &local, &mut links).await
    }

    /// Pushes every local profile; one failure does not stop the others.
    pub async fn push_all(&self) -> Result<Vec<(String, Result<PushOutcome>)>> {
        let brewer = self.api.active_brewer().await?;
        let remote = self.api.profiles().await?;
        let listing = self.store.list()?;
        let mut links = self.store.links()?;
        let mut out = Vec::new();
        for local in &listing.profiles {
            let r = self.push_one(&brewer, &remote, local, &mut links).await;
            out.push((local.name.clone(), r));
        }
        Ok(out)
    }

    fn pull_one(&self, brewer: &str, remote: &Profile, links: &mut Links) -> Result<PullOutcome> {
        let draft = remote.draft()?;
        if let Some(name) = links.name_for(brewer, &remote.id).map(str::to_owned)
            && self.store.exists(&name)
        {
            let unchanged = self.store.load(&name).is_ok_and(|l| l.draft == draft);
            if !unchanged {
                self.store.save(&LocalProfile {
                    name: name.clone(),
                    draft,
                })?;
            }
            let action = if unchanged {
                PullAction::Unchanged
            } else {
                PullAction::Updated
            };
            return Ok(PullOutcome {
                name,
                remote_id: remote.id.clone(),
                action,
            });
        }
        let name = self.unique_name(&slugify(&draft.title));
        self.store.save(&LocalProfile {
            name: name.clone(),
            draft,
        })?;
        links.link(brewer, &name, &remote.id);
        self.store.save_links(links)?;
        Ok(PullOutcome {
            name,
            remote_id: remote.id.clone(),
            action: PullAction::Created,
        })
    }

    /// Pulls one remote profile (by id or title) into the local store.
    pub async fn pull(&self, query: &str) -> Result<PullOutcome> {
        let brewer = self.api.active_brewer().await?;
        let remote = self.api.profiles().await?;
        let target = find_remote(&remote, query)?;
        let mut links = self.store.links()?;
        self.pull_one(&brewer, target, &mut links)
    }

    /// Pulls every remote profile; one failure does not stop the others.
    pub async fn pull_all(&self) -> Result<Vec<(String, Result<PullOutcome>)>> {
        let brewer = self.api.active_brewer().await?;
        let remote = self.api.profiles().await?;
        let mut links = self.store.links()?;
        Ok(remote
            .iter()
            .map(|r| (r.title.clone(), self.pull_one(&brewer, r, &mut links)))
            .collect())
    }

    /// Deletes a profile on the brewer, given a linked local name, a remote id or a title.
    /// The local file is kept (and becomes unlinked). Returns the deleted remote id.
    pub async fn delete_remote(&self, query: &str) -> Result<String> {
        let brewer = self.api.active_brewer().await?;
        let remote = self.api.profiles().await?;
        let mut links = self.store.links()?;
        let rid = match links.remote_id(&brewer, query) {
            Some(id) => id.to_owned(),
            None => find_remote(&remote, query)?.id.clone(),
        };
        self.api.delete_profile(&rid).await?;
        links.unlink_remote(&brewer, &rid);
        self.store.save_links(&links)?;
        Ok(rid)
    }

    /// Creates a share link for a profile that has been pushed.
    pub async fn share_link(&self, query: &str) -> Result<String> {
        let local = self.get_local(query)?;
        let brewer = self.api.active_brewer().await?;
        let links = self.store.links()?;
        let rid = links
            .remote_id(&brewer, &local.name)
            .ok_or_else(|| ProfileError::NotPushed(local.name.clone()))?;
        Ok(self.api.share_profile(rid).await?)
    }

    /// Fetches a shared profile and stores it locally (it is not pushed).
    pub async fn import_from_link(&self, link: &str) -> Result<LocalProfile> {
        let shared = self.api.fetch_shared_profile(link).await?;
        self.create_local(shared.draft()?)
    }
}
