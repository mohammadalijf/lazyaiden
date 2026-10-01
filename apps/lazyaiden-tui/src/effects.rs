//! Runs [`Effect`]s against the services and reports back as [`Msg`]s.

use std::path::Path;

use lazyaiden_core::Aiden;
use lazyaiden_core::profiles::{ImportOptions, PullAction, PushAction, template};

use crate::model::{DeviceRow, ProfileRow, Snapshot, UiError};
use crate::state::{Effect, Msg};

fn failed(e: impl std::error::Error + 'static) -> Msg {
    Msg::Failed(UiError::from_error(&e))
}

fn done(message: impl Into<String>) -> Msg {
    Msg::Done {
        message: message.into(),
    }
}

fn push_unique(errors: &mut Vec<UiError>, e: UiError) {
    if !errors.iter().any(|x| x.message == e.message) {
        errors.push(e);
    }
}

/// Loads everything the panels show. Parts that fail are reported in
/// `errors` while the rest still load (local profiles work while logged out).
pub async fn snapshot(aiden: &Aiden) -> Snapshot {
    let mut snap = Snapshot::default();
    match aiden.profiles.status().await {
        Ok(report) => {
            snap.remote_ok = true;
            snap.problems = report.problems;
            snap.profiles = report
                .entries
                .into_iter()
                .map(|e| ProfileRow {
                    name: e.name,
                    title: e.title,
                    state: Some(e.state),
                    remote_id: e.remote_id,
                    local: e.local,
                    remote: e.remote,
                })
                .collect();
        }
        Err(e) => {
            push_unique(&mut snap.errors, UiError::from_error(&e));
            match aiden.profiles.list_local() {
                Ok(listing) => {
                    snap.problems = listing.problems;
                    snap.profiles = listing
                        .profiles
                        .into_iter()
                        .map(|p| ProfileRow {
                            name: Some(p.name.clone()),
                            title: p.draft.title.clone(),
                            state: None,
                            remote_id: None,
                            local: Some(p),
                            remote: None,
                        })
                        .collect();
                }
                Err(e) => push_unique(&mut snap.errors, UiError::from_error(&e)),
            }
        }
    }
    match aiden.schedules.list().await {
        Ok(v) => snap.schedules = Some(v),
        Err(e) => push_unique(&mut snap.errors, UiError::from_error(&e)),
    }
    match aiden.devices.list().await {
        Ok(v) => {
            snap.devices = Some(
                v.into_iter()
                    .map(|d| DeviceRow {
                        device: d.device,
                        active: d.active,
                    })
                    .collect(),
            );
        }
        Err(e) => push_unique(&mut snap.errors, UiError::from_error(&e)),
    }
    snap
}

fn plural(n: usize, what: &str) -> String {
    format!("{n} {what}{}", if n == 1 { "" } else { "s" })
}

/// Executes one effect. Every effect yields exactly one message.
pub async fn perform(aiden: &Aiden, effect: Effect) -> Msg {
    let profiles = &aiden.profiles;
    match effect {
        Effect::Refresh => Msg::Refreshed(Box::new(snapshot(aiden).await)),
        Effect::NewProfile { title } => {
            let draft = template(&title);
            match profiles.create_local(draft.clone()) {
                Ok(p) => Msg::Created {
                    name: p.name,
                    draft,
                },
                Err(e) => failed(e),
            }
        }
        Effect::SaveProfile { name, draft } => {
            let result = match name {
                Some(n) => profiles.update_local(&n, draft),
                None => profiles.create_local(draft),
            };
            result.map_or_else(failed, |p| done(format!("saved {}", p.name)))
        }
        Effect::Import { path } => profiles
            .import_file(Path::new(&path), &ImportOptions::default())
            .map_or_else(failed, |p| {
                done(format!("imported {:?} as {}", p.draft.title, p.name))
            }),
        Effect::Export { name, path } => profiles
            .export_file(&name, Path::new(&path))
            .map_or_else(failed, |()| done(format!("exported {name} to {path}"))),
        Effect::OpenEditor { .. } | Effect::Copy { .. } => Msg::Failed(UiError::plain(
            "the external editor and the clipboard are handled by the runtime",
        )),
        Effect::Validate { name } => profiles
            .validate_local(&name)
            .map_or_else(failed, |p| done(format!("{} is valid", p.name))),
        Effect::Push { name } => profiles.push(&name).await.map_or_else(failed, |o| {
            let verb = match o.action {
                PushAction::Created => "created on the brewer",
                PushAction::Updated => "updated on the brewer",
                PushAction::Unchanged => "already up to date",
            };
            done(format!("{}: {verb} ({})", o.name, o.remote_id))
        }),
        Effect::PushAll => match profiles.push_all().await {
            Err(e) => failed(e),
            Ok(results) => {
                let (mut changed, mut same, mut errors) = (0, 0, Vec::new());
                for (name, r) in results {
                    match r {
                        Ok(o) if o.action == PushAction::Unchanged => same += 1,
                        Ok(_) => changed += 1,
                        Err(e) => errors.push(format!("{name}: {e}")),
                    }
                }
                if errors.is_empty() {
                    done(format!("pushed {changed}, {same} unchanged"))
                } else {
                    Msg::Failed(UiError::plain(format!(
                        "pushed {changed}, {same} unchanged, {} failed: {}",
                        errors.len(),
                        errors.join("; ")
                    )))
                }
            }
        },
        Effect::Pull { remote_id } => profiles.pull(&remote_id).await.map_or_else(failed, |o| {
            let verb = match o.action {
                PullAction::Created => "pulled",
                PullAction::Updated => "updated from the brewer",
                PullAction::Unchanged => "already up to date",
            };
            done(format!("{}: {verb}", o.name))
        }),
        Effect::PullAll => match profiles.pull_all().await {
            Err(e) => failed(e),
            Ok(results) => {
                let ok = results.iter().filter(|(_, r)| r.is_ok()).count();
                let bad: Vec<String> = results
                    .iter()
                    .filter_map(|(n, r)| r.as_ref().err().map(|e| format!("{n}: {e}")))
                    .collect();
                if bad.is_empty() {
                    done(format!("pulled {}", plural(ok, "profile")))
                } else {
                    Msg::Failed(UiError::plain(format!(
                        "pulled {ok}, {} failed: {}",
                        bad.len(),
                        bad.join("; ")
                    )))
                }
            }
        },
        Effect::DeleteLocal { name } => profiles
            .delete_local(&name)
            .map_or_else(failed, |()| done(format!("deleted local {name}"))),
        Effect::DeleteRemote { query } => profiles
            .delete_remote(&query)
            .await
            .map_or_else(failed, |id| done(format!("deleted {id} on the brewer"))),
        Effect::DeleteBoth { name } => match profiles.delete_remote(&name).await {
            Err(e) => failed(e),
            Ok(_) => profiles.delete_local(&name).map_or_else(failed, |()| {
                done(format!("deleted {name} locally and on the brewer"))
            }),
        },
        Effect::Share { name } => profiles
            .share_link(&name)
            .await
            .map_or_else(failed, |link| Msg::Shared { link }),
        Effect::ImportLink { link } => {
            profiles
                .import_from_link(&link)
                .await
                .map_or_else(failed, |p| {
                    done(format!(
                        "imported {:?} as {} (not pushed)",
                        p.draft.title, p.name
                    ))
                })
        }
        Effect::CreateSchedule(new) => aiden
            .schedules
            .create(new)
            .await
            .map_or_else(failed, |s| done(format!("created schedule {}", s.id))),
        Effect::ToggleSchedule { id } => {
            aiden.schedules.toggle(&id).await.map_or_else(failed, |on| {
                done(format!(
                    "schedule {id} {}",
                    if on { "enabled" } else { "disabled" }
                ))
            })
        }
        Effect::DeleteSchedule { id } => aiden
            .schedules
            .delete(&id)
            .await
            .map_or_else(failed, |()| done(format!("deleted schedule {id}"))),
        Effect::SelectBrewer { id } => aiden.use_brewer(&id).await.map_or_else(failed, |d| {
            done(format!("using brewer {}", d.display_name.unwrap_or(d.id)))
        }),
    }
}
