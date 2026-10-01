//! Plain-text rendering helpers.

use lazyaiden_core::profiles::{PullAction, PushAction, SyncState};

/// Left-aligned columns separated by two spaces; the last column is not padded.
pub(crate) fn table(header: &[&str], rows: &[Vec<String>]) -> String {
    let mut widths: Vec<usize> = header.iter().map(|h| h.chars().count()).collect();
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            widths[i] = widths[i].max(cell.chars().count());
        }
    }
    let line = |cells: Vec<&str>| {
        let last = cells.len() - 1;
        let mut out = String::new();
        for (i, c) in cells.iter().enumerate() {
            if i == last {
                out.push_str(c);
            } else {
                out.push_str(&format!("{c:<w$}  ", w = widths[i]));
            }
        }
        out.push('\n');
        out
    };
    let mut out = line(header.to_vec());
    for row in rows {
        out.push_str(&line(row.iter().map(String::as_str).collect()));
    }
    out
}

pub(crate) fn state_label(s: SyncState) -> &'static str {
    match s {
        SyncState::LocalOnly => "local-only",
        SyncState::InSync => "in-sync",
        SyncState::Modified => "modified",
        SyncState::RemoteMissing => "remote-missing",
        SyncState::RemoteOnly => "remote-only",
    }
}

pub(crate) fn push_label(a: PushAction) -> &'static str {
    match a {
        PushAction::Created => "created",
        PushAction::Updated => "updated",
        PushAction::Unchanged => "unchanged",
    }
}

pub(crate) fn pull_label(a: PullAction) -> &'static str {
    match a {
        PullAction::Created => "created",
        PullAction::Updated => "updated",
        PullAction::Unchanged => "unchanged",
    }
}
