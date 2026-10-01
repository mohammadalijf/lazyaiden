mod common;

use fellow_client::{Device, FellowError, Profile, Schedule, brew_id_from_link};
use serde_json::json;

#[test]
fn profile_decodes_server_shape_and_keeps_unknown_fields() {
    let raw = json!({
        "id": "p7", "title": "Morning", "profileType": 0, "ratio": 16, "bloomEnabled": true,
        "bloomRatio": 2, "bloomDuration": 30, "bloomTemperature": 96, "ssPulsesEnabled": true,
        "ssPulsesNumber": 3, "ssPulsesInterval": 20, "ssPulseTemperatures": [96, 96, 96],
        "batchPulsesEnabled": false, "batchPulsesNumber": 2, "batchPulsesInterval": 25,
        "batchPulseTemperatures": [95.5, 96], "createdAt": "2024-05-01", "isDefaultProfile": false,
        "somethingNew": {"nested": 1}
    });
    let profile: Profile = serde_json::from_value(raw.clone()).unwrap();
    assert_eq!(profile.id, "p7");
    assert_eq!(profile.extra["somethingNew"], json!({"nested": 1}));

    let draft = profile.draft().unwrap();
    assert_eq!(draft.title, "Morning");
    assert_eq!(draft.batch_pulse_temperatures, vec![95.5, 96.0]);
    assert!(draft.validate().is_ok());

    // Round trip keeps every server field.
    assert_eq!(serde_json::to_value(&profile).unwrap(), raw);
}

#[test]
fn draft_serializes_with_api_field_names_and_no_server_fields() {
    let value = serde_json::to_value(common::draft("T")).unwrap();
    let obj = value.as_object().unwrap();
    for key in [
        "profileType",
        "bloomEnabled",
        "ssPulseTemperatures",
        "batchPulsesInterval",
    ] {
        assert!(obj.contains_key(key), "missing {key}");
    }
    for key in fellow_client::models::SERVER_PROFILE_FIELDS {
        assert!(!obj.contains_key(*key), "server field {key} leaked");
    }
    assert_eq!(obj.len(), 15);
}

#[test]
fn profile_without_editable_fields_gives_decode_error() {
    let profile: Profile = serde_json::from_value(json!({"id": "p1", "title": "x"})).unwrap();
    assert!(matches!(profile.draft(), Err(FellowError::Decode(_))));
}

#[test]
fn from_draft_then_draft_is_identity() {
    let d = common::draft("Round Trip");
    assert_eq!(Profile::from_draft("p9", &d).draft().unwrap(), d);
}

#[test]
fn schedule_decodes_with_extra_fields() {
    let raw = json!({
        "id": "s1", "days": [true,false,false,false,false,false,true],
        "secondFromStartOfTheDay": 3600, "enabled": true, "amountOfWater": 300,
        "profileId": "p1", "createdAt": "x"
    });
    let s: Schedule = serde_json::from_value(raw.clone()).unwrap();
    assert_eq!(s.draft.amount_of_water, 300);
    assert_eq!(s.extra.len(), 1);
    assert_eq!(serde_json::to_value(&s).unwrap(), raw);
}

#[test]
fn schedule_with_wrong_day_count_fails_to_decode() {
    let raw = json!({"id":"s","days":[true],"secondFromStartOfTheDay":0,"enabled":true,"amountOfWater":200,"profileId":"p1"});
    assert!(serde_json::from_value::<Schedule>(raw).is_err());
}

#[test]
fn device_label() {
    let d: Device =
        serde_json::from_value(json!({"id": "b1", "displayName": "Kitchen", "x": 1})).unwrap();
    assert_eq!(d.label(), "Kitchen (b1)");
    let d: Device = serde_json::from_value(json!({"id": "b2"})).unwrap();
    assert_eq!(d.label(), "b2");
}

mod profile_validation {
    use super::common::draft;

    fn fields(d: &fellow_client::ProfileDraft) -> Vec<String> {
        d.issues().into_iter().map(|i| i.field).collect()
    }

    #[test]
    fn valid_draft_has_no_issues() {
        assert!(draft("Fine Title 1").issues().is_empty());
    }

    #[test]
    fn title_rules() {
        assert_eq!(fields(&draft("")), ["title"]);
        assert!(draft(&"a".repeat(50)).validate().is_ok());
        assert_eq!(fields(&draft(&"a".repeat(51))), ["title"]);
        assert_eq!(fields(&draft("Café")), ["title"]);
    }

    #[test]
    fn numeric_boundaries() {
        let mut d = draft("t");
        d.ratio = 14.0;
        d.bloom_duration = 1;
        d.ss_pulses_number = 10;
        d.ss_pulses_interval = 5;
        d.batch_pulses_number = 1;
        d.batch_pulses_interval = 60;
        d.bloom_temperature = 50.0;
        d.ss_pulse_temperatures = vec![98.5];
        assert!(d.issues().is_empty(), "{:?}", d.issues());

        d.ratio = 20.5;
        d.bloom_ratio = 3.5;
        d.bloom_duration = 0;
        d.ss_pulses_number = 11;
        d.ss_pulses_interval = 4;
        d.batch_pulses_number = 0;
        d.batch_pulses_interval = 61;
        d.bloom_temperature = 49.5;
        d.ss_pulse_temperatures = vec![99.0];
        d.batch_pulse_temperatures = vec![60.25];
        assert_eq!(
            fields(&d),
            [
                "ratio",
                "bloomRatio",
                "bloomDuration",
                "ssPulsesNumber",
                "ssPulsesInterval",
                "batchPulsesNumber",
                "batchPulsesInterval",
                "bloomTemperature",
                "ssPulseTemperatures",
                "batchPulseTemperatures"
            ]
        );
    }

    #[test]
    fn issue_messages_state_the_allowed_range() {
        let mut d = draft("t");
        d.ratio = 30.0;
        let msg = d.validate().unwrap_err().to_string();
        assert!(msg.contains("ratio") && msg.contains("14–20"), "{msg}");
    }
}

mod schedule_validation {
    use super::common::schedule;

    #[test]
    fn valid() {
        assert!(schedule("p1").validate().is_ok());
        assert!(schedule("plocal2").validate().is_ok());
    }

    #[test]
    fn boundaries() {
        let mut s = schedule("p1");
        s.second_from_start_of_the_day = 86_399;
        s.amount_of_water = 150;
        assert!(s.issues().is_empty());
        s.amount_of_water = 1500;
        assert!(s.issues().is_empty());

        s.second_from_start_of_the_day = 86_400;
        s.amount_of_water = 149;
        s.profile_id = "x1".into();
        let fields: Vec<_> = s.issues().into_iter().map(|i| i.field).collect();
        assert_eq!(
            fields,
            ["secondFromStartOfTheDay", "amountOfWater", "profileId"]
        );
        s.amount_of_water = 1501;
        assert!(s.issues().iter().any(|i| i.field == "amountOfWater"));
    }
}

#[test]
fn brew_id_parsing() {
    assert_eq!(
        brew_id_from_link("https://fellowproducts.com/p/abc123").unwrap(),
        "abc123"
    );
    assert_eq!(
        brew_id_from_link("https://fellowproducts.com/p/abc123/").unwrap(),
        "abc123"
    );
    assert_eq!(brew_id_from_link("  abc123 ").unwrap(), "abc123");
    for bad in ["", "/", "https://x.com/p/a-b", "a b"] {
        assert!(brew_id_from_link(bad).is_err(), "{bad:?}");
    }
}
