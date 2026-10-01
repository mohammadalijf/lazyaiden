use std::fs;

use lazyaiden_profiles::{FsProfileStore, Links, LocalProfile, ProfileStore, StoreError, template};

fn local(name: &str, title: &str) -> LocalProfile {
    LocalProfile {
        name: name.into(),
        draft: template(title),
    }
}

#[test]
fn listing_a_missing_directory_is_empty() {
    let dir = tempfile::tempdir().unwrap();
    let store = FsProfileStore::new(dir.path().join("nope"));
    let listing = store.list().unwrap();
    assert!(listing.profiles.is_empty() && listing.problems.is_empty());
    assert_eq!(store.links().unwrap(), Links::default());
}

#[test]
fn save_creates_the_directory_and_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    let store = FsProfileStore::new(dir.path().join("a/b"));
    let p = local("morning", "Morning");
    store.save(&p).unwrap();
    assert!(dir.path().join("a/b/morning.yaml").is_file());
    assert_eq!(store.load("morning").unwrap(), p);
    assert!(store.exists("morning"));
    assert!(!store.exists("other"));
}

#[test]
fn saved_yaml_uses_api_field_names() {
    let dir = tempfile::tempdir().unwrap();
    let store = FsProfileStore::new(dir.path());
    store.save(&local("x", "X")).unwrap();
    let text = fs::read_to_string(dir.path().join("x.yaml")).unwrap();
    assert!(
        text.contains("bloomEnabled:") && text.contains("ssPulseTemperatures:"),
        "{text}"
    );
}

#[test]
fn save_leaves_no_temp_files_and_overwrites() {
    let dir = tempfile::tempdir().unwrap();
    let store = FsProfileStore::new(dir.path());
    store.save(&local("x", "One")).unwrap();
    store.save(&local("x", "Two")).unwrap();
    assert_eq!(store.load("x").unwrap().draft.title, "Two");
    let names: Vec<_> = fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(names.len(), 1, "{names:?}");
}

#[test]
fn list_is_sorted_and_ignores_hidden_and_foreign_files() {
    let dir = tempfile::tempdir().unwrap();
    let store = FsProfileStore::new(dir.path());
    store.save(&local("b", "B")).unwrap();
    store.save(&local("a", "A")).unwrap();
    fs::write(dir.path().join("notes.txt"), "hi").unwrap();
    fs::write(dir.path().join(".hidden.yaml"), "x: 1").unwrap();
    fs::create_dir(dir.path().join("sub.yaml")).unwrap();
    store.save_links(&Links::default()).unwrap();
    let listing = store.list().unwrap();
    let names: Vec<_> = listing.profiles.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["a", "b"]);
    assert!(listing.problems.is_empty());
}

#[test]
fn yml_extension_is_read() {
    let dir = tempfile::tempdir().unwrap();
    let yaml = serde_yaml_ng_text(&template("Legacy"));
    fs::write(dir.path().join("legacy.yml"), yaml).unwrap();
    let store = FsProfileStore::new(dir.path());
    assert_eq!(store.load("legacy").unwrap().draft.title, "Legacy");
    assert_eq!(store.list().unwrap().profiles.len(), 1);
}

fn serde_yaml_ng_text(d: &fellow_client::ProfileDraft) -> String {
    let dir = tempfile::tempdir().unwrap();
    let store = FsProfileStore::new(dir.path());
    store
        .save(&LocalProfile {
            name: "t".into(),
            draft: d.clone(),
        })
        .unwrap();
    fs::read_to_string(dir.path().join("t.yaml")).unwrap()
}

#[test]
fn malformed_file_is_reported_with_position_and_does_not_hide_others() {
    let dir = tempfile::tempdir().unwrap();
    let store = FsProfileStore::new(dir.path());
    store.save(&local("good", "Good")).unwrap();
    fs::write(dir.path().join("bad.yaml"), "title: X\nratio: [unclosed\n").unwrap();
    fs::write(dir.path().join("incomplete.yaml"), "title: Only a title\n").unwrap();

    let listing = store.list().unwrap();
    assert_eq!(listing.profiles.len(), 1);
    assert_eq!(listing.problems.len(), 2);
    let bad = listing.problems.iter().find(|p| p.name == "bad").unwrap();
    assert!(bad.message.contains("line "), "{}", bad.message);
    let incomplete = listing
        .problems
        .iter()
        .find(|p| p.name == "incomplete")
        .unwrap();
    assert!(
        incomplete.message.contains("missing field"),
        "{}",
        incomplete.message
    );

    assert!(matches!(store.load("bad"), Err(StoreError::Parse { .. })));
    assert!(store.exists("bad"));
}

#[test]
fn invalid_names_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let store = FsProfileStore::new(dir.path());
    for name in ["", ".hidden", "../evil", "a/b", "a b", "x..y", "é"] {
        assert!(
            matches!(store.load(name), Err(StoreError::InvalidName(_))),
            "{name:?}"
        );
        assert!(
            matches!(
                store.save(&local(name, "t")),
                Err(StoreError::InvalidName(_))
            ),
            "{name:?}"
        );
        assert!(
            matches!(store.delete(name), Err(StoreError::InvalidName(_))),
            "{name:?}"
        );
        assert!(!store.exists(name));
    }
}

#[test]
fn delete_and_not_found() {
    let dir = tempfile::tempdir().unwrap();
    let store = FsProfileStore::new(dir.path());
    store.save(&local("x", "X")).unwrap();
    store.delete("x").unwrap();
    assert!(!store.exists("x"));
    assert!(matches!(store.delete("x"), Err(StoreError::NotFound(_))));
    assert!(matches!(store.load("x"), Err(StoreError::NotFound(_))));
}

#[test]
fn path_of_points_into_the_directory() {
    let dir = tempfile::tempdir().unwrap();
    let store = FsProfileStore::new(dir.path());
    assert_eq!(store.path_of("x").unwrap(), dir.path().join("x.yaml"));
    assert!(store.path_of("../x").is_none());
}

#[test]
fn links_round_trip_and_garbage_is_a_parse_error() {
    let dir = tempfile::tempdir().unwrap();
    let store = FsProfileStore::new(dir.path());
    let mut links = Links::default();
    links.link("b1", "morning", "p1");
    store.save_links(&links).unwrap();
    assert_eq!(store.links().unwrap(), links);

    fs::write(dir.path().join(".lazyaiden-links.yaml"), "- not\n- a map\n").unwrap();
    assert!(matches!(store.links(), Err(StoreError::Parse { .. })));
    fs::write(dir.path().join(".lazyaiden-links.yaml"), "  \n").unwrap();
    assert_eq!(store.links().unwrap(), Links::default());
}

#[test]
fn links_semantics() {
    let mut l = Links::default();
    l.link("b1", "a", "p1");
    l.link("b1", "b", "p2");
    l.link("b2", "a", "p9");
    assert_eq!(l.remote_id("b1", "a"), Some("p1"));
    assert_eq!(l.remote_id("b2", "a"), Some("p9"));
    assert_eq!(l.name_for("b1", "p2"), Some("b"));
    assert_eq!(l.remote_id("b3", "a"), None);

    // Re-linking a remote id to another name steals it; re-linking a name replaces its id.
    l.link("b1", "c", "p1");
    assert_eq!(l.remote_id("b1", "a"), None);
    assert_eq!(l.name_for("b1", "p1"), Some("c"));
    l.link("b1", "c", "p5");
    assert_eq!(l.remote_id("b1", "c"), Some("p5"));

    l.unlink("b1", "c");
    assert_eq!(l.remote_id("b1", "c"), None);
    l.unlink_remote("b1", "p2");
    assert_eq!(l.name_for("b1", "p2"), None);
    l.forget("a");
    assert_eq!(l, Links::default());
}
