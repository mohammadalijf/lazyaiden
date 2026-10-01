use std::sync::Arc;

use fellow_client::testing::InMemoryFellow;
use fellow_client::{FellowApi, FellowError, ProfileDraft};
use lazyaiden_schedules::{Days, NewSchedule, ScheduleError, ScheduleService, TimeOfDay};

fn profile(title: &str) -> ProfileDraft {
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

fn new_schedule(profile: &str) -> NewSchedule {
    NewSchedule {
        days: Days::WEEKDAYS,
        time: "7:30".parse().unwrap(),
        water_ml: 500,
        profile: profile.into(),
        enabled: true,
    }
}

async fn setup() -> (Arc<InMemoryFellow>, ScheduleService, String) {
    let fake = Arc::new(InMemoryFellow::new());
    let p = fake.create_profile(&profile("Morning")).await.unwrap();
    (fake.clone(), ScheduleService::new(fake), p.id)
}

#[tokio::test]
async fn create_by_title_and_by_id() {
    let (fake, svc, pid) = setup().await;
    let a = svc.create(new_schedule("morning")).await.unwrap();
    let b = svc.create(new_schedule(&pid)).await.unwrap();
    assert_eq!(a.draft.profile_id, pid);
    assert_eq!(b.draft.profile_id, pid);
    assert_eq!(a.draft.second_from_start_of_the_day, 27_000);
    assert_eq!(a.draft.days, Days::WEEKDAYS.0);
    assert_eq!(fake.schedules().await.unwrap().len(), 2);
}

#[tokio::test]
async fn create_rejects_bad_input_without_calling_the_api() {
    let (fake, svc, _) = setup().await;

    let mut none = new_schedule("morning");
    none.days = Days::default();
    assert!(
        matches!(svc.create(none).await, Err(ScheduleError::Validation(v)) if v[0].field == "days")
    );

    for ml in [149, 1501, 0] {
        let mut s = new_schedule("morning");
        s.water_ml = ml;
        assert!(
            matches!(svc.create(s).await, Err(ScheduleError::Validation(v)) if v[0].field == "amountOfWater"),
            "{ml}"
        );
    }
    for ml in [150, 1500] {
        let mut s = new_schedule("morning");
        s.water_ml = ml;
        assert!(svc.create(s).await.is_ok(), "{ml}");
    }
    assert_eq!(fake.schedules().await.unwrap().len(), 2);
}

#[tokio::test]
async fn create_with_unknown_or_ambiguous_profile() {
    let (fake, svc, _) = setup().await;
    assert!(matches!(
        svc.create(new_schedule("nope")).await,
        Err(ScheduleError::NotFound(_))
    ));
    fake.create_profile(&profile("Morning")).await.unwrap();
    match svc.create(new_schedule("MORNING")).await {
        Err(ScheduleError::Ambiguous { matches, .. }) => assert_eq!(matches.len(), 2),
        other => panic!("{other:?}"),
    }
    assert!(fake.schedules().await.unwrap().is_empty());
}

#[tokio::test]
async fn list_joins_profile_titles_and_marks_orphans() {
    let (fake, svc, pid) = setup().await;
    svc.create(new_schedule("morning")).await.unwrap();
    let views = svc.list().await.unwrap();
    assert_eq!(views[0].profile_title.as_deref(), Some("Morning"));
    assert_eq!(
        views[0].time(),
        Some(TimeOfDay::from_hms(7, 30, 0).unwrap())
    );
    assert_eq!(views[0].days(), Days::WEEKDAYS);

    fake.delete_profile(&pid).await.unwrap();
    assert_eq!(svc.list().await.unwrap()[0].profile_title, None);
}

#[tokio::test]
async fn toggle_and_set_enabled() {
    let (fake, svc, _) = setup().await;
    let s = svc.create(new_schedule("morning")).await.unwrap();
    assert!(!svc.toggle(&s.id).await.unwrap());
    assert!(!fake.schedules().await.unwrap()[0].draft.enabled);
    assert!(svc.toggle(&s.id).await.unwrap());
    svc.set_enabled(&s.id, false).await.unwrap();
    assert!(!fake.schedules().await.unwrap()[0].draft.enabled);
    svc.set_enabled(&s.id, false).await.unwrap(); // idempotent
}

#[tokio::test]
async fn unknown_ids_are_not_found() {
    let (_, svc, _) = setup().await;
    assert!(matches!(
        svc.toggle("s999").await,
        Err(ScheduleError::NotFound(_))
    ));
    assert!(matches!(
        svc.set_enabled("s999", true).await,
        Err(ScheduleError::NotFound(_))
    ));
    assert!(matches!(
        svc.delete("s999").await,
        Err(ScheduleError::NotFound(_))
    ));
}

#[tokio::test]
async fn delete_removes_the_schedule() {
    let (fake, svc, _) = setup().await;
    let s = svc.create(new_schedule("morning")).await.unwrap();
    svc.delete(&s.id).await.unwrap();
    assert!(fake.schedules().await.unwrap().is_empty());
    assert!(matches!(
        svc.delete(&s.id).await,
        Err(ScheduleError::NotFound(_))
    ));
}

#[tokio::test]
async fn brewer_selection_errors_propagate() {
    let fake = Arc::new(InMemoryFellow::with_brewers(&[
        ("b1", "One"),
        ("b2", "Two"),
    ]));
    let svc = ScheduleService::new(fake.clone());
    assert!(matches!(
        svc.list().await,
        Err(ScheduleError::Fellow(FellowError::BrewerNotSelected(_)))
    ));
    fake.select_brewer("b2");
    assert!(svc.list().await.unwrap().is_empty());
}
