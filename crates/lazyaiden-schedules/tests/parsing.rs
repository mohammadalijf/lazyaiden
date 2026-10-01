use lazyaiden_schedules::{Days, ScheduleError, TimeOfDay};
use proptest::prelude::*;

fn t(s: &str) -> u32 {
    s.parse::<TimeOfDay>().unwrap().seconds()
}

#[test]
fn parses_24_hour_times() {
    assert_eq!(t("00:00"), 0);
    assert_eq!(t("7:30"), 7 * 3600 + 30 * 60);
    assert_eq!(t("07:30"), 27_000);
    assert_eq!(t("23:59:59"), 86_399);
    assert_eq!(t("  18:05 "), 18 * 3600 + 300);
}

#[test]
fn parses_12_hour_times() {
    assert_eq!(t("12am"), 0);
    assert_eq!(t("12:30 AM"), 30 * 60);
    assert_eq!(t("7am"), 7 * 3600);
    assert_eq!(t("7:30am"), 27_000);
    assert_eq!(t("12pm"), 12 * 3600);
    assert_eq!(t("12:00PM"), 12 * 3600);
    assert_eq!(t("11:59 pm"), 23 * 3600 + 59 * 60);
}

#[test]
fn rejects_bad_times() {
    for bad in [
        "", "7", "24:00", "7:60", "7:30:60", "13pm", "0am", "13:00pm", "a:b", "7:3x", "1:2:3:4",
        "-1:00", "123:00", "7:", ":30", "7:30 xm",
    ] {
        assert!(
            matches!(
                bad.parse::<TimeOfDay>(),
                Err(ScheduleError::Parse { what: "time", .. })
            ),
            "{bad:?} should not parse"
        );
    }
}

#[test]
fn time_boundaries() {
    assert!(TimeOfDay::from_seconds(86_399).is_some());
    assert!(TimeOfDay::from_seconds(86_400).is_none());
    assert!(TimeOfDay::from_hms(24, 0, 0).is_none());
    assert_eq!(
        TimeOfDay::from_seconds(27_015).unwrap().to_string(),
        "07:30:15"
    );
    assert_eq!(
        TimeOfDay::from_seconds(27_000).unwrap().to_string(),
        "07:30"
    );
    assert_eq!(TimeOfDay::from_seconds(27_000).unwrap().hms(), (7, 30, 0));
}

fn d(s: &str) -> Days {
    s.parse().unwrap()
}

#[test]
fn parses_days() {
    assert_eq!(
        d("mon,wed,fri").0,
        [false, true, false, true, false, true, false]
    );
    assert_eq!(d("Mon Wed"), d("monday,wednesday"));
    assert_eq!(
        d("sun"),
        Days([true, false, false, false, false, false, false])
    );
    assert_eq!(d("mon-fri"), Days::WEEKDAYS);
    assert_eq!(d("weekdays"), Days::WEEKDAYS);
    assert_eq!(d("weekends"), Days::WEEKENDS);
    assert_eq!(d("daily"), Days::DAILY);
    assert_eq!(d("EVERYDAY"), Days::DAILY);
    assert_eq!(d("tues,thurs"), d("tue,thu"));
}

#[test]
fn ranges_can_wrap_and_combine() {
    assert_eq!(
        d("fri-mon").0,
        [true, true, false, false, false, true, true]
    );
    assert_eq!(d("sat-sun"), Days::WEEKENDS);
    assert_eq!(
        d("mon-wed,sat").0,
        [false, true, true, true, false, false, true]
    );
    assert_eq!(
        d("weekdays,sun").0,
        [true, true, true, true, true, true, false]
    );
    assert_eq!(
        d("mon-mon").0,
        [false, true, false, false, false, false, false]
    );
}

#[test]
fn rejects_bad_days() {
    for bad in [
        "", "  ", ",", "funday", "mon-", "-fri", "mon-xyz", "mon;wed",
    ] {
        assert!(
            matches!(
                bad.parse::<Days>(),
                Err(ScheduleError::Parse { what: "days", .. })
            ),
            "{bad:?} should not parse"
        );
    }
}

#[test]
fn days_display() {
    assert_eq!(Days::DAILY.to_string(), "daily");
    assert_eq!(Days::WEEKDAYS.to_string(), "weekdays");
    assert_eq!(Days::WEEKENDS.to_string(), "weekends");
    assert_eq!(Days::default().to_string(), "none");
    assert_eq!(d("wed,mon").to_string(), "mon,wed");
    assert!(Days::default().is_empty() && !Days::DAILY.is_empty());
}

proptest! {
    #[test]
    fn time_round_trips(secs in 0u32..86_400) {
        let t = TimeOfDay::from_seconds(secs).unwrap();
        prop_assert_eq!(t.to_string().parse::<TimeOfDay>().unwrap(), t);
    }

    #[test]
    fn time_hms_round_trips(h in 0u32..24, m in 0u32..60, s in 0u32..60) {
        let t = TimeOfDay::from_hms(h, m, s).unwrap();
        prop_assert_eq!(t.hms(), (h, m, s));
        prop_assert_eq!(TimeOfDay::from_seconds(t.seconds()), Some(t));
    }

    #[test]
    fn twelve_hour_clock_agrees_with_24_hour(h in 1u32..=12, m in 0u32..60, pm in any::<bool>()) {
        let text = format!("{h}:{m:02}{}", if pm { "pm" } else { "am" });
        let h24 = h % 12 + if pm { 12 } else { 0 };
        prop_assert_eq!(text.parse::<TimeOfDay>().unwrap(), TimeOfDay::from_hms(h24, m, 0).unwrap());
    }

    #[test]
    fn days_round_trip(mask in proptest::array::uniform7(any::<bool>())) {
        let days = Days(mask);
        prop_assert_eq!(days.to_string().parse::<Days>().unwrap(), days);
    }

    #[test]
    fn arbitrary_input_never_panics(s in ".{0,30}") {
        let _ = s.parse::<TimeOfDay>();
        let _ = s.parse::<Days>();
    }
}
