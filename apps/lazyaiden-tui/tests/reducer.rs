//! The reducer on its own: no effects are executed, so these are pure state tests.

use lazyaiden_core::profiles::SyncState;
use lazyaiden_tui::model::{DeviceRow, List, Origin, Panel, ProfileRow, Snapshot, UiError};
use lazyaiden_tui::state::{App, Effect, Event, Mode, Msg, press};
use ratatui::crossterm::event::KeyCode;

fn row(
    name: Option<&str>,
    title: &str,
    state: Option<SyncState>,
    remote_id: Option<&str>,
) -> ProfileRow {
    ProfileRow {
        name: name.map(Into::into),
        title: title.into(),
        state,
        remote_id: remote_id.map(Into::into),
        local: None,
        remote: None,
    }
}

fn app_with(rows: Vec<ProfileRow>) -> App {
    let mut app = App::default();
    app.update(Event::Msg(Msg::Refreshed(Box::new(Snapshot {
        profiles: rows,
        remote_ok: true,
        ..Snapshot::default()
    }))));
    app
}

fn keys(app: &mut App, s: &str) -> Vec<Effect> {
    let mut out = Vec::new();
    for c in s.chars() {
        out.extend(app.update(press(KeyCode::Char(c))));
    }
    out
}

#[test]
fn start_requests_one_refresh_and_marks_busy() {
    let mut app = App::default();
    assert_eq!(app.start(), [Effect::Refresh]);
    assert_eq!(app.busy, 1);
    app.update(Event::Msg(Msg::Refreshed(Box::default())));
    assert_eq!((app.busy, app.loaded), (0, true));
}

#[test]
fn every_emitted_effect_counts_as_busy_until_its_message_arrives() {
    let mut app = app_with(vec![row(Some("a"), "A", Some(SyncState::LocalOnly), None)]);
    assert_eq!(keys(&mut app, "p"), [Effect::Push { name: "a".into() }]);
    assert_eq!(app.busy, 1);
    let next = app.update(Event::Msg(Msg::Done {
        message: "ok".into(),
    }));
    assert_eq!(next, [Effect::Refresh]);
    assert_eq!(app.busy, 1, "the follow-up refresh is in flight");
    app.update(Event::Msg(Msg::Refreshed(Box::default())));
    assert_eq!(app.busy, 0);
    app.update(Event::Msg(Msg::Refreshed(Box::default())));
    assert_eq!(app.busy, 0, "never underflows");
}

#[test]
fn activity_names_the_running_effect_and_clears_when_idle() {
    let mut app = app_with(vec![row(Some("a"), "A", Some(SyncState::LocalOnly), None)]);
    assert_eq!(app.activity, None);
    keys(&mut app, "p");
    assert_eq!(app.activity.as_deref(), Some("pushing a"));
    app.update(Event::Tick);
    app.update(Event::Tick);
    assert_eq!((app.spinner, app.busy), (2, 1), "ticks only animate");
    app.update(Event::Msg(Msg::Done {
        message: "ok".into(),
    }));
    assert_eq!(app.activity.as_deref(), Some("refreshing"));
    app.update(Event::Msg(Msg::Refreshed(Box::default())));
    assert_eq!(app.activity, None);
}

#[test]
fn origin_comes_from_the_remote_id() {
    for (id, origin) in [
        ("d150", Some(Origin::Fellow)),
        ("plocal0", Some(Origin::Fellow)),
        ("p12", Some(Origin::Custom)),
        ("p", None),
        ("d", None),
        ("plocal", None),
        ("dx1", None),
        ("x1", None),
    ] {
        assert_eq!(Origin::of(id), origin, "{id}");
    }
    assert_eq!(row(Some("a"), "A", None, None).origin(), None);
}

#[test]
fn fellow_profiles_are_hidden_until_f_is_pressed() {
    let rows = vec![
        row(Some("mine"), "Mine", Some(SyncState::InSync), Some("p3")),
        row(
            None,
            "Light Roast",
            Some(SyncState::RemoteOnly),
            Some("plocal0"),
        ),
        row(None, "Drop", Some(SyncState::RemoteOnly), Some("d150")),
        row(Some("new"), "New", Some(SyncState::LocalOnly), None),
    ];
    let mut app = app_with(rows);
    let titles = |app: &App| {
        app.profiles
            .items
            .iter()
            .map(|r| r.title.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(titles(&app), ["Mine", "New"]);
    assert_eq!(app.hidden_fellow(), 2);

    keys(&mut app, "j");
    keys(&mut app, "f");
    assert_eq!(titles(&app), ["Mine", "Light Roast", "Drop", "New"]);
    assert_eq!(app.hidden_fellow(), 0);
    assert_eq!(
        app.selected_profile().unwrap().title,
        "New",
        "selection follows the profile"
    );

    keys(&mut app, "k");
    keys(&mut app, "f");
    assert_eq!(titles(&app), ["Mine", "New"]);
}

#[test]
fn schedule_form_offers_hidden_fellow_profiles_too() {
    let mut app = app_with(vec![row(
        None,
        "Light Roast",
        Some(SyncState::RemoteOnly),
        Some("plocal0"),
    )]);
    app.all_profiles[0].remote = Some(
        serde_json::from_value(serde_json::json!({"id": "plocal0", "title": "Light Roast"}))
            .unwrap(),
    );
    keys(&mut app, "2n");
    assert!(matches!(app.mode, Mode::Schedule(_)), "{:?}", app.mode);
}

#[test]
fn failed_actions_refresh_and_show_the_error() {
    let mut app = App::default();
    let next = app.update(Event::Msg(Msg::Failed(UiError::plain("boom"))));
    assert_eq!(next, [Effect::Refresh]);
    let m = app.message.unwrap();
    assert!(m.error && m.text == "boom");
}

#[test]
fn push_and_pull_require_the_right_side_to_exist() {
    let mut app = app_with(vec![row(
        None,
        "Remote",
        Some(SyncState::RemoteOnly),
        Some("p1"),
    )]);
    assert!(keys(&mut app, "p").is_empty(), "nothing local to push");
    assert!(app.message.as_ref().unwrap().error);
    assert!(
        keys(&mut app, "e").is_empty()
            && keys(&mut app, "E").is_empty()
            && keys(&mut app, "x").is_empty()
    );
    assert!(keys(&mut app, "s").is_empty());
    // P pulls a remote-only profile (the row has no remote payload here, so it is refused)
    assert!(keys(&mut app, "P").is_empty());

    let mut app = app_with(vec![row(Some("a"), "A", Some(SyncState::LocalOnly), None)]);
    assert!(
        keys(&mut app, "P").is_empty(),
        "never pushed, nothing to pull"
    );
}

#[test]
fn delete_dialogs_depend_on_the_sync_state() {
    let cases = [
        (
            row(Some("a"), "A", Some(SyncState::LocalOnly), None),
            "Delete local file",
        ),
        (
            row(Some("a"), "A", Some(SyncState::InSync), Some("p1")),
            "[b]oth",
        ),
        (
            row(Some("a"), "A", Some(SyncState::Modified), Some("p1")),
            "[b]oth",
        ),
        (
            row(Some("a"), "A", Some(SyncState::RemoteMissing), Some("p1")),
            "Delete local file",
        ),
        (
            row(None, "R", Some(SyncState::RemoteOnly), Some("p9")),
            "from the brewer",
        ),
        (row(Some("a"), "A", None, None), "Delete local file"),
    ];
    for (r, expect) in cases {
        let mut app = app_with(vec![r]);
        keys(&mut app, "d");
        let Mode::Confirm(c) = &app.mode else {
            panic!("{:?}", app.mode)
        };
        assert!(c.text.contains(expect), "{}", c.text);
    }
}

#[test]
fn confirm_dialogs_only_act_on_an_explicit_yes() {
    let mut app = app_with(vec![row(Some("a"), "A", Some(SyncState::LocalOnly), None)]);
    keys(&mut app, "d");
    assert!(keys(&mut app, "x").is_empty());
    assert!(
        matches!(app.mode, Mode::Confirm(_)),
        "unrelated keys keep the dialog open"
    );
    assert!(keys(&mut app, "n").is_empty());
    assert!(matches!(app.mode, Mode::Normal));
    keys(&mut app, "d");
    assert_eq!(
        keys(&mut app, "y"),
        [Effect::DeleteLocal { name: "a".into() }]
    );
}

#[test]
fn delete_both_offers_three_answers() {
    let r = row(Some("a"), "A", Some(SyncState::InSync), Some("p1"));
    let mut app = app_with(vec![r.clone()]);
    keys(&mut app, "d");
    assert_eq!(
        keys(&mut app, "l"),
        [Effect::DeleteLocal { name: "a".into() }]
    );
    let mut app = app_with(vec![r.clone()]);
    keys(&mut app, "d");
    assert_eq!(
        keys(&mut app, "b"),
        [Effect::DeleteBoth { name: "a".into() }]
    );
    let mut app = app_with(vec![r]);
    keys(&mut app, "d");
    assert_eq!(
        keys(&mut app, "y"),
        [Effect::DeleteBoth { name: "a".into() }]
    );
}

#[test]
fn prompts_map_to_effects() {
    let mut app = app_with(vec![row(Some("a"), "A", Some(SyncState::LocalOnly), None)]);
    keys(&mut app, "i");
    keys(&mut app, "f.yaml");
    assert_eq!(
        app.update(press(KeyCode::Enter)),
        [Effect::Import {
            path: "f.yaml".into()
        }]
    );
    keys(&mut app, "L");
    keys(&mut app, "abc123");
    assert_eq!(
        app.update(press(KeyCode::Enter)),
        [Effect::ImportLink {
            link: "abc123".into()
        }]
    );
    keys(&mut app, "x");
    assert_eq!(
        app.update(press(KeyCode::Enter)),
        [Effect::Export {
            name: "a".into(),
            path: "a.yaml".into()
        }]
    );
    keys(&mut app, "n");
    assert!(
        app.update(press(KeyCode::Enter)).is_empty(),
        "an empty title is not submitted"
    );
    assert!(matches!(app.mode, Mode::Prompt(_)));
    app.update(press(KeyCode::Esc));
    assert!(matches!(app.mode, Mode::Normal));
}

#[test]
fn snapshot_with_brewer_choices_opens_the_picker_once() {
    let device = |id: &str| lazyaiden_core::Device {
        id: id.into(),
        display_name: Some(id.into()),
        extra: Default::default(),
    };
    let mut app = App::default();
    let snap = Snapshot {
        errors: vec![UiError {
            message: "choose".into(),
            brewer_choices: Some(vec![device("b1"), device("b2")]),
        }],
        ..Snapshot::default()
    };
    app.update(Event::Msg(Msg::Refreshed(Box::new(snap.clone()))));
    let Mode::Brewer(list) = &app.mode else {
        panic!()
    };
    assert_eq!(list.items.len(), 2);

    // A refresh arriving while a form is open must not steal the keyboard.
    let mut app = App::default();
    keys(&mut app, "n");
    app.update(Event::Msg(Msg::Refreshed(Box::new(snap))));
    assert!(matches!(app.mode, Mode::Prompt(_)));
}

#[test]
fn picker_selection_starts_on_the_active_brewer() {
    let device = |id: &str, active| DeviceRow {
        device: lazyaiden_core::Device {
            id: id.into(),
            display_name: None,
            extra: Default::default(),
        },
        active,
    };
    let mut app = App::default();
    app.update(Event::Msg(Msg::Refreshed(Box::new(Snapshot {
        devices: Some(vec![device("b1", false), device("b2", true)]),
        ..Snapshot::default()
    }))));
    assert_eq!(app.devices.selected, 1);
    app.focus = Panel::Device;
    keys(&mut app, "b");
    let Mode::Brewer(list) = &app.mode else {
        panic!()
    };
    assert_eq!(list.selected, 1);
    assert_eq!(
        app.update(press(KeyCode::Enter)),
        [Effect::SelectBrewer { id: "b2".into() }]
    );
}

#[test]
fn picking_a_brewer_before_devices_load_is_an_error() {
    let mut app = App::default();
    keys(&mut app, "b");
    assert!(app.message.as_ref().unwrap().error);
    assert!(matches!(app.mode, Mode::Normal));
}

#[test]
fn list_stepping_is_clamped_and_replace_keeps_the_index_in_range() {
    let mut l = List::new(vec![1, 2, 3]);
    l.step(-5);
    assert_eq!(l.selected, 0);
    l.step(10);
    assert_eq!(l.selected, 2);
    l.replace(vec![1]);
    assert_eq!(l.selected, 0);
    l.replace(vec![]);
    assert_eq!((l.selected, l.current()), (0, None));
    l.step(1);
    l.first();
    l.last();
    assert_eq!(l.selected, 0);
}

#[test]
fn panel_cycle() {
    assert_eq!(Panel::Profiles.next(), Panel::Schedules);
    assert_eq!(Panel::Device.next(), Panel::Profiles);
    assert_eq!(Panel::Profiles.prev(), Panel::Device);
    for p in Panel::ALL {
        assert_eq!(p.next().prev(), p);
        assert!(!p.title().is_empty());
    }
}

#[test]
fn unknown_keys_and_empty_lists_never_panic() {
    let mut app = App::default();
    for code in [
        KeyCode::F(5),
        KeyCode::PageUp,
        KeyCode::Insert,
        KeyCode::Null,
        KeyCode::Char('Z'),
        KeyCode::Enter,
    ] {
        app.update(press(code));
    }
    for focus in Panel::ALL {
        app.focus = focus;
        for c in "jkgGrtdesnpPxULiD".chars() {
            app.update(press(KeyCode::Char(c)));
            app.mode = Mode::Normal;
        }
    }
    assert!(!app.quit);
}
