//! Geographic primitives shared across the server: distance, bearing,
//! and other lat/lon utilities. Inputs and outputs use the project-wide
//! E5 micro-degree wire unit (see [`consts::E5_TO_RAD`]).

mod approx;
mod aspect_ratio;
mod consts;
mod fcc;
mod haversine;
mod rdp;

pub use approx::approximate_distance_m;
pub(crate) use approx::project_track_points_m;
pub use aspect_ratio::track_aspect_ratio;
pub use consts::{E5_TO_DEGREES, E5_TO_RAD, EARTH_RADIUS_M, METERS_PER_KM};
pub use fcc::fcc_distance_km;
pub use haversine::haversine_m;
pub(crate) use rdp::{
    Point, RdpCapped, rdp, rdp_indexes, rdp_indexes_capped, rdp_indexes_with_chord_cap,
};
