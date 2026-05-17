//! Geographic primitives shared across the server: distance, bearing,
//! and other lat/lon utilities. Inputs and outputs use the project-wide
//! E5 micro-degree wire unit (see [`consts::E5_TO_RAD`]).

mod approx;
mod consts;
mod haversine;

pub use approx::approximate_distance_m;
pub use consts::{E5_TO_RAD, EARTH_RADIUS_M};
pub use haversine::haversine_m;
