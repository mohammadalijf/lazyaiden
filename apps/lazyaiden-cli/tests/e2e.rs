mod common;

use common::{Harness, LIGHT, MORNING};
use fellow_client::FellowApi;
use fellow_client::testing::InMemoryFellow;
use serde_json::Value;

fn json(text: &str) -> Value {
    serde_json::from_str(text).unwrap_or_else(|e| panic!("not JSON ({e}): {text}"))
}

// ---- profile: local ------------------------------------------------------------

#[tokio::test]
async fn add_from_file_and_stdin() {
    let mut h = Harness::new();
    let f = h.write("in/morning.yaml", MORNING);
    let out = h.ok(&["profile", "add", "-i", f.to_str().unwrap()]).await;
    assert_eq!(out, "Imported \"Morning V60\" as morning-v60.\n");

    let o = h
        .run_with_stdin(&["profile", "add", "-i", "-", "--name", "light"], LIGHT)
        .await;
    assert_eq!(
        (o.code, o.out.as_str()),
        (0, "Imported \"Light Roast Batch\" as light.\n"),
        "{}",
        o.err
    );
    assert!(h.profiles_dir().join("light.yaml").is_file());
}

#[tokio::test]
async fn add_reports_invalid_data_with_exit_code_4() {
    let mut h = Harness::new();
    let bad = h.write("bad.yaml", &MORNING.replace("ratio: 16.0", "ratio: 99"));
    let o = h
        .run(&["profile", "add", "-i", bad.to_str().unwrap()])
        .await;
    assert_eq!(o.code, 4);
    assert!(
        o.err.contains("ratio") && o.err.contains("14–20"),
        "{}",
        o.err
    );

    let o = h
        .run_with_stdin(&["profile", "add", "-i", "-"], "title: [")
        .await;
    assert_eq!(o.code, 4);
    assert!(o.err.contains("line"), "{}", o.err);
    assert!(h.aiden.profiles.list_local().unwrap().profiles.is_empty());
}

#[tokio::test]
async fn add_conflict_and_overwrite() {
    let mut h = Harness::new();
    let f = h.write("m.yaml", MORNING);
    let f = f.to_str().unwrap();
    h.ok(&["profile", "add", "-i", f]).await;
    let o = h.run(&["profile", "add", "-i", f]).await;
    assert_eq!(o.code, 6, "{}", o.err);
    h.ok(&["profile", "add", "-i", f, "--overwrite"]).await;
}

#[tokio::test]
async fn add_missing_file_is_a_plain_failure() {
    let mut h = Harness::new();
    assert_eq!(
        h.run(&["profile", "add", "-i", "/no/such/file.yaml"])
            .await
            .code,
        1
    );
}

#[tokio::test]
async fn show_and_export_round_trip() {
    let mut h = Harness::new();
    h.run_with_stdin(&["profile", "add", "-i", "-"], MORNING)
        .await;

    let shown = h.ok(&["profile", "show", "Morning V60"]).await;
    assert!(
        shown.starts_with("profileType: 0\ntitle: Morning V60\n"),
        "{shown}"
    );
    assert_eq!(h.ok(&["profile", "export", "morning-v60"]).await, shown);
    assert_eq!(
        json(&h.ok(&["--json", "profile", "show", "morning-v60"]).await)["bloomDuration"],
        35
    );

    let target = h.dir.path().join("exported.yaml");
    h.ok(&[
        "profile",
        "export",
        "morning-v60",
        "-o",
        target.to_str().unwrap(),
    ])
    .await;
    assert_eq!(std::fs::read_to_string(&target).unwrap(), shown);

    assert_eq!(h.run(&["profile", "show", "ghost"]).await.code, 5);
}

#[tokio::test]
async fn new_creates_a_valid_template() {
    let mut h = Harness::new();
    let out = h.ok(&["profile", "new", "My Brew"]).await;
    assert!(out.starts_with("Created my-brew ("), "{out}");
    assert!(h.aiden.profiles.validate_local("my-brew").is_ok());
    assert_eq!(h.run(&["profile", "new", "Bad \u{e9}"]).await.code, 4);
}

#[tokio::test]
async fn list_local_warns_about_unreadable_files() {
    let mut h = Harness::new();
    h.run_with_stdin(&["profile", "add", "-i", "-"], MORNING)
        .await;
    h.write("profiles/broken.yaml", "title: [");
    let o = h.run(&["profile", "list", "--local"]).await;
    assert_eq!(o.code, 0);
    assert!(o.out.contains("morning-v60") && !o.out.contains("broken"));
    assert!(o.err.contains("warning: broken:"), "{}", o.err);

    let j = json(&h.run(&["--json", "profile", "list", "--local"]).await.out);
    assert_eq!(j["profiles"][0]["name"], "morning-v60");
    assert_eq!(j["problems"][0]["name"], "broken");
}

#[tokio::test]
async fn edit_validates_after_the_editor_returns() {
    let mut h = Harness::new();
    h.run_with_stdin(&["profile", "add", "-i", "-"], MORNING)
        .await;

    h.editor = Box::new(|p| {
        let t = std::fs::read_to_string(p)?;
        std::fs::write(p, t.replace("bloomDuration: 35", "bloomDuration: 40"))?;
        Ok(true)
    });
    assert_eq!(
        h.ok(&["profile", "edit", "morning-v60"]).await,
        "morning-v60 is valid.\n"
    );
    assert_eq!(
        h.aiden
            .profiles
            .get_local("morning-v60")
            .unwrap()
            .draft
            .bloom_duration,
        40
    );

    h.editor = Box::new(|p| {
        let t = std::fs::read_to_string(p)?;
        std::fs::write(p, t.replace("bloomDuration: 40", "bloomDuration: 400"))?;
        Ok(true)
    });
    let o = h.run(&["profile", "edit", "morning-v60"]).await;
    assert_eq!(o.code, 4);
    assert!(
        o.err.contains("bloomDuration") && o.err.contains("not usable yet"),
        "{}",
        o.err
    );

    h.editor = Box::new(|_| Ok(false));
    assert_eq!(h.run(&["profile", "edit", "morning-v60"]).await.code, 1);
}

// ---- profile: sync ---------------------------------------------------------------

#[tokio::test]
async fn push_pull_status_cycle() {
    let mut h = Harness::new();
    h.run_with_stdin(&["profile", "add", "-i", "-"], MORNING)
        .await;

    insta::assert_snapshot!("status_local_only", h.ok(&["profile", "list"]).await);
    insta::assert_snapshot!(
        "push_created",
        h.ok(&["profile", "push", "morning-v60"]).await
    );
    insta::assert_snapshot!("push_unchanged", h.ok(&["profile", "push", "--all"]).await);
    insta::assert_snapshot!("status_in_sync", h.ok(&["profile", "list"]).await);

    // someone else creates a profile on the brewer
    h.fake
        .create_profile(&lazyaiden_core::profiles::template("Elsewhere"))
        .await
        .unwrap();
    insta::assert_snapshot!("status_with_remote_only", h.ok(&["profile", "list"]).await);
    insta::assert_snapshot!("pull_all", h.ok(&["profile", "pull", "--all"]).await);
    assert_eq!(h.fake.profiles().await.unwrap().len(), 2);
}

#[tokio::test]
async fn json_output_for_push_and_status() {
    let mut h = Harness::new();
    h.run_with_stdin(&["profile", "add", "-i", "-"], MORNING)
        .await;
    insta::assert_json_snapshot!(
        "push_json",
        json(&h.ok(&["--json", "profile", "push", "morning-v60"]).await)
    );
    insta::assert_json_snapshot!(
        "status_json",
        json(&h.ok(&["--json", "profile", "list"]).await)
    );
}

#[tokio::test]
async fn push_all_reports_failures_and_exits_1_after_pushing_the_rest() {
    let mut h = Harness::new();
    h.run_with_stdin(&["profile", "add", "-i", "-"], MORNING)
        .await;
    h.write(
        "profiles/zbad.yaml",
        &LIGHT.replace("ratio: 15.5", "ratio: 99"),
    );
    let o = h.run(&["profile", "push", "--all"]).await;
    assert_eq!(o.code, 1);
    assert!(o.out.contains("created    morning-v60 -> p1"), "{}", o.out);
    assert!(
        o.out.contains("error      zbad: invalid profile") && o.out.contains("ratio"),
        "{}",
        o.out
    );
    assert_eq!(o.err, "", "failures are reported once, in the results");
    assert_eq!(h.fake.profiles().await.unwrap().len(), 1);
}

#[tokio::test]
async fn push_and_pull_argument_errors() {
    let mut h = Harness::new();
    assert_eq!(h.run(&["profile", "push"]).await.code, 2);
    assert_eq!(h.run(&["profile", "pull"]).await.code, 2);
    assert_eq!(
        h.run(&["profile", "push", "a", "--all"]).await.code,
        2,
        "clap rejects the conflict"
    );
    assert_eq!(h.run(&["profile", "push", "ghost"]).await.code, 5);
    assert_eq!(h.run(&["profile", "pull", "ghost"]).await.code, 5);
}

#[tokio::test]
async fn diff_share_and_import_link() {
    let mut h = Harness::new();
    h.run_with_stdin(&["profile", "add", "-i", "-"], MORNING)
        .await;
    assert_eq!(
        h.run(&["profile", "diff", "morning-v60"]).await.code,
        5,
        "not pushed yet"
    );
    assert_eq!(h.run(&["profile", "share", "morning-v60"]).await.code, 5);

    h.ok(&["profile", "push", "morning-v60"]).await;
    assert_eq!(
        h.ok(&["profile", "diff", "morning-v60"]).await,
        "No differences.\n"
    );

    let f = h.write("edit.yaml", &MORNING.replace("ratio: 16.0", "ratio: 17.0"));
    h.ok(&["profile", "add", "-i", f.to_str().unwrap(), "--overwrite"])
        .await;
    let diff = h.ok(&["profile", "diff", "morning-v60"]).await;
    assert_eq!(diff, "ratio: local 17.0 / remote 16.0\n");
    assert_eq!(
        json(&h.ok(&["--json", "profile", "diff", "morning-v60"]).await)["changes"][0]["field"],
        "ratio"
    );

    let link = h.ok(&["profile", "share", "morning-v60"]).await;
    assert!(link.starts_with("https://fellowproducts.com/p/"), "{link}");
    let out = h.ok(&["profile", "import-link", link.trim()]).await;
    assert!(out.contains("(not pushed)"), "{out}");
    assert_eq!(h.aiden.profiles.list_local().unwrap().profiles.len(), 2);
}

#[tokio::test]
async fn rm_local_and_remote() {
    let mut h = Harness::new();
    h.run_with_stdin(&["profile", "add", "-i", "-"], MORNING)
        .await;
    h.ok(&["profile", "push", "morning-v60"]).await;

    assert_eq!(
        h.run(&["profile", "rm", "morning-v60", "--remote"])
            .await
            .code,
        2,
        "needs --yes"
    );
    assert_eq!(
        h.ok(&["profile", "rm", "morning-v60", "--remote", "--yes"])
            .await,
        "Deleted morning-v60 (local and remote).\n"
    );
    assert!(h.fake.profiles().await.unwrap().is_empty());
    assert!(h.aiden.profiles.list_local().unwrap().profiles.is_empty());
    assert_eq!(h.run(&["profile", "rm", "morning-v60"]).await.code, 5);

    // --remote on a never-pushed profile must not delete the local file
    h.run_with_stdin(&["profile", "add", "-i", "-"], LIGHT)
        .await;
    assert_eq!(
        h.run(&["profile", "rm", "light-roast-batch", "--remote", "--yes"])
            .await
            .code,
        5
    );
    assert_eq!(h.aiden.profiles.list_local().unwrap().profiles.len(), 1);
    assert_eq!(
        h.ok(&["profile", "rm", "light-roast-batch"]).await,
        "Deleted light-roast-batch.\n"
    );
}

// ---- schedule -------------------------------------------------------------------

#[tokio::test]
async fn schedule_lifecycle() {
    let mut h = Harness::new();
    h.run_with_stdin(&["profile", "add", "-i", "-"], MORNING)
        .await;
    h.ok(&["profile", "push", "morning-v60"]).await;

    let out = h
        .ok(&[
            "schedule",
            "add",
            "--profile",
            "Morning V60",
            "--days",
            "mon-fri",
            "--time",
            "7:30am",
            "--water",
            "500",
        ])
        .await;
    assert_eq!(out, "Created schedule s2 (weekdays at 07:30, 500 ml).\n");
    h.ok(&[
        "schedule",
        "add",
        "--profile",
        "p1",
        "--days",
        "sat,sun",
        "--time",
        "09:15",
        "--water",
        "750",
        "--disabled",
    ])
    .await;
    insta::assert_snapshot!("schedule_list", h.ok(&["schedule", "list"]).await);
    insta::assert_json_snapshot!(
        "schedule_list_json",
        json(&h.ok(&["--json", "schedule", "list"]).await)
    );

    assert_eq!(
        h.ok(&["schedule", "toggle", "s2"]).await,
        "Schedule s2 is now disabled.\n"
    );
    assert_eq!(
        h.ok(&["schedule", "toggle", "s2", "--on"]).await,
        "Schedule s2 is now enabled.\n"
    );
    assert_eq!(
        h.ok(&["schedule", "toggle", "s2", "--off"]).await,
        "Schedule s2 is now disabled.\n"
    );
    assert_eq!(
        h.run(&["schedule", "toggle", "s2", "--on", "--off"])
            .await
            .code,
        2
    );

    assert_eq!(
        h.ok(&["schedule", "rm", "s2"]).await,
        "Deleted schedule s2.\n"
    );
    assert_eq!(h.run(&["schedule", "rm", "s2"]).await.code, 5);
    assert_eq!(h.run(&["schedule", "toggle", "s2"]).await.code, 5);
}

#[tokio::test]
async fn schedule_add_validates_input() {
    let mut h = Harness::new();
    h.run_with_stdin(&["profile", "add", "-i", "-"], MORNING)
        .await;
    h.ok(&["profile", "push", "morning-v60"]).await;
    let add =
        |days: &'static str, time: &'static str, water: &'static str, profile: &'static str| {
            [
                "schedule",
                "add",
                "--profile",
                profile,
                "--days",
                days,
                "--time",
                time,
                "--water",
                water,
            ]
        };
    assert_eq!(h.run(&add("funday", "7:30", "500", "p1")).await.code, 4);
    assert_eq!(h.run(&add("mon", "25:00", "500", "p1")).await.code, 4);
    let o = h.run(&add("mon", "7:30", "100", "p1")).await;
    assert_eq!(o.code, 4);
    assert!(
        o.err.contains("amountOfWater") && o.err.contains("150"),
        "{}",
        o.err
    );
    assert_eq!(h.run(&add("mon", "7:30", "500", "nope")).await.code, 5);
    assert_eq!(h.run(&add("none", "7:30", "500", "p1")).await.code, 4);
    assert_eq!(
        h.run(&add("mon", "7:30", "abc", "p1")).await.code,
        2,
        "clap rejects a non-number"
    );
    assert!(h.fake.schedules().await.unwrap().is_empty());
}

// ---- device -----------------------------------------------------------------------

#[tokio::test]
async fn device_commands_with_several_brewers() {
    let mut h = Harness::with_fake(InMemoryFellow::with_brewers(&[
        ("b1", "Kitchen"),
        ("b2", "Office"),
    ]));
    insta::assert_snapshot!("device_list_unselected", h.ok(&["device", "list"]).await);

    let o = h.run(&["profile", "list"]).await;
    assert_eq!(o.code, 5, "{}", o.err);
    assert!(
        o.err.contains("several brewers") && o.err.contains("Kitchen (b1)"),
        "{}",
        o.err
    );

    assert_eq!(
        h.ok(&["device", "use", "office"]).await,
        "Using brewer Office (b2).\n"
    );
    insta::assert_snapshot!("device_list_selected", h.ok(&["device", "list"]).await);
    assert!(
        h.ok(&["device", "info"])
            .await
            .starts_with("id:   b2\nname: Office\n")
    );
    assert_eq!(json(&h.ok(&["--json", "device", "info"]).await)["id"], "b2");
    assert!(
        std::fs::read_to_string(h.dir.path().join("config.toml"))
            .unwrap()
            .contains("b2")
    );
    h.ok(&["profile", "list"]).await;

    assert_eq!(h.run(&["device", "use", "ghost"]).await.code, 5);
}
