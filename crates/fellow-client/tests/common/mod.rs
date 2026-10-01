#![allow(dead_code)]

use fellow_client::{ProfileDraft, ScheduleDraft};

pub fn draft(title: &str) -> ProfileDraft {
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
        ss_pulse_temperatures: vec![96.0, 96.0, 96.0],
        batch_pulses_enabled: true,
        batch_pulses_number: 2,
        batch_pulses_interval: 25,
        batch_pulse_temperatures: vec![95.5, 96.0],
    }
}

pub fn schedule(profile_id: &str) -> ScheduleDraft {
    ScheduleDraft {
        days: [false, true, true, true, true, true, false],
        second_from_start_of_the_day: 7 * 3600 + 30 * 60,
        enabled: true,
        amount_of_water: 500,
        profile_id: profile_id.into(),
    }
}

pub mod fake_server;
