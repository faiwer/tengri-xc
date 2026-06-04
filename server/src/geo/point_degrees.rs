use serde::Serialize;

use super::PointE5;

/// WGS-84 geographic point in decimal degrees.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct PointDegrees {
    pub lat: f64,
    pub lon: f64,
}

impl PointDegrees {
    pub fn from_e5(lat: i32, lon: i32) -> Self {
        Self {
            lat: lat as f64 / 1e5,
            lon: lon as f64 / 1e5,
        }
    }
}

impl From<PointE5> for PointDegrees {
    fn from(point: PointE5) -> Self {
        Self::from_e5(point.lat, point.lon)
    }
}
