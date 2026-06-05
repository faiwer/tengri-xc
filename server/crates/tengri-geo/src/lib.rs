//! Geographic primitives shared across Tengri crates: distance, bearing, and
//! other lat/lon utilities. Inputs and outputs use the project-wide E5
//! micro-degree wire unit (see [`E5_TO_RAD`]).

mod approx;
mod aspect_ratio;
mod consts;
mod fcc;
mod haversine;
mod point_degrees;
mod point_e5;
mod rdp;

pub use approx::{approximate_distance_m, project_track_points_m};
pub use aspect_ratio::track_aspect_ratio;
pub use consts::{E5_TO_DEGREES, E5_TO_RAD, EARTH_RADIUS_M, METERS_PER_KM};
pub use fcc::fcc_distance_km;
pub use haversine::haversine_m;
pub use point_degrees::PointDegrees;
pub use point_e5::{HasE5Coords, PointE5};
pub use rdp::{
    Point, RdpCapped, rdp, rdp_indexes_capped, rdp_indexes_with_chord_cap,
    simplify_track_for_scoring_with_chord_cap,
};
