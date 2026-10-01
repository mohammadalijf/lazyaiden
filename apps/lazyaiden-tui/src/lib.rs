//! `lazyaiden`: a terminal UI for the Fellow Aiden brewer.
//!
//! The design is a small Elm-style loop:
//!
//! * [`state::App::update`] is a *pure reducer*: an [`state::Event`] (key press
//!   or result message) changes the state and returns [`state::Effect`]s.
//! * [`effects::perform`] runs an effect against the services of
//!   [`lazyaiden_core::Aiden`] and answers with a [`state::Msg`].
//! * [`ui::draw`] renders the state; [`runtime::run`] owns the terminal and the
//!   event loop.
//!
//! Because the reducer and renderer do no I/O, almost everything is tested
//! without a terminal or a network.

#![warn(missing_docs)]

pub mod effects;
pub mod forms;
pub mod model;
pub mod runtime;
pub mod state;
pub mod ui;

pub use runtime::run;
pub use state::{App, Effect, Event, Msg};
