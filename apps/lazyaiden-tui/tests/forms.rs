use lazyaiden_core::profiles::template;
use lazyaiden_tui::forms::{FormOutcome, Kind, ProfileForm, Prompt, PromptKind, ScheduleForm};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn k(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}
fn ctrl(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
}
fn type_into<F>(text: &str, mut f: F)
where
    F: FnMut(KeyEvent) -> FormOutcome,
{
    for c in text.chars() {
        f(k(KeyCode::Char(c)));
    }
}
fn clear(f: &mut ProfileForm) {
    f.handle_key(ctrl('u'));
}

fn value<'a>(f: &'a ProfileForm, key: &str) -> &'a str {
    &f.fields.iter().find(|x| x.key == key).unwrap().value
}

fn goto(f: &mut ProfileForm, key: &str) {
    let idx = f.fields.iter().position(|x| x.key == key).unwrap();
    while f.focus < idx {
        f.handle_key(k(KeyCode::Tab));
    }
    while f.focus > idx {
        f.handle_key(k(KeyCode::BackTab));
    }
}

#[test]
fn profile_form_round_trips_a_draft() {
    let d = template("Round Trip");
    let f = ProfileForm::new(Some("rt".into()), &d);
    assert_eq!(f.fields.len(), 14);
    assert!(f.issues.is_empty());
    assert_eq!(f.draft().unwrap(), d);
    assert_eq!(value(&f, "ratio"), "16");
    assert_eq!(value(&f, "ssPulseTemperatures"), "96, 96, 96");
}

#[test]
fn profile_type_is_preserved_though_not_editable() {
    let mut d = template("T");
    d.profile_type = 3;
    assert_eq!(ProfileForm::new(None, &d).draft().unwrap().profile_type, 3);
}

#[test]
fn live_validation_follows_every_edit() {
    let mut f = ProfileForm::new(None, &template("Live"));
    goto(&mut f, "ratio");
    clear(&mut f);
    assert!(f.issue_for("ratio").is_some(), "empty is not a number");
    type_into("99", |e| f.handle_key(e));
    let msg = &f.issue_for("ratio").unwrap().message;
    assert!(msg.contains("14–20"), "{msg}");
    clear(&mut f);
    type_into("17.5", |e| f.handle_key(e));
    assert!(f.issues.is_empty());
    assert_eq!(f.draft().unwrap().ratio, 17.5);
}

#[test]
fn parse_errors_are_reported_per_field() {
    let mut f = ProfileForm::new(None, &template("Parse"));
    goto(&mut f, "bloomDuration");
    clear(&mut f);
    type_into("abc", |e| f.handle_key(e));
    goto(&mut f, "ssPulseTemperatures");
    clear(&mut f);
    type_into("96, x", |e| f.handle_key(e));
    assert_eq!(
        f.issue_for("bloomDuration").unwrap().message,
        "must be a whole number"
    );
    assert!(
        f.issue_for("ssPulseTemperatures")
            .unwrap()
            .message
            .contains("commas")
    );
    assert_eq!(f.draft().unwrap_err().len(), 2);

    goto(&mut f, "bloomDuration");
    clear(&mut f);
    type_into("-5", |e| f.handle_key(e));
    assert!(
        f.issue_for("bloomDuration").is_some(),
        "negative is not a u32"
    );
}

#[test]
fn title_rules_apply() {
    let mut f = ProfileForm::new(None, &template("Ok"));
    goto(&mut f, "title");
    type_into("é", |e| f.handle_key(e));
    assert!(
        f.issue_for("title")
            .unwrap()
            .message
            .contains("unsupported")
    );
    f.handle_key(k(KeyCode::Backspace));
    assert!(f.issues.is_empty());
    clear(&mut f);
    assert!(f.issue_for("title").is_some(), "empty title");
}

#[test]
fn bool_fields_toggle_and_ignore_typing() {
    let mut f = ProfileForm::new(None, &template("B"));
    goto(&mut f, "bloomEnabled");
    assert_eq!(value(&f, "bloomEnabled"), "true");
    f.handle_key(k(KeyCode::Char(' ')));
    assert_eq!(value(&f, "bloomEnabled"), "false");
    f.handle_key(k(KeyCode::Right));
    assert_eq!(value(&f, "bloomEnabled"), "true");
    f.handle_key(k(KeyCode::Char('x')));
    assert_eq!(value(&f, "bloomEnabled"), "true");
    assert_eq!(f.fields.iter().filter(|x| x.kind == Kind::Bool).count(), 3);
}

#[test]
fn navigation_and_outcomes() {
    let mut f = ProfileForm::new(None, &template("Nav"));
    assert_eq!(f.focus, 0);
    f.handle_key(k(KeyCode::Up));
    assert_eq!(f.focus, 0, "clamped at the top");
    f.handle_key(k(KeyCode::Down));
    f.handle_key(k(KeyCode::Enter));
    assert_eq!(f.focus, 2, "Enter advances");
    for _ in 0..30 {
        f.handle_key(k(KeyCode::Tab));
    }
    assert_eq!(f.focus, 13, "clamped at the bottom");
    assert_eq!(
        f.handle_key(k(KeyCode::Enter)),
        FormOutcome::Submit,
        "Enter on the last field submits"
    );
    assert_eq!(f.handle_key(ctrl('s')), FormOutcome::Submit);
    assert_eq!(f.handle_key(k(KeyCode::Esc)), FormOutcome::Cancel);
    assert_eq!(f.handle_key(k(KeyCode::Char('z'))), FormOutcome::Continue);
}

#[test]
fn ctrl_chars_are_not_typed_into_fields() {
    let mut f = ProfileForm::new(None, &template("Ctl"));
    f.handle_key(ctrl('x'));
    assert_eq!(value(&f, "title"), "Ctl");
}

// ---- schedule form ----------------------------------------------------------------

fn profiles() -> Vec<(String, String)> {
    vec![
        ("p1".into(), "Morning".into()),
        ("p2".into(), "Evening".into()),
    ]
}

#[test]
fn schedule_form_defaults_are_valid() {
    let f = ScheduleForm::new(profiles());
    let s = f.new_schedule().unwrap();
    assert_eq!(
        (s.profile.as_str(), s.water_ml, s.enabled),
        ("p1", 500, true)
    );
    assert_eq!(s.days.to_string(), "weekdays");
    assert_eq!(s.time.to_string(), "07:30");
}

#[test]
fn schedule_form_edits_and_validates() {
    let mut f = ScheduleForm::new(profiles());
    f.handle_key(k(KeyCode::Right));
    assert_eq!(f.new_schedule().unwrap().profile, "p2");
    f.handle_key(k(KeyCode::Right));
    assert_eq!(f.new_schedule().unwrap().profile, "p1", "wraps");
    f.handle_key(k(KeyCode::Left));
    assert_eq!(f.new_schedule().unwrap().profile, "p2");

    f.handle_key(k(KeyCode::Tab));
    f.handle_key(ctrl('u'));
    type_into("funday", |e| f.handle_key(e));
    assert!(f.issue_for("days").is_some());
    f.handle_key(ctrl('u'));
    type_into("sat,sun", |e| f.handle_key(e));
    assert!(f.issues.is_empty());

    f.handle_key(k(KeyCode::Tab));
    f.handle_key(ctrl('u'));
    type_into("25:00", |e| f.handle_key(e));
    assert!(f.issue_for("time").is_some());
    f.handle_key(ctrl('u'));
    type_into("6:15pm", |e| f.handle_key(e));

    f.handle_key(k(KeyCode::Tab));
    f.handle_key(ctrl('u'));
    type_into("149", |e| f.handle_key(e));
    assert!(f.issue_for("water").unwrap().message.contains("150"));
    f.handle_key(ctrl('u'));
    type_into("1500", |e| f.handle_key(e));
    assert!(f.issues.is_empty());

    f.handle_key(k(KeyCode::Tab));
    f.handle_key(k(KeyCode::Char(' ')));
    let s = f.new_schedule().unwrap();
    assert_eq!(
        (s.time.seconds(), s.water_ml, s.enabled),
        (18 * 3600 + 15 * 60, 1500, false)
    );
    assert_eq!(s.days.to_string(), "weekends");
    assert_eq!(
        f.handle_key(k(KeyCode::Enter)),
        FormOutcome::Submit,
        "Enter on the last row submits"
    );
}

#[test]
fn schedule_form_needs_profiles_and_days() {
    let f = ScheduleForm::new(Vec::new());
    assert!(f.issue_for("profile").is_some());
    let mut f = ScheduleForm::new(profiles());
    f.handle_key(k(KeyCode::Tab));
    f.handle_key(ctrl('u'));
    type_into("none", |e| f.handle_key(e));
    assert!(
        f.issue_for("days")
            .unwrap()
            .message
            .contains("at least one")
    );
}

#[test]
fn prompt_requires_an_answer() {
    let mut p = Prompt::new(PromptKind::Import, "File", "");
    assert_eq!(
        p.handle_key(k(KeyCode::Enter)),
        FormOutcome::Continue,
        "empty answer"
    );
    type_into("  ", |e| p.handle_key(e));
    assert_eq!(
        p.handle_key(k(KeyCode::Enter)),
        FormOutcome::Continue,
        "blank answer"
    );
    type_into("a.yaml", |e| p.handle_key(e));
    assert_eq!(p.handle_key(k(KeyCode::Enter)), FormOutcome::Submit);
    p.handle_key(k(KeyCode::Backspace));
    assert_eq!(p.value, "  a.yam");
    p.handle_key(ctrl('u'));
    assert_eq!(p.value, "");
    assert_eq!(p.handle_key(k(KeyCode::Esc)), FormOutcome::Cancel);
}
