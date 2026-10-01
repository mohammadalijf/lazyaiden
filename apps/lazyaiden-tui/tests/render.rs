mod common;

use common::Tui;
use fellow_client::FellowApi;
use fellow_client::testing::InMemoryFellow;
use lazyaiden_core::profiles::{SyncState, template};
use lazyaiden_tui::model::{ProfileRow, Snapshot};
use lazyaiden_tui::state::App;
use lazyaiden_tui::state::{Event, Msg, press};
use lazyaiden_tui::ui::render_text;
use ratatui::crossterm::event::KeyCode;

const W: u16 = 110;
const H: u16 = 30;

fn shot(t: &Tui) -> String {
    render_text(&t.app, W, H)
}

/// A world with one profile in every sync state, a schedule and a brewer.
async fn populated() -> Tui {
    let fake = InMemoryFellow::new();
    let mut other = template("Remote Only");
    other.ratio = 15.0;
    fake.create_profile(&other).await.unwrap();
    let mut t = Tui::with_fake(fake).await;

    let svc = &t.aiden.profiles;
    for title in ["In Sync", "Modified", "Local Only", "Gone"] {
        svc.create_local(template(title)).unwrap();
    }
    for name in ["in-sync", "modified", "gone"] {
        svc.push(name).await.unwrap();
    }
    let mut changed = template("Modified");
    changed.ratio = 17.0;
    svc.update_local("modified", changed).unwrap();
    let gone = t
        .fake
        .profiles()
        .await
        .unwrap()
        .into_iter()
        .find(|p| p.title == "Gone")
        .unwrap();
    t.fake.delete_profile(&gone.id).await.unwrap();

    t.aiden
        .schedules
        .create(lazyaiden_core::schedules::NewSchedule {
            days: lazyaiden_core::schedules::Days::WEEKDAYS,
            time: "7:30".parse().unwrap(),
            water_ml: 500,
            profile: "In Sync".into(),
            enabled: true,
        })
        .await
        .unwrap();
    t.refresh().await;
    t.keys("g").await;
    t
}

#[tokio::test]
async fn empty_start() {
    let t = Tui::new().await;
    insta::assert_snapshot!("empty", shot(&t));
}

#[tokio::test]
async fn main_view_shows_every_sync_state() {
    let t = populated().await;
    insta::assert_snapshot!("populated", shot(&t));
}

#[tokio::test]
async fn details_pane_shows_differences_for_modified_profiles() {
    let mut t = populated().await;
    t.keys("g").await;
    let titles = t.titles();
    let idx = titles.iter().position(|x| x == "Modified").unwrap();
    for _ in 0..idx {
        t.keys("j").await;
    }
    let text = shot(&t);
    assert!(
        text.contains("Differences") && text.contains("ratio"),
        "{text}"
    );
    insta::assert_snapshot!("details_modified", text);
}

#[tokio::test]
async fn schedule_and_device_panels_have_their_own_details() {
    let mut t = populated().await;
    t.keys("2").await;
    insta::assert_snapshot!("schedule_focus", shot(&t));
    t.keys("3").await;
    insta::assert_snapshot!("device_focus", shot(&t));
}

#[tokio::test]
async fn profile_form_shows_inline_errors() {
    let mut t = Tui::new().await;
    t.add_local("Form Demo").await;
    t.keys("e <Down> <C-u> 99 <Down> <Down> <Down> <C-u> abc")
        .await;
    let text = shot(&t);
    assert!(text.contains("✗") && text.contains("problem(s)"), "{text}");
    insta::assert_snapshot!("profile_form_errors", text);
}

#[tokio::test]
async fn valid_profile_form() {
    let mut t = Tui::new().await;
    t.add_local("Form OK").await;
    t.keys("e").await;
    insta::assert_snapshot!("profile_form_valid", shot(&t));
}

#[tokio::test]
async fn schedule_form() {
    let mut t = Tui::new().await;
    t.add_local("For Schedule").await;
    t.keys("p 2 n <Tab> <C-u> nope").await;
    insta::assert_snapshot!("schedule_form", shot(&t));
}

#[tokio::test]
async fn dialogs() {
    let mut t = populated().await;
    t.keys("n").await;
    t.text("Nameless").await;
    insta::assert_snapshot!("prompt", shot(&t));
    t.keys("<Esc> d").await;
    insta::assert_snapshot!("confirm_delete", shot(&t));
    t.keys("<Esc> ?").await;
    insta::assert_snapshot!("help", shot(&t));
}

#[tokio::test]
async fn brewer_picker_and_status_bar() {
    let t = Tui::with_fake(InMemoryFellow::with_brewers(&[
        ("b1", "Kitchen"),
        ("b2", "Office"),
    ]))
    .await;
    insta::assert_snapshot!("brewer_picker", shot(&t));
}

#[tokio::test]
async fn offline_status() {
    let mut t = Tui::logged_out().await;
    t.write_profile("local", common::MORNING);
    t.refresh().await;
    let text = shot(&t);
    assert!(
        text.contains("offline") && text.contains("not logged in"),
        "{text}"
    );
    insta::assert_snapshot!("offline", text);
}

#[tokio::test]
async fn tiny_terminals_do_not_panic() {
    let t = populated().await;
    for (w, h) in [(1, 1), (10, 3), (40, 8), (60, 12), (200, 60)] {
        let _ = render_text(&t.app, w, h);
    }
    let mut t = populated().await;
    t.keys("e").await;
    for (w, h) in [(1, 1), (20, 5), (50, 10)] {
        let _ = render_text(&t.app, w, h);
    }
    let _ = render_text(&App::default(), 80, 24);
}

#[tokio::test]
async fn busy_indicator_is_shown_while_effects_run() {
    let mut app = App::default();
    let _ = app.start();
    let text = render_text(&app, W, H);
    assert!(
        text.contains("refreshing…")
            && text.contains("loading profiles…")
            && text.contains("loading from the brewer…"),
        "{text}"
    );
    app.update(Event::Msg(Msg::Refreshed(Box::default())));
    let text = render_text(&app, W, H);
    assert!(
        !text.contains("refreshing") && text.contains("No profiles yet"),
        "{text}"
    );
}

#[test]
fn fellow_profiles_are_tagged_and_hidden_by_default() {
    let row = |title: &str, id: &str| ProfileRow {
        name: None,
        title: title.into(),
        state: Some(SyncState::RemoteOnly),
        remote_id: Some(id.into()),
        local: None,
        remote: None,
    };
    let mut app = App::default();
    app.update(Event::Msg(Msg::Refreshed(Box::new(Snapshot {
        profiles: vec![row("Mine", "p7"), row("Light Roast", "plocal0")],
        remote_ok: true,
        ..Snapshot::default()
    }))));
    let text = render_text(&app, W, H);
    assert!(
        text.contains("[C] Mine")
            && !text.contains("Light Roast")
            && text.contains("1 Fellow hidden"),
        "{text}"
    );
    app.update(press(KeyCode::Char('f')));
    let text = render_text(&app, W, H);
    assert!(
        text.contains("[F] Light Roast") && !text.contains("Fellow hidden"),
        "{text}"
    );
}
