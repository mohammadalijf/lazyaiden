use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use fellow_client::ProfileDraft;
use serde::{Deserialize, Serialize};

use crate::error::StoreError;
use crate::local::{LoadProblem, LocalListing, LocalProfile};

type Result<T> = std::result::Result<T, StoreError>;

/// Which local profile corresponds to which remote profile, per brewer.
///
/// Serialized as `brewer id -> { local name -> remote profile id }`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Links(BTreeMap<String, BTreeMap<String, String>>);

impl Links {
    /// Remote id linked to `name` on `brewer`.
    pub fn remote_id(&self, brewer: &str, name: &str) -> Option<&str> {
        self.0.get(brewer)?.get(name).map(String::as_str)
    }

    /// Local name linked to `remote_id` on `brewer`.
    pub fn name_for(&self, brewer: &str, remote_id: &str) -> Option<&str> {
        self.0
            .get(brewer)?
            .iter()
            .find(|(_, rid)| rid.as_str() == remote_id)
            .map(|(name, _)| name.as_str())
    }

    /// Links `name` to `remote_id` on `brewer`, replacing any previous link of either side.
    pub fn link(&mut self, brewer: &str, name: &str, remote_id: &str) {
        let map = self.0.entry(brewer.into()).or_default();
        map.retain(|n, rid| n == name || rid != remote_id);
        map.insert(name.into(), remote_id.into());
    }

    /// Removes the link of `name` on `brewer`.
    pub fn unlink(&mut self, brewer: &str, name: &str) {
        if let Some(map) = self.0.get_mut(brewer) {
            map.remove(name);
            if map.is_empty() {
                self.0.remove(brewer);
            }
        }
    }

    /// Removes every link pointing at `remote_id` on `brewer`.
    pub fn unlink_remote(&mut self, brewer: &str, remote_id: &str) {
        if let Some(map) = self.0.get_mut(brewer) {
            map.retain(|_, rid| rid != remote_id);
            if map.is_empty() {
                self.0.remove(brewer);
            }
        }
    }

    /// Removes all links of `name` on every brewer.
    pub fn forget(&mut self, name: &str) {
        self.0.values_mut().for_each(|m| {
            m.remove(name);
        });
        self.0.retain(|_, m| !m.is_empty());
    }
}

/// Persistence for local profiles. Implementations must be safe to share.
pub trait ProfileStore: Send + Sync {
    /// Loads every profile; unreadable files are reported, not fatal.
    fn list(&self) -> Result<LocalListing>;
    /// Loads one profile by name.
    fn load(&self, name: &str) -> Result<LocalProfile>;
    /// Whether a profile with this name exists (even if unreadable).
    fn exists(&self, name: &str) -> bool;
    /// Writes (creates or replaces) a profile.
    fn save(&self, profile: &LocalProfile) -> Result<()>;
    /// Removes a profile.
    fn delete(&self, name: &str) -> Result<()>;
    /// On-disk location of a profile, if the store is file based (used to open an editor).
    fn path_of(&self, name: &str) -> Option<PathBuf>;
    /// Reads the remote links.
    fn links(&self) -> Result<Links>;
    /// Writes the remote links.
    fn save_links(&self, links: &Links) -> Result<()>;
}

/// Stores one `<name>.yaml` per profile in a directory.
///
/// Files are written atomically (temp file + rename). Hidden files (leading
/// dot) are ignored when listing; `.lazyaiden-links.yaml` holds the remote links.
#[derive(Debug, Clone)]
pub struct FsProfileStore {
    dir: PathBuf,
}

const LINKS_FILE: &str = ".lazyaiden-links.yaml";

fn io(path: &Path) -> impl FnOnce(std::io::Error) -> StoreError + '_ {
    move |source| StoreError::Io {
        path: path.into(),
        source,
    }
}

fn parse_error(path: &Path, e: &serde_yaml_ng::Error) -> StoreError {
    let message = match e.location() {
        Some(loc) => format!("line {}, column {}: {e}", loc.line(), loc.column()),
        None => e.to_string(),
    };
    StoreError::Parse {
        path: path.into(),
        message,
    }
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('.')
        && !name.contains("..")
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}

impl FsProfileStore {
    /// A store rooted at `dir` (created on first write).
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// The directory holding the profiles.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn file(&self, name: &str) -> Result<PathBuf> {
        if !valid_name(name) {
            return Err(StoreError::InvalidName(name.into()));
        }
        let yaml = self.dir.join(format!("{name}.yaml"));
        let yml = self.dir.join(format!("{name}.yml"));
        Ok(if !yaml.exists() && yml.exists() {
            yml
        } else {
            yaml
        })
    }

    fn read(path: &Path, name: &str) -> Result<LocalProfile> {
        let text = fs::read_to_string(path).map_err(io(path))?;
        let draft: ProfileDraft =
            serde_yaml_ng::from_str(&text).map_err(|e| parse_error(path, &e))?;
        Ok(LocalProfile {
            name: name.into(),
            draft,
        })
    }

    fn write_atomic(&self, path: &Path, text: &str) -> Result<()> {
        fs::create_dir_all(&self.dir).map_err(io(&self.dir))?;
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("profile");
        let tmp = self.dir.join(format!(".{file_name}.tmp"));
        fs::write(&tmp, text).map_err(io(&tmp))?;
        fs::rename(&tmp, path).map_err(io(path))
    }
}

impl ProfileStore for FsProfileStore {
    fn list(&self) -> Result<LocalListing> {
        let mut listing = LocalListing::default();
        let entries = match fs::read_dir(&self.dir) {
            Ok(e) => e,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(listing),
            Err(e) => return Err(io(&self.dir)(e)),
        };
        let mut paths: Vec<PathBuf> = entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| {
                p.is_file()
                    && p.extension().is_some_and(|x| x == "yaml" || x == "yml")
                    && p.file_name()
                        .is_some_and(|n| !n.to_string_lossy().starts_with('.'))
            })
            .collect();
        paths.sort();
        for path in paths {
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or_default()
                .to_owned();
            match Self::read(&path, &name) {
                Ok(p) => listing.profiles.push(p),
                Err(e) => listing.problems.push(LoadProblem {
                    name,
                    message: e.to_string(),
                }),
            }
        }
        Ok(listing)
    }

    fn load(&self, name: &str) -> Result<LocalProfile> {
        let path = self.file(name)?;
        if !path.exists() {
            return Err(StoreError::NotFound(name.into()));
        }
        Self::read(&path, name)
    }

    fn exists(&self, name: &str) -> bool {
        self.file(name).is_ok_and(|p| p.exists())
    }

    fn save(&self, profile: &LocalProfile) -> Result<()> {
        let path = self.file(&profile.name)?;
        let text = serde_yaml_ng::to_string(&profile.draft).map_err(|e| parse_error(&path, &e))?;
        self.write_atomic(&path, &text)
    }

    fn delete(&self, name: &str) -> Result<()> {
        let path = self.file(name)?;
        if !path.exists() {
            return Err(StoreError::NotFound(name.into()));
        }
        fs::remove_file(&path).map_err(io(&path))
    }

    fn path_of(&self, name: &str) -> Option<PathBuf> {
        self.file(name).ok()
    }

    fn links(&self) -> Result<Links> {
        let path = self.dir.join(LINKS_FILE);
        match fs::read_to_string(&path) {
            Ok(text) if text.trim().is_empty() => Ok(Links::default()),
            Ok(text) => serde_yaml_ng::from_str(&text).map_err(|e| parse_error(&path, &e)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Links::default()),
            Err(e) => Err(io(&path)(e)),
        }
    }

    fn save_links(&self, links: &Links) -> Result<()> {
        let path = self.dir.join(LINKS_FILE);
        let text = serde_yaml_ng::to_string(links).map_err(|e| parse_error(&path, &e))?;
        self.write_atomic(&path, &text)
    }
}
