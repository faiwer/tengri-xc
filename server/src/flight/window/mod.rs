//! Takeoff/landing detection over a parsed [`crate::flight::Track`].
//!
//! Port of `igc_lib`'s two-pass algorithm: Viterbi smoothing over a
//! ground-speed-derived flying/standing emission stream, then a
//! `min_landing_time` re-merge. See [`detect`] for the algorithm and
//! [`viterbi`] for the underlying HMM utility.

mod detect;
mod viterbi;

pub use detect::{FlightWindow, find_flight_window};
