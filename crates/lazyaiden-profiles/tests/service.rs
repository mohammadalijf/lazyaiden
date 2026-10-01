use std::fs;
use std::sync::Arc;

use fellow_client::testing::InMemoryFellow;
use fellow_client::{FellowApi, FellowError, ProfileDraft};
use lazyaiden_profiles::*;
use tempfile::TempDir;

const MORNING: &str = include_str!("../../../examples/profiles/morning-v60.yaml");
const LIGHT: &str = include_str!("../../../examples/profiles/light-roast-batch.yaml");

struct Env {
    dir: TempDir,
    fake: Arc<InMemoryFellow>,
    svc: ProfileService,
}

fn env_with(fake: InMemoryFellow) -> Env {
    let dir = tempfile::tempdir().unwrap();
    let fake = Arc::new(fake);
    let svc = ProfileService::new(fake.clone(), Arc::new(FsProfileStore::new(dir.path())));
    Env { dir, fake, svc }
}

fn env() -> Env {
    env_with(InMemoryFellow::new())
}

fn draft(title: &str) -> ProfileDraft {
    template(title)
}

fn write_raw(env: &Env, name: &str, text: &str) {
    fs::write(env.dir.path().join(format!("{name}.yaml")), text).unwrap();
}

// ---- local ---------------------------------------------------------------

#[test]
fn create_local_derives_unique_names() {
    let e = env();
    assert_eq!(
        e.svc.create_local(draft("Morning")).unwrap().name,
        "morning"
    );
    assert_eq!(
        e.svc.create_local(draft("Morning")).unwrap().name,
        "morning-2"
    );
    assert_eq!(
        e.svc.create_local(draft("MORNING!")).unwrap().name,
        "morning-3"
    );
    assert_eq!(e.svc.list_local().unwrap().profiles.len(), 3);
}

#[test]
fn create_and_update_reject_invalid_drafts_without_writing() {
    let e = env();
    let mut bad = draft("Bad");
    bad.ratio = 99.0;
    assert!(
        matches!(e.svc.create_local(bad.clone()), Err(ProfileError::Validation(v)) if v[0].field == "ratio")
    );
    assert!(e.svc.list_local().unwrap().profiles.is_empty());

    let ok = e.svc.create_local(draft("Ok")).unwrap();
    assert!(matches!(
        e.svc.update_local(&ok.name, bad),
        Err(ProfileError::Validation(_))
    ));
    assert_eq!(e.svc.get_local("ok").unwrap().draft.ratio, 16.0);
    assert!(matches!(
        e.svc.update_local("ghost", draft("x")),
        Err(ProfileError::Store(StoreError::NotFound(_)))
    ));
}

#[test]
fn get_local_by_name_then_by_title() {
    let e = env();
    e.svc.create_local(draft("Morning V60")).unwrap();
    assert_eq!(e.svc.get_local("morning-v60").unwrap().name, "morning-v60");
    assert_eq!(e.svc.get_local("MORNING v60").unwrap().name, "morning-v60");
    assert!(matches!(
        e.svc.get_local("nope"),
        Err(ProfileError::NotFound(_))
    ));

    let mut other = draft("morning v60");
    other.ratio = 17.0;
    e.svc.create_local(other).unwrap();
    match e.svc.get_local("Morning V60") {
        Err(ProfileError::Ambiguous { matches, .. }) => assert_eq!(matches.len(), 2),
        other => panic!("{other:?}"),
    }
    // The file name still disambiguates.
    assert_eq!(e.svc.get_local("morning-v60-2").unwrap().draft.ratio, 17.0);
}

#[test]
fn update_local_replaces_data() {
    let e = env();
    let p = e.svc.create_local(draft("A")).unwrap();
    let mut d = p.draft.clone();
    d.ratio = 18.0;
    e.svc.update_local(&p.name, d).unwrap();
    assert_eq!(e.svc.get_local("a").unwrap().draft.ratio, 18.0);
}

// ---- import / export -----------------------------------------------------

#[test]
fn imports_the_example_files() {
    let e = env();
    let a = e
        .svc
        .import_str(MORNING, &ImportOptions::default())
        .unwrap();
    let b = e.svc.import_str(LIGHT, &ImportOptions::default()).unwrap();
    assert_eq!(
        (a.name.as_str(), b.name.as_str()),
        ("morning-v60", "light-roast-batch")
    );
    assert_eq!(a.draft.bloom_duration, 35);
    assert_eq!(b.draft.batch_pulse_temperatures.len(), 4);
}

#[test]
fn import_accepts_json_and_ignores_server_fields() {
    let e = env();
    let json = r#"{"id":"p12","createdAt":"2024","isDefaultProfile":false,"profileType":0,"title":"From API",
      "ratio":16,"bloomEnabled":true,"bloomRatio":2,"bloomDuration":30,"bloomTemperature":96,
      "ssPulsesEnabled":true,"ssPulsesNumber":3,"ssPulsesInterval":20,"ssPulseTemperatures":[96,96,96],
      "batchPulsesEnabled":true,"batchPulsesNumber":3,"batchPulsesInterval":20,"batchPulseTemperatures":[96,96,96]}"#;
    let p = e.svc.import_str(json, &ImportOptions::default()).unwrap();
    assert_eq!(p.draft.title, "From API");
    assert!(
        !fs::read_to_string(e.dir.path().join("from-api.yaml"))
            .unwrap()
            .contains("createdAt")
    );
}

#[test]
fn import_validates_and_reports_parse_errors() {
    let e = env();
    let bad_value = MORNING.replace("ratio: 16.0", "ratio: 21.0");
    assert!(matches!(
        e.svc.import_str(&bad_value, &ImportOptions::default()),
        Err(ProfileError::Validation(v)) if v.iter().any(|i| i.field == "ratio")
    ));
    match e
        .svc
        .import_str("title: [oops\n", &ImportOptions::default())
    {
        Err(ProfileError::Parse(m)) => assert!(m.contains("line"), "{m}"),
        other => panic!("{other:?}"),
    }
    assert!(matches!(
        e.svc.import_str("title: only\n", &ImportOptions::default()),
        Err(ProfileError::Parse(m)) if m.contains("missing field")
    ));
    assert!(matches!(
        e.svc.import_str("", &ImportOptions::default()),
        Err(ProfileError::Parse(_))
    ));
    assert!(e.svc.list_local().unwrap().profiles.is_empty());
}

#[test]
fn import_conflicts_and_overwrite() {
    let e = env();
    e.svc
        .import_str(MORNING, &ImportOptions::default())
        .unwrap();
    assert!(matches!(
        e.svc.import_str(MORNING, &ImportOptions::default()),
        Err(ProfileError::Conflict(_))
    ));

    let changed = MORNING.replace("bloomDuration: 35", "bloomDuration: 40");
    let o = ImportOptions {
        overwrite: true,
        ..Default::default()
    };
    let p = e.svc.import_str(&changed, &o).unwrap();
    assert_eq!(p.name, "morning-v60");
    assert_eq!(p.draft.bloom_duration, 40);
    assert_eq!(e.svc.list_local().unwrap().profiles.len(), 1);
}

#[test]
fn import_with_explicit_name() {
    let e = env();
    let named = ImportOptions {
        name: Some("daily".into()),
        overwrite: false,
    };
    assert_eq!(e.svc.import_str(MORNING, &named).unwrap().name, "daily");
    assert!(matches!(
        e.svc.import_str(LIGHT, &named),
        Err(ProfileError::Conflict(_))
    ));
    let o = ImportOptions {
        name: Some("daily".into()),
        overwrite: true,
    };
    assert_eq!(
        e.svc.import_str(LIGHT, &o).unwrap().draft.title,
        "Light Roast Batch"
    );
    let bad = ImportOptions {
        name: Some("../x".into()),
        overwrite: false,
    };
    assert!(matches!(
        e.svc.import_str(MORNING, &bad),
        Err(ProfileError::Store(StoreError::InvalidName(_)))
    ));
}

#[test]
fn import_file_and_export_round_trip() {
    let e = env();
    let src = e.dir.path().join("elsewhere.txt");
    fs::write(&src, MORNING).unwrap();
    let p = e.svc.import_file(&src, &ImportOptions::default()).unwrap();

    let outside = tempfile::tempdir().unwrap();
    let out = outside.path().join("out.yaml");
    e.svc.export_file(&p.name, &out).unwrap();
    let again = env();
    let q = again
        .svc
        .import_file(&out, &ImportOptions::default())
        .unwrap();
    assert_eq!(q.draft, p.draft);

    let text = e.svc.export_str("Morning V60").unwrap();
    assert!(
        !text.contains("remote") && !text.contains("p1"),
        "export must be pristine:\n{text}"
    );

    assert!(matches!(
        e.svc.import_file(
            &e.dir.path().join("missing.yaml"),
            &ImportOptions::default()
        ),
        Err(ProfileError::Store(StoreError::Io { .. }))
    ));
}

// ---- push ----------------------------------------------------------------

#[tokio::test]
async fn push_creates_then_is_unchanged_then_updates_without_duplicates() {
    let e = env();
    e.svc
        .import_str(MORNING, &ImportOptions::default())
        .unwrap();

    let first = e.svc.push("morning-v60").await.unwrap();
    assert_eq!(first.action, PushAction::Created);
    assert_eq!(
        e.svc.push("morning-v60").await.unwrap().action,
        PushAction::Unchanged
    );

    let mut d = e.svc.get_local("morning-v60").unwrap().draft;
    d.ratio = 17.0;
    e.svc.update_local("morning-v60", d).unwrap();
    let third = e.svc.push("Morning V60").await.unwrap();
    assert_eq!(
        (third.action, &third.remote_id),
        (PushAction::Updated, &first.remote_id)
    );

    let remote = e.fake.profiles().await.unwrap();
    assert_eq!(remote.len(), 1);
    assert_eq!(remote[0].draft().unwrap().ratio, 17.0);
}

#[tokio::test]
async fn push_does_not_modify_the_profile_file() {
    let e = env();
    e.svc
        .import_str(MORNING, &ImportOptions::default())
        .unwrap();
    let path = e.dir.path().join("morning-v60.yaml");
    let before = fs::read_to_string(&path).unwrap();
    e.svc.push("morning-v60").await.unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), before);
    assert!(e.dir.path().join(".lazyaiden-links.yaml").is_file());
}

#[tokio::test]
async fn push_recreates_a_remotely_deleted_profile() {
    let e = env();
    e.svc.create_local(draft("A")).unwrap();
    let first = e.svc.push("a").await.unwrap();
    e.fake.delete_profile(&first.remote_id).await.unwrap();
    let again = e.svc.push("a").await.unwrap();
    assert_eq!(again.action, PushAction::Created);
    assert_eq!(e.fake.profiles().await.unwrap().len(), 1);
}

#[tokio::test]
async fn push_refuses_hand_edited_invalid_files() {
    let e = env();
    e.svc
        .import_str(MORNING, &ImportOptions::default())
        .unwrap();
    write_raw(
        &e,
        "morning-v60",
        &MORNING.replace("bloomDuration: 35", "bloomDuration: 500"),
    );
    assert!(matches!(
        e.svc.push("morning-v60").await,
        Err(ProfileError::Validation(_))
    ));
    assert!(e.fake.profiles().await.unwrap().is_empty());
}

#[tokio::test]
async fn push_all_reports_each_result() {
    let e = env();
    e.svc
        .import_str(MORNING, &ImportOptions::default())
        .unwrap();
    e.svc.import_str(LIGHT, &ImportOptions::default()).unwrap();
    write_raw(
        &e,
        "zbad",
        &LIGHT.replace("title: Light Roast Batch", "title: Z\nratio: 99"),
    );
    let mut results = e.svc.push_all().await.unwrap();
    results.sort_by(|a, b| a.0.cmp(&b.0));
    let names: Vec<_> = results.iter().map(|(n, _)| n.as_str()).collect();
    // duplicate `ratio` key makes zbad a parse problem, so it is not pushed at all
    assert_eq!(names, ["light-roast-batch", "morning-v60"]);
    assert!(results.iter().all(|(_, r)| r.is_ok()));
    assert_eq!(e.fake.profiles().await.unwrap().len(), 2);
    assert_eq!(e.svc.list_local().unwrap().problems.len(), 1);
}

#[tokio::test]
async fn push_all_continues_after_a_validation_failure() {
    let e = env();
    e.svc
        .import_str(MORNING, &ImportOptions::default())
        .unwrap();
    write_raw(&e, "invalid", &LIGHT.replace("ratio: 15.5", "ratio: 99"));
    let results = e.svc.push_all().await.unwrap();
    assert_eq!(results.len(), 2);
    let bad = results.iter().find(|(n, _)| n == "invalid").unwrap();
    assert!(matches!(bad.1, Err(ProfileError::Validation(_))));
    let good = results.iter().find(|(n, _)| n == "morning-v60").unwrap();
    assert!(good.1.is_ok());
}

#[tokio::test]
async fn push_unknown_profile_is_not_found() {
    let e = env();
    assert!(matches!(
        e.svc.push("ghost").await,
        Err(ProfileError::NotFound(_))
    ));
}

// ---- status / diff ---------------------------------------------------------

#[tokio::test]
async fn status_classifies_every_state() {
    let e = env();
    // in sync
    e.svc.create_local(draft("Synced")).unwrap();
    e.svc.push("synced").await.unwrap();
    // modified
    e.svc.create_local(draft("Changed")).unwrap();
    e.svc.push("changed").await.unwrap();
    let mut d = draft("Changed");
    d.ratio = 19.0;
    e.svc.update_local("changed", d).unwrap();
    // remote missing
    e.svc.create_local(draft("Gone")).unwrap();
    let gone = e.svc.push("gone").await.unwrap();
    e.fake.delete_profile(&gone.remote_id).await.unwrap();
    // local only
    e.svc.create_local(draft("Fresh")).unwrap();
    // remote only
    e.fake.create_profile(&draft("Elsewhere")).await.unwrap();
    // unreadable
    write_raw(&e, "broken", "title: [");

    let report = e.svc.status().await.unwrap();
    let state_of = |title: &str| {
        report
            .entries
            .iter()
            .find(|s| s.title == title)
            .unwrap()
            .state
    };
    assert_eq!(state_of("Synced"), SyncState::InSync);
    assert_eq!(state_of("Changed"), SyncState::Modified);
    assert_eq!(state_of("Gone"), SyncState::RemoteMissing);
    assert_eq!(state_of("Fresh"), SyncState::LocalOnly);
    assert_eq!(state_of("Elsewhere"), SyncState::RemoteOnly);
    assert_eq!(report.entries.len(), 5);
    assert_eq!(report.problems.len(), 1);
    assert_eq!(report.problems[0].name, "broken");

    let elsewhere = report
        .entries
        .iter()
        .find(|s| s.title == "Elsewhere")
        .unwrap();
    assert!(elsewhere.name.is_none() && elsewhere.local.is_none() && elsewhere.remote.is_some());
}

#[tokio::test]
async fn diff_lists_changed_fields_only() {
    let e = env();
    e.svc.create_local(draft("D")).unwrap();
    assert!(matches!(
        e.svc.diff("d").await,
        Err(ProfileError::NotPushed(_))
    ));
    e.svc.push("d").await.unwrap();
    assert!(e.svc.diff("d").await.unwrap().is_empty());

    let mut changed = draft("D");
    changed.ratio = 18.0;
    changed.bloom_duration = 50;
    e.svc.update_local("d", changed).unwrap();
    let diff = e.svc.diff("d").await.unwrap();
    let fields: Vec<_> = diff.iter().map(|c| c.field.as_str()).collect();
    assert_eq!(fields, ["bloomDuration", "ratio"]);
    assert_eq!(diff[1].local, serde_json::json!(18.0));
    assert_eq!(diff[1].remote, serde_json::json!(16.0));
}

// ---- pull ------------------------------------------------------------------

#[tokio::test]
async fn pull_creates_links_and_is_idempotent() {
    let e = env();
    let remote = e.fake.create_profile(&draft("Remote One")).await.unwrap();
    let out = e.svc.pull(&remote.id).await.unwrap();
    assert_eq!(
        (out.name.as_str(), out.action),
        ("remote-one", PullAction::Created)
    );
    assert_eq!(
        e.svc.get_local("remote-one").unwrap().draft,
        draft("Remote One")
    );
    assert_eq!(
        e.svc.pull("Remote One").await.unwrap().action,
        PullAction::Unchanged
    );

    // Linked: a push right after a pull changes nothing.
    assert_eq!(
        e.svc.push("remote-one").await.unwrap().action,
        PushAction::Unchanged
    );
    assert_eq!(e.svc.list_local().unwrap().profiles.len(), 1);
}

#[tokio::test]
async fn pull_overwrites_the_linked_file_when_remote_changed() {
    let e = env();
    let remote = e.fake.create_profile(&draft("R")).await.unwrap();
    e.svc.pull(&remote.id).await.unwrap();
    let mut d = draft("R");
    d.ratio = 18.5;
    e.fake.update_profile(&remote.id, &d).await.unwrap();
    assert_eq!(
        e.svc.pull(&remote.id).await.unwrap().action,
        PullAction::Updated
    );
    assert_eq!(e.svc.get_local("r").unwrap().draft.ratio, 18.5);
    assert_eq!(e.svc.list_local().unwrap().profiles.len(), 1);
}

#[tokio::test]
async fn pull_errors() {
    let e = env();
    e.fake.create_profile(&draft("Twin")).await.unwrap();
    e.fake.create_profile(&draft("Twin")).await.unwrap();
    assert!(matches!(
        e.svc.pull("nope").await,
        Err(ProfileError::NotFound(_))
    ));
    assert!(matches!(
        e.svc.pull("twin").await,
        Err(ProfileError::Ambiguous { .. })
    ));
}

#[tokio::test]
async fn pull_all_gives_each_profile_its_own_file() {
    let e = env();
    e.fake.create_profile(&draft("Twin")).await.unwrap();
    e.fake.create_profile(&draft("Twin")).await.unwrap();
    e.fake.create_profile(&draft("Solo")).await.unwrap();
    let results = e.svc.pull_all().await.unwrap();
    assert_eq!(results.len(), 3);
    assert!(results.iter().all(|(_, r)| r.is_ok()));
    let mut names: Vec<_> = e
        .svc
        .list_local()
        .unwrap()
        .profiles
        .into_iter()
        .map(|p| p.name)
        .collect();
    names.sort();
    assert_eq!(names, ["solo", "twin", "twin-2"]);
    assert!(
        e.svc
            .status()
            .await
            .unwrap()
            .entries
            .iter()
            .all(|s| s.state == SyncState::InSync)
    );
}

// ---- delete / share / link import -----------------------------------------

#[tokio::test]
async fn delete_remote_keeps_the_local_file_and_unlinks_it() {
    let e = env();
    e.svc.create_local(draft("K")).unwrap();
    let pushed = e.svc.push("k").await.unwrap();
    assert_eq!(e.svc.delete_remote("k").await.unwrap(), pushed.remote_id);
    assert!(e.fake.profiles().await.unwrap().is_empty());
    let status = e.svc.status().await.unwrap();
    assert_eq!(status.entries[0].state, SyncState::LocalOnly);
    assert_eq!(e.svc.push("k").await.unwrap().action, PushAction::Created);
}

#[tokio::test]
async fn delete_remote_by_remote_id_or_title() {
    let e = env();
    let a = e.fake.create_profile(&draft("A")).await.unwrap();
    e.fake.create_profile(&draft("B")).await.unwrap();
    assert_eq!(e.svc.delete_remote(&a.id).await.unwrap(), a.id);
    e.svc.delete_remote("b").await.unwrap();
    assert!(matches!(
        e.svc.delete_remote("b").await,
        Err(ProfileError::NotFound(_))
    ));
}

#[tokio::test]
async fn delete_local_forgets_the_link() {
    let e = env();
    e.svc.create_local(draft("L")).unwrap();
    e.svc.push("l").await.unwrap();
    e.svc.delete_local("l").unwrap();
    assert!(e.svc.list_local().unwrap().profiles.is_empty());
    // The remote copy remains and shows up as remote-only.
    let report = e.svc.status().await.unwrap();
    assert_eq!(report.entries.len(), 1);
    assert_eq!(report.entries[0].state, SyncState::RemoteOnly);
    assert!(matches!(
        e.svc.delete_local("l"),
        Err(ProfileError::NotFound(_))
    ));
}

#[tokio::test]
async fn share_requires_a_push_and_import_from_link_creates_a_local_copy() {
    let e = env();
    e.svc.create_local(draft("Shared Brew")).unwrap();
    assert!(matches!(
        e.svc.share_link("shared-brew").await,
        Err(ProfileError::NotPushed(_))
    ));
    e.svc.push("shared-brew").await.unwrap();
    let link = e.svc.share_link("shared-brew").await.unwrap();

    let other = env_with(InMemoryFellow::new());
    // Same fake backend so the link resolves:
    let svc2 = ProfileService::new(
        e.fake.clone(),
        Arc::new(FsProfileStore::new(other.dir.path())),
    );
    let imported = svc2.import_from_link(&link).await.unwrap();
    assert_eq!(imported.draft, draft("Shared Brew"));
    assert_eq!(svc2.list_local().unwrap().profiles.len(), 1);
    assert_eq!(
        e.fake.profiles().await.unwrap().len(),
        1,
        "import must not push"
    );
}

#[tokio::test]
async fn import_from_a_seeded_link_drops_server_fields() {
    let e = env();
    e.fake.seed_shared("abc123", &draft("Friends Brew"));
    let p = e
        .svc
        .import_from_link("https://fellowproducts.com/p/abc123")
        .await
        .unwrap();
    assert_eq!(p.name, "friends-brew");
    assert!(
        !fs::read_to_string(e.dir.path().join("friends-brew.yaml"))
            .unwrap()
            .contains("sharedFrom")
    );
    assert!(matches!(
        e.svc.import_from_link("zzz").await,
        Err(ProfileError::Fellow(FellowError::NotFound(_)))
    ));
}

// ---- multiple brewers --------------------------------------------------------

#[tokio::test]
async fn links_are_kept_per_brewer() {
    let e = env_with(InMemoryFellow::with_brewers(&[
        ("b1", "One"),
        ("b2", "Two"),
    ]));
    e.svc.create_local(draft("Shared Recipe")).unwrap();

    assert!(matches!(
        e.svc.push("shared-recipe").await,
        Err(ProfileError::Fellow(FellowError::BrewerNotSelected(_)))
    ));

    e.fake.select_brewer("b1");
    assert_eq!(
        e.svc.push("shared-recipe").await.unwrap().action,
        PushAction::Created
    );
    assert_eq!(
        e.svc.push("shared-recipe").await.unwrap().action,
        PushAction::Unchanged
    );

    e.fake.select_brewer("b2");
    assert_eq!(
        e.svc.status().await.unwrap().entries[0].state,
        SyncState::LocalOnly
    );
    assert_eq!(
        e.svc.push("shared-recipe").await.unwrap().action,
        PushAction::Created
    );

    e.fake.select_brewer("b1");
    assert_eq!(
        e.svc.status().await.unwrap().entries[0].state,
        SyncState::InSync
    );
    assert_eq!(e.fake.profiles().await.unwrap().len(), 1);
    e.fake.select_brewer("b2");
    assert_eq!(e.fake.profiles().await.unwrap().len(), 1);
}

#[test]
fn template_is_valid() {
    assert!(template("Anything").validate().is_ok());
}

#[test]
fn path_of_and_validate_local_support_hand_editing() {
    let e = env();
    let p = e.svc.create_local(draft("Hand Edited")).unwrap();
    let path = e.svc.path_of("Hand Edited").unwrap();
    assert_eq!(path, e.dir.path().join("hand-edited.yaml"));
    assert!(e.svc.validate_local(&p.name).is_ok());

    let text = fs::read_to_string(&path).unwrap();
    fs::write(&path, text.replace("ratio: 16.0", "ratio: 99.0")).unwrap();
    assert!(
        matches!(e.svc.validate_local(&p.name), Err(ProfileError::Validation(v)) if v[0].field == "ratio")
    );

    fs::write(&path, "title: [").unwrap();
    assert!(matches!(
        e.svc.validate_local(&p.name),
        Err(ProfileError::Store(StoreError::Parse { .. }))
    ));
    assert!(matches!(
        e.svc.path_of("ghost"),
        Err(ProfileError::NotFound(_))
    ));
}
