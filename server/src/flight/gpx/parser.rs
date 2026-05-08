//! GPX 1.0 / 1.1 parser. We accept the standard `<trk><trkseg><trkpt>`
//! shape and ignore everything else (waypoints, routes, extension
//! payloads, custom namespaces — all stripped).
//!
//! # Multi-segment / multi-track files
//!
//! GPX permits multiple `<trk>` per file and multiple `<trkseg>` per
//! track. A `<trkseg>` boundary is a "discontinuity hint" (the
//! recorder lost signal, or the user paused), but our pipeline treats
//! the file as a single chronological sequence either way: we
//! concatenate trackpoints in document order. If a recorder's
//! segment-break carries semantic meaning we'll revisit that.
//!
//! # Required vs optional fields
//!
//! - `lat`, `lon` (XML attributes) — **required**. Missing rejects the
//!   point with `MissingAttribute`.
//! - `<time>` — **required**. Our pipeline keys on epoch seconds; a
//!   GPX without timestamps is unusable for analytics. Missing rejects
//!   the point.
//! - `<ele>` — optional. Missing → `geo_alt = 0`. The GPX spec doesn't
//!   distinguish geodetic from barometric altitude (both are written
//!   to `<ele>`); we always treat it as geodetic and leave
//!   `pressure_alt = None`.
//!
//! # Encoding
//!
//! GPX is XML and therefore UTF-8 (per the XML spec). We don't
//! tolerate codepage drift here — if you see a real GPX in something
//! other than UTF-8, the recorder is broken and the right fix is to
//! transcode at the source.

use roxmltree::{Document, Node};

use crate::flight::geo_text::{deg_to_e5, m_to_dm, parse_iso8601_u32};
use crate::flight::types::{Track, TrackPoint};

use super::error::GpxError;

pub fn parse_bytes(bytes: &[u8]) -> Result<Track, GpxError> {
    if bytes.is_empty() {
        return Err(GpxError::Empty);
    }
    let text = std::str::from_utf8(bytes)?;
    parse_str(text)
}

pub fn parse_str(input: &str) -> Result<Track, GpxError> {
    if input.trim().is_empty() {
        return Err(GpxError::Empty);
    }
    let doc = Document::parse(input)?;

    let mut points: Vec<TrackPoint> = Vec::new();
    for trkpt in doc
        .descendants()
        .filter(|n| n.is_element() && n.tag_name().name() == "trkpt")
    {
        let index = points.len();
        points.push(parse_trkpt(trkpt, index)?);
    }

    if points.is_empty() {
        return Err(GpxError::NoFixes);
    }

    let start_time = points[0].time;
    Ok(Track { start_time, points })
}

fn parse_trkpt(node: Node, index: usize) -> Result<TrackPoint, GpxError> {
    let lat_str = node
        .attribute("lat")
        .ok_or_else(|| GpxError::MissingAttribute {
            index,
            reason: "lat".into(),
        })?;
    let lon_str = node
        .attribute("lon")
        .ok_or_else(|| GpxError::MissingAttribute {
            index,
            reason: "lon".into(),
        })?;

    let lat_deg = lat_str.parse::<f64>().map_err(|e| GpxError::BadCoord {
        index,
        reason: format!("bad lat {lat_str:?}: {e}"),
    })?;
    let lon_deg = lon_str.parse::<f64>().map_err(|e| GpxError::BadCoord {
        index,
        reason: format!("bad lon {lon_str:?}: {e}"),
    })?;
    if !lat_deg.is_finite() || !lon_deg.is_finite() {
        return Err(GpxError::BadCoord {
            index,
            reason: format!("non-finite lat/lon: {lat_deg}/{lon_deg}"),
        });
    }

    let time_text = node
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "time")
        .and_then(|n| n.text())
        .ok_or_else(|| GpxError::MissingAttribute {
            index,
            reason: "<time>".into(),
        })?;
    let time =
        parse_iso8601_u32(time_text).map_err(|reason| GpxError::BadTime { index, reason })?;

    let ele_dm: i32 = node
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "ele")
        .and_then(|n| n.text())
        .map(|s| {
            s.trim().parse::<f64>().map_err(|e| GpxError::BadCoord {
                index,
                reason: format!("bad ele {s:?}: {e}"),
            })
        })
        .transpose()?
        .filter(|m| m.is_finite())
        .map(m_to_dm)
        .unwrap_or(0);

    Ok(TrackPoint {
        time,
        lat: deg_to_e5(lat_deg),
        lon: deg_to_e5(lon_deg),
        geo_alt: ele_dm,
        pressure_alt: None,
        tas: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::DateTime;

    #[test]
    fn rejects_empty() {
        assert!(matches!(parse_str(""), Err(GpxError::Empty)));
        assert!(matches!(parse_str("   \n  "), Err(GpxError::Empty)));
    }

    #[test]
    fn rejects_xml_without_trkpt() {
        let input = r#"<?xml version="1.0"?><gpx><wpt lat="0" lon="0"/></gpx>"#;
        assert!(matches!(parse_str(input), Err(GpxError::NoFixes)));
    }

    #[test]
    fn parses_minimal_gpx() {
        let input = r#"<?xml version="1.0" encoding="UTF-8"?>
<gpx xmlns="http://www.topografix.com/GPX/1/1" version="1.1" creator="test">
  <trk>
    <trkseg>
      <trkpt lat="46.765998" lon="13.166333">
        <ele>1741</ele>
        <time>2026-05-02T10:54:33Z</time>
      </trkpt>
      <trkpt lat="46.766000" lon="13.166400">
        <ele>1742</ele>
        <time>2026-05-02T10:54:34Z</time>
      </trkpt>
    </trkseg>
  </trk>
</gpx>"#;
        let t = parse_str(input).expect("parse");
        assert_eq!(t.points.len(), 2);
        let expected_start = DateTime::parse_from_rfc3339("2026-05-02T10:54:33Z")
            .unwrap()
            .timestamp() as u32;
        assert_eq!(t.start_time, expected_start);
        assert_eq!(t.points[1].time, expected_start + 1);
        assert_eq!(t.points[0].lat, 4_676_600);
        assert_eq!(t.points[0].lon, 1_316_633);
        assert_eq!(t.points[0].geo_alt, 17_410);
        assert!(t.points.iter().all(|p| p.pressure_alt.is_none()));
        assert!(t.points.iter().all(|p| p.tas.is_none()));
    }

    #[test]
    fn concatenates_multiple_segments_and_tracks() {
        let input = r#"<?xml version="1.0"?>
<gpx>
  <trk>
    <trkseg>
      <trkpt lat="46.0" lon="13.0"><time>2026-05-02T10:00:00Z</time></trkpt>
    </trkseg>
    <trkseg>
      <trkpt lat="46.1" lon="13.1"><time>2026-05-02T10:00:01Z</time></trkpt>
    </trkseg>
  </trk>
  <trk>
    <trkseg>
      <trkpt lat="46.2" lon="13.2"><time>2026-05-02T10:00:02Z</time></trkpt>
    </trkseg>
  </trk>
</gpx>"#;
        let t = parse_str(input).expect("parse");
        assert_eq!(t.points.len(), 3);
        assert_eq!(t.points[0].time + 1, t.points[1].time);
        assert_eq!(t.points[1].time + 1, t.points[2].time);
    }

    #[test]
    fn missing_ele_defaults_to_zero() {
        let input = r#"<?xml version="1.0"?>
<gpx><trk><trkseg>
  <trkpt lat="46.0" lon="13.0"><time>2026-05-02T10:00:00Z</time></trkpt>
</trkseg></trk></gpx>"#;
        let t = parse_str(input).expect("parse");
        assert_eq!(t.points[0].geo_alt, 0);
    }

    #[test]
    fn rejects_trkpt_without_time() {
        let input = r#"<?xml version="1.0"?>
<gpx><trk><trkseg>
  <trkpt lat="46.0" lon="13.0"><ele>1500</ele></trkpt>
</trkseg></trk></gpx>"#;
        assert!(matches!(
            parse_str(input),
            Err(GpxError::MissingAttribute { index: 0, .. })
        ));
    }

    #[test]
    fn rejects_trkpt_without_lat() {
        let input = r#"<?xml version="1.0"?>
<gpx><trk><trkseg>
  <trkpt lon="13.0"><time>2026-05-02T10:00:00Z</time></trkpt>
</trkseg></trk></gpx>"#;
        assert!(matches!(
            parse_str(input),
            Err(GpxError::MissingAttribute { index: 0, .. })
        ));
    }

    #[test]
    fn rejects_bad_lat_value() {
        let input = r#"<?xml version="1.0"?>
<gpx><trk><trkseg>
  <trkpt lat="hello" lon="13.0"><time>2026-05-02T10:00:00Z</time></trkpt>
</trkseg></trk></gpx>"#;
        assert!(matches!(parse_str(input), Err(GpxError::BadCoord { .. })));
    }

    #[test]
    fn rejects_bad_time_value() {
        let input = r#"<?xml version="1.0"?>
<gpx><trk><trkseg>
  <trkpt lat="46.0" lon="13.0"><time>not-a-date</time></trkpt>
</trkseg></trk></gpx>"#;
        assert!(matches!(parse_str(input), Err(GpxError::BadTime { .. })));
    }

    /// Real-world GPX often has decimal `<ele>` (sub-metre precision).
    /// We round to decimetres and store as an integer, so `1741.6 m`
    /// must produce `17_416 dm`.
    #[test]
    fn ele_with_decimals_rounds_correctly() {
        let input = r#"<?xml version="1.0"?>
<gpx><trk><trkseg>
  <trkpt lat="46.0" lon="13.0">
    <ele>1741.6</ele>
    <time>2026-05-02T10:00:00Z</time>
  </trkpt>
</trkseg></trk></gpx>"#;
        let t = parse_str(input).expect("parse");
        assert_eq!(t.points[0].geo_alt, 17_416);
    }
}
