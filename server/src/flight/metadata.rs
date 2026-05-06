//! Off-track metadata that travels with a flight in a `.tengri` envelope.
//!
//! Deliberately a sibling of [`CompactTrack`](super::compact::CompactTrack),
//! never nested inside it: the compact format stays strictly about time and
//! geometry. Pilot, glider, recorder, competition info etc. land here once
//! we start populating them from IGC headers / user input.
//!
//! The struct is empty for now. New fields will require a `.tengri` version
//! bump (see [`super::tengri::VERSION`]) because bincode is positional.

use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Metadata {}
