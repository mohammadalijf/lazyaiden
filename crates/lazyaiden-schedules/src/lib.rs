//! Schedule business logic.
//!
//! A Fellow *schedule* is a recurring on-device brew: weekdays, a time of day,
//! an amount of water and a profile. [`ScheduleService`] manages them through an
//! injected [`FellowApi`](fellow_client::FellowApi); [`Days`] and [`TimeOfDay`]
//! parse and format the human-friendly inputs used by the apps
//! (`mon-fri`, `7:30am`).

#![warn(missing_docs)]

mod error;
mod service;
mod time;

pub use error::{Result, ScheduleError};
pub use service::{NewSchedule, ScheduleService, ScheduleView};
pub use time::{Days, TimeOfDay};
