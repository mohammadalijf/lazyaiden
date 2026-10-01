mod common;

use common::{MORNING, Tui};
use fellow_client::FellowApi;
use fellow_client::testing::InMemoryFellow;
use lazyaiden_core::profiles::{SyncState, template};
use lazyaiden_tui::model::Panel;
use lazyaiden_tui::state::Mode;

fn state_of(t: &Tui, title: &str) -> Option<SyncState> {
    t.app
        .profiles
        .items
        .iter()
        .find(|r| r.title == title)
        .unwrap()
        .state
}

// ---- startup -----------------------------------------------------------------

#[tokio::test]
async fn startup_loads_profiles_schedules_and_devices() {
    let fake = InMemoryFellow::new();
    let p = fake.create_profile(&template("On Brewer")).await.unwrap();
    fake.create_schedule(&lazyaiden_core::ScheduleDraft {
        days: [false, true, true, true, true, true, false],
        second_from_start_of_the_day: 27_000,
        enabled: true,
        amount_of_water: 500,
        profile_id: p.id,
    })
    .await
    .unwrap();
    let t = Tui::with_fake(fake).await;

    assert!(t.app.loaded && t.app.remote_ok);
    assert_eq!(t.titles(), ["On Brewer"]);
    assert_eq!(state_of(&t, "On Brewer"), Some(SyncState::RemoteOnly));
    assert_eq!(t.app.schedules.items.len(), 1);
    assert_eq!(t.app.devices.items.len(), 1);
    assert!(t.app.devices.items[0].active);
    assert_eq!(t.app.message, None);
}

#[tokio::test]
async fn logged_out_still_shows_local_profiles_and_explains() {
    let mut t = Tui::logged_out().await;
    t.write_profile("local", MORNING);
    t.refresh().await;
    assert!(!t.app.remote_ok);
    assert_eq!(t.titles(), ["Morning V60"]);
    assert_eq!(state_of(&t, "Morning V60"), None);
    assert!(t.message().contains("not logged in"), "{}", t.message());

    t.keys("p").await;
    assert!(t.app.message.as_ref().unwrap().error);
    assert!(t.message().contains("not logged in"));
}

#[tokio::test]
async fn unreadable_files_are_surfaced() {
    let mut t = Tui::new().await;
    t.write_profile("broken", "title: [");
    t.refresh().await;
    assert_eq!(t.app.problems.len(), 1);
    assert_eq!(t.app.problems[0].name, "broken");
}

// ---- profiles -----------------------------------------------------------------

#[tokio::test]
async fn new_profile_opens_the_form_and_saves() {
    let mut t = Tui::new().await;
    t.keys("n").await;
    assert!(matches!(t.app.mode, Mode::Prompt(_)));
    t.text("Flow Brew").await;
    t.keys("<Enter>").await;

    let Mode::Profile(form) = &t.app.mode else {
        panic!("{:?}", t.app.mode)
    };
    assert_eq!(form.name.as_deref(), Some("flow-brew"));
    assert!(
        t.titles().contains(&"Flow Brew".to_owned()),
        "file already exists after creation"
    );

    t.keys("<C-s>").await;
    assert!(matches!(t.app.mode, Mode::Normal));
    assert_eq!(t.message(), "saved flow-brew");
    assert_eq!(state_of(&t, "Flow Brew"), Some(SyncState::LocalOnly));
}

#[tokio::test]
async fn new_profile_with_an_invalid_title_fails_cleanly() {
    let mut t = Tui::new().await;
    t.keys("n").await;
    t.text("Caf\u{e9}").await;
    t.keys("<Enter>").await;
    assert!(matches!(t.app.mode, Mode::Normal));
    assert!(t.app.message.as_ref().unwrap().error);
    assert!(t.message().contains("title"), "{}", t.message());
    assert!(t.titles().is_empty());
}

#[tokio::test]
async fn editing_in_the_form_validates_before_saving() {
    let mut t = Tui::new().await;
    t.add_local("Edit Me").await;
    t.keys("e").await;
    assert!(matches!(t.app.mode, Mode::Profile(_)));

    t.keys("<Down> <C-u> 99").await; // ratio
    t.keys("<C-s>").await;
    assert!(
        matches!(t.app.mode, Mode::Profile(_)),
        "invalid form stays open"
    );
    assert!(t.app.message.as_ref().unwrap().error);

    t.keys("<C-u> 18 <C-s>").await;
    assert!(matches!(t.app.mode, Mode::Normal));
    assert_eq!(
        t.aiden.profiles.get_local("edit-me").unwrap().draft.ratio,
        18.0
    );
}

#[tokio::test]
async fn escape_discards_form_edits() {
    let mut t = Tui::new().await;
    t.add_local("Keep").await;
    t.keys("e <Down> <C-u> 19 <Esc>").await;
    assert!(matches!(t.app.mode, Mode::Normal));
    assert_eq!(
        t.aiden.profiles.get_local("keep").unwrap().draft.ratio,
        16.0
    );
}

#[tokio::test]
async fn push_modify_and_pull_overwrite_cycle() {
    let mut t = Tui::new().await;
    t.add_local("Cycle").await;
    assert_eq!(state_of(&t, "Cycle"), Some(SyncState::LocalOnly));

    t.keys("p").await;
    assert!(
        t.message().starts_with("cycle: created on the brewer"),
        "{}",
        t.message()
    );
    assert_eq!(state_of(&t, "Cycle"), Some(SyncState::InSync));
    assert_eq!(t.remote_titles().await, ["Cycle"]);

    t.keys("p").await;
    assert!(t.message().contains("already up to date"));

    // local change -> modified
    t.keys("e <Down> <C-u> 17 <C-s>").await;
    assert_eq!(state_of(&t, "Cycle"), Some(SyncState::Modified));

    // pulling would lose local changes: asks first
    t.keys("P").await;
    assert!(matches!(t.app.mode, Mode::Confirm(_)));
    t.keys("n").await;
    assert_eq!(
        t.aiden.profiles.get_local("cycle").unwrap().draft.ratio,
        17.0
    );
    t.keys("P y").await;
    assert_eq!(
        t.aiden.profiles.get_local("cycle").unwrap().draft.ratio,
        16.0
    );
    assert_eq!(state_of(&t, "Cycle"), Some(SyncState::InSync));

    // push the change instead
    t.keys("e <Down> <C-u> 17 <C-s> p").await;
    assert!(
        t.message().contains("updated on the brewer"),
        "{}",
        t.message()
    );
    assert_eq!(t.fake.profiles().await.unwrap().len(), 1, "no duplicates");
}

#[tokio::test]
async fn pull_a_remote_only_profile() {
    let fake = InMemoryFellow::new();
    fake.create_profile(&template("Remote Brew")).await.unwrap();
    let mut t = Tui::with_fake(fake).await;
    t.keys("e").await;
    assert!(
        t.app.message.as_ref().unwrap().error,
        "cannot edit without a local copy"
    );
    t.keys("P").await;
    assert_eq!(t.message(), "remote-brew: pulled");
    assert_eq!(state_of(&t, "Remote Brew"), Some(SyncState::InSync));
}

#[tokio::test]
async fn push_all_and_pull_all_ask_for_confirmation() {
    let mut t = Tui::new().await;
    t.add_local("One").await;
    t.add_local("Two").await;
    t.keys("U").await;
    assert!(matches!(t.app.mode, Mode::Confirm(_)));
    t.keys("<Esc>").await;
    assert!(t.remote_titles().await.is_empty());
    t.keys("U y").await;
    assert_eq!(t.message(), "pushed 2, 0 unchanged");
    assert_eq!(t.remote_titles().await.len(), 2);

    t.keys("D y").await;
    assert_eq!(t.message(), "pulled 2 profiles");
}

#[tokio::test]
async fn delete_variants() {
    let mut t = Tui::new().await;
    t.add_local("Local Only").await;
    t.keys("d").await;
    assert!(matches!(t.app.mode, Mode::Confirm(_)));
    t.keys("n").await;
    assert_eq!(t.titles().len(), 1);
    t.keys("d y").await;
    assert!(t.titles().is_empty());

    // linked: choose local only, then both
    t.add_local("Linked A").await;
    t.keys("p d l").await;
    assert!(
        t.titles()
            .iter()
            .all(|x| x != "Linked A" || state_of(&t, "Linked A") == Some(SyncState::RemoteOnly))
    );
    assert_eq!(t.remote_titles().await, ["Linked A"], "remote copy kept");

    t.keys("P").await;
    t.keys("d b").await;
    assert!(t.titles().is_empty());
    assert!(t.remote_titles().await.is_empty());
}

#[tokio::test]
async fn delete_a_remote_only_profile() {
    let fake = InMemoryFellow::new();
    fake.create_profile(&template("Gone Soon")).await.unwrap();
    let mut t = Tui::with_fake(fake).await;
    t.keys("d y").await;
    assert!(t.remote_titles().await.is_empty());
    assert!(t.titles().is_empty());
}

#[tokio::test]
async fn import_and_export_files() {
    let mut t = Tui::new().await;
    let src = t.dir.path().join("incoming.yaml");
    std::fs::write(&src, MORNING).unwrap();

    t.keys("i").await;
    t.text(src.to_str().unwrap()).await;
    t.keys("<Enter>").await;
    assert!(
        t.message().starts_with("imported \"Morning V60\""),
        "{}",
        t.message()
    );
    assert_eq!(t.titles(), ["Morning V60"]);

    t.keys("i").await;
    t.text(src.to_str().unwrap()).await;
    t.keys("<Enter>").await;
    assert!(
        t.app.message.as_ref().unwrap().error,
        "duplicate import is refused"
    );

    let out = t.dir.path().join("exported.yaml");
    t.keys("x <C-u>").await;
    t.text(out.to_str().unwrap()).await;
    t.keys("<Enter>").await;
    assert!(out.is_file());
    assert_eq!(
        std::fs::read_to_string(&out).unwrap(),
        t.aiden.profiles.export_str("morning-v60").unwrap()
    );
}

#[tokio::test]
async fn share_and_import_link() {
    let mut t = Tui::new().await;
    t.add_local("Share Me").await;
    t.keys("s").await;
    assert!(
        t.app.message.as_ref().unwrap().error,
        "must be pushed first"
    );
    t.keys("p s").await;
    let link = t
        .message()
        .strip_prefix("copied to clipboard: ")
        .unwrap()
        .to_owned();
    assert!(link.starts_with("https://fellowproducts.com/p/"));
    assert_eq!(t.clipboard, std::slice::from_ref(&link));

    t.keys("L").await;
    t.text(&link).await;
    t.keys("<Enter>").await;
    assert!(t.message().contains("(not pushed)"), "{}", t.message());
    assert_eq!(t.titles().len(), 2);
}

#[tokio::test]
async fn share_link_is_still_shown_when_the_clipboard_fails() {
    let mut t = Tui::new().await;
    t.add_local("Share Me").await;
    t.clipboard_error = Some("no clipboard".into());
    t.keys("p s").await;
    let m = t.app.message.clone().unwrap();
    assert!(!m.error);
    assert!(
        m.text
            .starts_with("share link: https://fellowproducts.com/p/")
            && m.text.ends_with("(could not copy: no clipboard)"),
        "{}",
        m.text
    );
    assert!(t.clipboard.is_empty());
}

#[tokio::test]
async fn external_editor_round_trip() {
    let mut t = Tui::new().await;
    t.add_local("Ext").await;

    t.editor = Box::new(|p| {
        let text = std::fs::read_to_string(p)?;
        std::fs::write(p, text.replace("bloomDuration: 30", "bloomDuration: 45"))?;
        Ok(true)
    });
    t.keys("E").await;
    assert_eq!(t.message(), "ext is valid");
    assert_eq!(
        t.aiden
            .profiles
            .get_local("ext")
            .unwrap()
            .draft
            .bloom_duration,
        45
    );

    t.editor = Box::new(|p| {
        let text = std::fs::read_to_string(p)?;
        std::fs::write(p, text.replace("bloomDuration: 45", "bloomDuration: 450"))?;
        Ok(true)
    });
    t.keys("E").await;
    assert!(t.app.message.as_ref().unwrap().error);
    assert!(t.message().contains("bloomDuration"), "{}", t.message());
    assert!(
        t.app
            .selected_profile()
            .unwrap()
            .local
            .as_ref()
            .unwrap()
            .draft
            .issues()
            .len()
            == 1
    );

    t.editor = Box::new(|_| Ok(false));
    t.keys("E").await;
    assert!(t.message().contains("editor exited"));
}

// ---- schedules ----------------------------------------------------------------

#[tokio::test]
async fn schedule_lifecycle() {
    let mut t = Tui::new().await;
    t.keys("2 n").await;
    assert!(t.message().contains("push one first"), "{}", t.message());

    t.keys("1").await;
    t.add_local("Brewed").await;
    t.keys("p 2 n").await;
    assert!(matches!(t.app.mode, Mode::Schedule(_)));
    t.keys("<C-s>").await;
    assert_eq!(t.message(), "created schedule s2");
    assert_eq!(t.app.schedules.items.len(), 1);
    assert!(t.app.schedules.items[0].schedule.draft.enabled);

    t.keys("t").await;
    assert_eq!(t.message(), "schedule s2 disabled");
    assert!(!t.app.schedules.items[0].schedule.draft.enabled);
    t.keys("t").await;
    assert!(t.app.schedules.items[0].schedule.draft.enabled);

    t.keys("d").await;
    assert!(matches!(t.app.mode, Mode::Confirm(_)));
    t.keys("y").await;
    assert!(t.app.schedules.items.is_empty());
}

#[tokio::test]
async fn invalid_schedule_form_cannot_be_submitted() {
    let mut t = Tui::new().await;
    t.add_local("S").await;
    t.keys("p 2 n <Tab> <C-u> nonsense <C-s>").await;
    assert!(matches!(t.app.mode, Mode::Schedule(_)));
    assert!(t.app.message.as_ref().unwrap().error);
    assert!(t.fake.schedules().await.unwrap().is_empty());
}

// ---- brewers -------------------------------------------------------------------

#[tokio::test]
async fn several_brewers_open_the_picker_until_one_is_chosen() {
    let mut t = Tui::with_fake(InMemoryFellow::with_brewers(&[
        ("b1", "Kitchen"),
        ("b2", "Office"),
    ]))
    .await;
    let Mode::Brewer(list) = &t.app.mode else {
        panic!("{:?}", t.app.mode)
    };
    assert_eq!(list.items.len(), 2);

    t.keys("<Down> <Enter>").await;
    assert!(matches!(t.app.mode, Mode::Normal));
    assert_eq!(t.message(), "using brewer Office");
    assert!(t.app.remote_ok);
    assert!(
        t.app
            .devices
            .items
            .iter()
            .any(|d| d.active && d.device.id == "b2")
    );
    assert!(
        std::fs::read_to_string(t.dir.path().join("config.toml"))
            .unwrap()
            .contains("b2")
    );

    // profiles are per brewer
    t.add_local("Per Brewer").await;
    t.keys("p").await;
    t.keys("b <Up> <Enter>").await;
    assert_eq!(t.message(), "using brewer Kitchen");
    assert_eq!(state_of(&t, "Per Brewer"), Some(SyncState::LocalOnly));
}

#[tokio::test]
async fn picker_can_be_cancelled_and_reopened() {
    let mut t = Tui::with_fake(InMemoryFellow::with_brewers(&[("b1", "A"), ("b2", "B")])).await;
    t.keys("<Esc>").await;
    assert!(matches!(t.app.mode, Mode::Normal));
    assert!(!t.app.remote_ok, "nothing remote until a brewer is chosen");
    t.keys("3 <Enter>").await;
    assert!(matches!(t.app.mode, Mode::Brewer(_)));
    t.keys("<Enter>").await;
    assert!(t.app.remote_ok);
}

// ---- navigation ------------------------------------------------------------------

#[tokio::test]
async fn navigation_keys() {
    let mut t = Tui::new().await;
    for n in ["A", "B", "C"] {
        t.add_local(n).await;
    }
    assert_eq!(t.app.focus, Panel::Profiles);
    t.keys("j j j j").await;
    assert_eq!(t.app.profiles.selected, 2, "clamped");
    t.keys("g").await;
    assert_eq!(t.app.profiles.selected, 0);
    t.keys("G").await;
    assert_eq!(t.app.profiles.selected, 2);
    t.keys("<Up> k k k").await;
    assert_eq!(t.app.profiles.selected, 0);

    t.keys("<Tab>").await;
    assert_eq!(t.app.focus, Panel::Schedules);
    t.keys("<Tab> <Tab>").await;
    assert_eq!(t.app.focus, Panel::Profiles);
    t.keys("<BackTab>").await;
    assert_eq!(t.app.focus, Panel::Device);
    t.keys("1").await;
    assert_eq!(t.app.focus, Panel::Profiles);
}

#[tokio::test]
async fn selection_survives_refresh_and_clamps_when_rows_vanish() {
    let mut t = Tui::new().await;
    for n in ["A", "B", "C"] {
        t.add_local(n).await;
    }
    t.keys("G").await;
    assert_eq!(t.app.profiles.selected, 2);
    t.keys("r").await;
    assert_eq!(t.app.profiles.selected, 2);
    t.keys("d y").await;
    assert_eq!(
        t.app.profiles.selected, 1,
        "clamped after the last row was deleted"
    );
}

#[tokio::test]
async fn help_and_quit() {
    let mut t = Tui::new().await;
    t.keys("?").await;
    assert!(matches!(t.app.mode, Mode::Help));
    t.keys("x").await;
    assert!(matches!(t.app.mode, Mode::Normal));
    assert!(!t.app.quit);
    t.keys("q").await;
    assert!(t.app.quit);
}

#[tokio::test]
async fn ctrl_c_quits_from_any_mode() {
    let mut t = Tui::new().await;
    t.add_local("X").await;
    t.keys("e <C-c>").await;
    assert!(t.app.quit);
}

#[tokio::test]
async fn typing_q_inside_a_prompt_does_not_quit() {
    let mut t = Tui::new().await;
    t.keys("n").await;
    t.text("Quartz").await;
    assert!(!t.app.quit);
    let Mode::Prompt(p) = &t.app.mode else {
        panic!()
    };
    assert_eq!(p.value, "Quartz");
}

#[tokio::test]
async fn messages_clear_on_the_next_key() {
    let mut t = Tui::new().await;
    t.keys("e").await;
    assert!(!t.message().is_empty());
    t.keys("j").await;
    assert_eq!(t.app.message, None);
}
