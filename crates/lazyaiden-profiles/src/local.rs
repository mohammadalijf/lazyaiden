use fellow_client::ProfileDraft;

/// A profile stored locally. `name` is the file stem (`morning` for `morning.yaml`).
#[derive(Debug, Clone, PartialEq)]
pub struct LocalProfile {
    /// File stem; the stable local identity of the profile.
    pub name: String,
    /// The profile data.
    pub draft: ProfileDraft,
}

/// A file in the store that could not be loaded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadProblem {
    /// File stem.
    pub name: String,
    /// Why loading failed (includes line/column for YAML errors).
    pub message: String,
}

/// Result of listing the local store: good profiles plus files that failed to load.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LocalListing {
    /// Successfully loaded profiles, sorted by name.
    pub profiles: Vec<LocalProfile>,
    /// Unreadable files, so UIs can surface them instead of silently hiding them.
    pub problems: Vec<LoadProblem>,
}

/// Turns a title into a file-name-safe slug: lowercase ASCII alphanumerics
/// separated by single dashes; `"profile"` when nothing is left.
pub fn slugify(title: &str) -> String {
    let mut out = String::new();
    let mut pending_dash = false;
    for c in title.chars() {
        if c.is_ascii_alphanumeric() {
            if pending_dash && !out.is_empty() {
                out.push('-');
            }
            pending_dash = false;
            out.push(c.to_ascii_lowercase());
        } else {
            pending_dash = true;
        }
    }
    if out.is_empty() {
        "profile".into()
    } else {
        out
    }
}

/// A sensible starting point for a new profile.
pub fn template(title: &str) -> ProfileDraft {
    ProfileDraft {
        profile_type: 0,
        title: title.into(),
        ratio: 16.0,
        bloom_enabled: true,
        bloom_ratio: 2.0,
        bloom_duration: 30,
        bloom_temperature: 96.0,
        ss_pulses_enabled: true,
        ss_pulses_number: 3,
        ss_pulses_interval: 20,
        ss_pulse_temperatures: vec![96.0; 3],
        batch_pulses_enabled: true,
        batch_pulses_number: 3,
        batch_pulses_interval: 20,
        batch_pulse_temperatures: vec![96.0; 3],
    }
}
