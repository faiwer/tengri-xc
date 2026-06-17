//! GeoTIFF CRS detection.
//!
//! Reads `GeoKeyDirectoryTag` (TIFF tag 34735) and decides which of the two
//! supported projections the file is in: [`TifProjection::Wgs84`] (EPSG:4326)
//! or [`TifProjection::WebMercator`] (EPSG:3857). Anything else is rejected at
//! open time so a misconfigured source fails close to the cause instead of
//! producing nonsense bounds far downstream.
//!
//! `GeoKeyDirectoryTag` layout: a `SHORT[]` with a 4-entry header
//! (`[VersionKey, KeyRevision, MinorRevision, NumberOfKeys]`) followed by
//! `4·NumberOfKeys` entries `[KeyID, TIFFTagLocation, Count, Value_Offset]`.
//! When `TIFFTagLocation == 0` the value is stored inline in `Value_Offset`,
//! which is what 4326 / 3857 always do (both fit in a u16). We only handle the
//! inline case.

use crate::tif::error::TiffReadError;

/// CRS the TIFF declares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TifProjection {
    /// EPSG:4326 — geographic (lat/lon, degrees-per-pixel).
    Wgs84,
    /// EPSG:3857 — Web Mercator (projected metres-per-pixel).
    WebMercator,
}

const KEY_GT_MODEL_TYPE: u16 = 1024;
const KEY_GEOGRAPHIC_TYPE: u16 = 2048;
const KEY_PROJECTED_CS_TYPE: u16 = 3072;

const MODEL_TYPE_PROJECTED: u16 = 1;
const MODEL_TYPE_GEOGRAPHIC: u16 = 2;

const EPSG_WGS84: u16 = 4326;
const EPSG_WEB_MERCATOR: u16 = 3857;

/// Decode the `GeoKeyDirectoryTag` byte stream. Returns `None` for "no
/// GeoKeys present"; callers may treat that as legacy-tiepoint-only WGS84.
pub(super) fn parse_geo_key_directory(
    directory: &[u16],
) -> Result<Option<TifProjection>, TiffReadError> {
    if directory.is_empty() {
        return Ok(None);
    }
    if directory.len() < 4 {
        return Err(TiffReadError::UnsupportedLayout(
            "GeoKeyDirectoryTag too short to contain a header",
        ));
    }
    let key_count = directory[3] as usize;
    let body = directory.get(4..).unwrap_or(&[]);
    if body.len() < key_count.saturating_mul(4) {
        return Err(TiffReadError::UnsupportedLayout(
            "GeoKeyDirectoryTag truncated before declared key count",
        ));
    }

    let mut model_type: Option<u16> = None;
    let mut geographic_type: Option<u16> = None;
    let mut projected_cs_type: Option<u16> = None;

    for entry in body[..key_count * 4].chunks_exact(4) {
        let key_id = entry[0];
        let tag_location = entry[1];
        let count = entry[2];
        let value = entry[3];
        if tag_location != 0 || count != 1 {
            // Value lives in another tag (GeoDoubleParams / GeoAsciiParams)
            // or spans multiple shorts. Both forms are unused for the EPSG
            // codes we care about, so skip.
            continue;
        }
        match key_id {
            KEY_GT_MODEL_TYPE => model_type = Some(value),
            KEY_GEOGRAPHIC_TYPE => geographic_type = Some(value),
            KEY_PROJECTED_CS_TYPE => projected_cs_type = Some(value),
            _ => {}
        }
    }

    match (model_type, projected_cs_type, geographic_type) {
        (Some(MODEL_TYPE_PROJECTED), Some(EPSG_WEB_MERCATOR), _)
        | (None, Some(EPSG_WEB_MERCATOR), _) => Ok(Some(TifProjection::WebMercator)),
        (Some(MODEL_TYPE_GEOGRAPHIC), _, Some(EPSG_WGS84))
        | (None, None, Some(EPSG_WGS84)) => Ok(Some(TifProjection::Wgs84)),
        (_, Some(epsg), _) | (_, _, Some(epsg)) => {
            Err(TiffReadError::UnsupportedProjection(epsg))
        }
        _ => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(n_keys: u16) -> Vec<u16> {
        vec![1, 1, 0, n_keys]
    }

    fn entry(key: u16, location: u16, count: u16, value: u16) -> [u16; 4] {
        [key, location, count, value]
    }

    fn directory(entries: &[[u16; 4]]) -> Vec<u16> {
        let mut out = header(entries.len() as u16);
        for entry in entries {
            out.extend_from_slice(entry);
        }
        out
    }

    #[test]
    fn empty_directory_means_no_geo_keys() {
        assert_eq!(parse_geo_key_directory(&[]).unwrap(), None);
    }

    #[test]
    fn header_only_means_no_keys_to_decide_from() {
        assert_eq!(parse_geo_key_directory(&header(0)).unwrap(), None);
    }

    #[test]
    fn detects_wgs84_via_geographic_type_alone() {
        let dir = directory(&[entry(KEY_GEOGRAPHIC_TYPE, 0, 1, EPSG_WGS84)]);
        assert_eq!(parse_geo_key_directory(&dir).unwrap(), Some(TifProjection::Wgs84));
    }

    #[test]
    fn detects_wgs84_via_full_geographic_metadata() {
        let dir = directory(&[
            entry(KEY_GT_MODEL_TYPE, 0, 1, MODEL_TYPE_GEOGRAPHIC),
            entry(KEY_GEOGRAPHIC_TYPE, 0, 1, EPSG_WGS84),
        ]);
        assert_eq!(parse_geo_key_directory(&dir).unwrap(), Some(TifProjection::Wgs84));
    }

    #[test]
    fn detects_web_mercator_via_projected_cs_alone() {
        let dir = directory(&[entry(KEY_PROJECTED_CS_TYPE, 0, 1, EPSG_WEB_MERCATOR)]);
        assert_eq!(
            parse_geo_key_directory(&dir).unwrap(),
            Some(TifProjection::WebMercator),
        );
    }

    #[test]
    fn detects_web_mercator_via_full_projected_metadata() {
        let dir = directory(&[
            entry(KEY_GT_MODEL_TYPE, 0, 1, MODEL_TYPE_PROJECTED),
            entry(KEY_PROJECTED_CS_TYPE, 0, 1, EPSG_WEB_MERCATOR),
        ]);
        assert_eq!(
            parse_geo_key_directory(&dir).unwrap(),
            Some(TifProjection::WebMercator),
        );
    }

    #[test]
    fn rejects_other_epsg_codes_loudly() {
        let dir = directory(&[entry(KEY_PROJECTED_CS_TYPE, 0, 1, 32633)]); // UTM 33N
        assert!(matches!(
            parse_geo_key_directory(&dir),
            Err(TiffReadError::UnsupportedProjection(32633))
        ));
    }

    #[test]
    fn skips_keys_that_reference_other_tags() {
        // KEY_PROJECTED_CS_TYPE with tag_location != 0 means the value
        // would live in another tag — we don't follow that pointer.
        let dir = directory(&[
            entry(KEY_PROJECTED_CS_TYPE, 34736 /* GeoDoubleParams */, 1, 0),
            entry(KEY_GEOGRAPHIC_TYPE, 0, 1, EPSG_WGS84),
        ]);
        assert_eq!(parse_geo_key_directory(&dir).unwrap(), Some(TifProjection::Wgs84));
    }

    #[test]
    fn rejects_truncated_directory() {
        let mut dir = directory(&[entry(KEY_PROJECTED_CS_TYPE, 0, 1, EPSG_WEB_MERCATOR)]);
        dir.pop();
        dir.pop();
        assert!(matches!(
            parse_geo_key_directory(&dir),
            Err(TiffReadError::UnsupportedLayout(_))
        ));
    }
}
