use crate::flight::types::TrackPoint;

use super::consts::METERS_PER_KM;
use super::fcc::fcc_distance_km;
use super::haversine::haversine_m;

/// Any type that stores a geographic position as E5 integer micro-degrees.
pub trait HasE5Coords {
    fn lat_e5(&self) -> i32;
    fn lon_e5(&self) -> i32;
}

impl HasE5Coords for TrackPoint {
    fn lat_e5(&self) -> i32 { self.lat }
    fn lon_e5(&self) -> i32 { self.lon }
}

/// An E5-encoded geographic point: lat/lon as integer micro-degrees (1 degree =
/// 1e5 units).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PointE5 {
    pub lat: i32,
    pub lon: i32,
}

impl HasE5Coords for PointE5 {
    fn lat_e5(&self) -> i32 { self.lat }
    fn lon_e5(&self) -> i32 { self.lon }
}

impl PointE5 {
    pub fn new(lat: i32, lon: i32) -> Self {
        Self { lat, lon }
    }

    pub fn from_track_point(point: &TrackPoint) -> Self {
        Self::new(point.lat, point.lon)
    }

    pub fn distance_fcc_km(self, other: &Self) -> f64 {
        fcc_distance_km(self.lat, self.lon, other.lat, other.lon)
    }

    pub fn distance_fcc_m(self, other: &Self) -> f64 {
        self.distance_fcc_km(other) * 1000.0
    }

    pub fn distance_haversine_km(self, other: &Self) -> f64 {
        haversine_m(self.lat, self.lon, other.lat, other.lon) / METERS_PER_KM
    }
}
