//! KML parser. Three flavors are accepted:
//!
//! 1. **GpsDumpAndroid** track Placemark: identified by a child
//!    `<Metadata src="..." type="track">` element. The Placemark holds
//!    `time_of_first_point="..."` (ISO-8601 Z) plus a custom
//!    `<SecondsFromTimeOfFirstPoint>` element with whitespace-separated
//!    integer deltas, and a `<LineString><coordinates>` block of
//!    `lon,lat,alt` triplets. The two sequences must have equal length.
//!
//! 2. **Standard `<gx:Track>`**: paired children — a `<when>` (ISO-8601
//!    timestamp) followed by a `<gx:coord>` (`lon lat alt`, space
//!    separated) for every fix, in document order. Used by Google Earth
//!    and several flight planners.
//!
//! 3. **GPSBabel / OGR `track_points`**: a `<Folder name="track_points">`
//!    holding one `<Placemark>` per fix. Each Placemark carries a 2D
//!    `<Point><coordinates>lon,lat</coordinates></Point>` and an
//!    `<ExtendedData><SchemaData>` block with `track_fid`, `track_seg_id`,
//!    `track_seg_point_id`, `ele` (metres), and `time` (`YYYY/MM/DD
//!    HH:MM:SS+ZZ`). Multiple `track_seg_id`s within the first
//!    `track_fid` are concatenated in document order.
//!
//!    **Caveat.** GPSBabel converting an IGC emits *two* `track_fid`s,
//!    one per altitude channel (pressure / GPS), losing which is which.
//!    We take the first `track_fid` only. For native GPX/CSV input the
//!    file has one `track_fid` and there's no ambiguity.
//!
//! Other Placemarks (FAI distance lines, waypoints, the trip summary)
//! are ignored. We do not extract `<Metadata>` payloads — that data
//! belongs on the metadata pipeline, not on the geometry track.
//!
//! # Coordinate / altitude conversion
//!
//! KML coordinates are decimal degrees `lon,lat,alt`. We convert:
//! - lat / lon → E5 micro-degrees (deg × 10⁵, rounded), matching the
//!   IGC parser.
//! - alt       → decimeters (m × 10), matching the rest of the pipeline.
//!
//! # Altitude channel
//!
//! KML carries one altitude column. GpsDumpAndroid's source is GPS
//! altitude even when the recorder has a barometer; standard `<gx:Track>`
//! is also GPS-derived. We always set `geo_alt` and leave
//! `pressure_alt = None`. TAS is never present in KML and is always None.

use roxmltree::{Document, Node};

use crate::flight::geo_text::{deg_to_e5, m_to_dm, parse_gpsbabel_time_u32, parse_iso8601_u32};
use crate::flight::types::{Track, TrackPoint};

use super::error::KmlError;

pub fn parse_bytes(bytes: &[u8]) -> Result<Track, KmlError> {
    if bytes.is_empty() {
        return Err(KmlError::Empty);
    }
    let text = std::str::from_utf8(bytes)?;
    parse_str(text)
}

pub fn parse_str(input: &str) -> Result<Track, KmlError> {
    if input.trim().is_empty() {
        return Err(KmlError::Empty);
    }
    let doc = Document::parse(input)?;

    if let Some(track_pm) = find_gpsdump_track_placemark(&doc) {
        return parse_gpsdump_track(track_pm);
    }
    if let Some(gx_track) = find_gx_track(&doc) {
        return parse_gx_track(gx_track);
    }
    if let Some(folder) = find_track_points_folder(&doc) {
        return parse_gpsbabel_track_points(folder);
    }
    Err(KmlError::NoTrack)
}

// ── GpsDumpAndroid flavor ──────────────────────────────────────────────────

/// Find a `<Placemark>` whose immediate `<Metadata>` child has
/// `type="track"`. Match is case-sensitive, namespace-agnostic
/// (`local_name` only) — KML in the wild rarely declares prefixes on
/// these elements but we don't want to be brittle to that.
fn find_gpsdump_track_placemark<'a, 'input: 'a>(
    doc: &'a Document<'input>,
) -> Option<Node<'a, 'input>> {
    doc.descendants()
        .filter(|n| n.is_element() && n.tag_name().name() == "Placemark")
        .find(|pm| {
            pm.children()
                .filter(|n| n.is_element() && n.tag_name().name() == "Metadata")
                .any(|m| m.attribute("type") == Some("track"))
        })
}

fn parse_gpsdump_track(pm: Node) -> Result<Track, KmlError> {
    let metadata = pm
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "Metadata")
        .ok_or(KmlError::MissingElement("Metadata"))?;

    let fs_info = metadata
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "FsInfo")
        .ok_or(KmlError::MissingElement("Metadata/FsInfo"))?;

    let time_of_first = fs_info
        .attribute("time_of_first_point")
        .ok_or(KmlError::MissingElement("FsInfo/@time_of_first_point"))?;
    let start = parse_iso8601_z(time_of_first, 0)?;

    let seconds_text = fs_info
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "SecondsFromTimeOfFirstPoint")
        .ok_or(KmlError::MissingElement(
            "FsInfo/SecondsFromTimeOfFirstPoint",
        ))?
        .text()
        .ok_or(KmlError::MissingElement(
            "FsInfo/SecondsFromTimeOfFirstPoint (text)",
        ))?;

    let line_string = pm
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "LineString")
        .ok_or(KmlError::MissingElement("Placemark/LineString"))?;
    let coords_text = line_string
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "coordinates")
        .ok_or(KmlError::MissingElement("LineString/coordinates"))?
        .text()
        .ok_or(KmlError::MissingElement("LineString/coordinates (text)"))?;

    let deltas = parse_seconds_deltas(seconds_text)?;
    let coords = parse_coordinates(coords_text)?;

    if deltas.len() != coords.len() {
        return Err(KmlError::LengthMismatch {
            times: deltas.len(),
            coords: coords.len(),
        });
    }
    if coords.is_empty() {
        return Err(KmlError::NoFixes);
    }

    let points: Vec<TrackPoint> = deltas
        .into_iter()
        .zip(coords)
        .map(|(dt, (lon_e5, lat_e5, geo_alt))| TrackPoint {
            time: start.saturating_add(dt),
            lat: lat_e5,
            lon: lon_e5,
            geo_alt,
            pressure_alt: None,
            tas: None,
        })
        .collect();

    Ok(Track {
        start_time: points[0].time,
        points,
    })
}

fn parse_seconds_deltas(text: &str) -> Result<Vec<u32>, KmlError> {
    text.split_ascii_whitespace()
        .enumerate()
        .map(|(i, tok)| {
            tok.parse::<u32>().map_err(|e| KmlError::BadTime {
                index: i,
                reason: format!("expected non-negative integer seconds, got {tok:?}: {e}"),
            })
        })
        .collect()
}

// ── Standard <gx:Track> flavor ─────────────────────────────────────────────

fn find_gx_track<'a, 'input: 'a>(doc: &'a Document<'input>) -> Option<Node<'a, 'input>> {
    doc.descendants()
        .find(|n| n.is_element() && n.tag_name().name() == "Track")
}

fn parse_gx_track(track: Node) -> Result<Track, KmlError> {
    // Walk children in document order. KML spec: each fix is a `<when>`
    // immediately paired with a `<gx:coord>`. Some emitters group all
    // `<when>`s first then all `<gx:coord>`s, but the spec is the
    // pairwise form. We accept the pairwise order strictly.
    let mut times: Vec<u32> = Vec::new();
    let mut coords: Vec<(i32, i32, i32)> = Vec::new();

    for child in track.children().filter(Node::is_element) {
        match child.tag_name().name() {
            "when" => {
                let text = child
                    .text()
                    .ok_or(KmlError::MissingElement("when (text)"))?;
                let t = parse_iso8601_z(text.trim(), times.len())?;
                times.push(t);
            }
            "coord" => {
                let text = child
                    .text()
                    .ok_or(KmlError::MissingElement("coord (text)"))?;
                let triple = parse_coord_space_separated(text.trim(), coords.len())?;
                coords.push(triple);
            }
            _ => continue,
        }
    }

    if times.len() != coords.len() {
        return Err(KmlError::LengthMismatch {
            times: times.len(),
            coords: coords.len(),
        });
    }
    if coords.is_empty() {
        return Err(KmlError::NoFixes);
    }

    let points: Vec<TrackPoint> = times
        .into_iter()
        .zip(coords)
        .map(|(t, (lon_e5, lat_e5, geo_alt))| TrackPoint {
            time: t,
            lat: lat_e5,
            lon: lon_e5,
            geo_alt,
            pressure_alt: None,
            tas: None,
        })
        .collect();

    Ok(Track {
        start_time: points[0].time,
        points,
    })
}

// ── GPSBabel / OGR `track_points` flavor ───────────────────────────────────

fn find_track_points_folder<'a, 'input: 'a>(doc: &'a Document<'input>) -> Option<Node<'a, 'input>> {
    doc.descendants()
        .filter(|n| n.is_element() && n.tag_name().name() == "Folder")
        .find(|f| {
            f.children()
                .filter(|n| n.is_element() && n.tag_name().name() == "name")
                .any(|name| name.text().map(str::trim) == Some("track_points"))
        })
}

fn parse_gpsbabel_track_points(folder: Node) -> Result<Track, KmlError> {
    // Walk Placemarks in document order. Within `track_points` each
    // Placemark is one fix; the Folder may contain other elements
    // (Style, name, …) which we skip.
    //
    // We keep only fixes whose `track_fid` matches the first one we see.
    // For IGC-sourced KMZ this drops the duplicate altitude channel;
    // for native single-track files it's a no-op.
    let mut points: Vec<TrackPoint> = Vec::new();
    let mut first_track_fid: Option<&str> = None;

    for pm in folder
        .children()
        .filter(|n| n.is_element() && n.tag_name().name() == "Placemark")
    {
        let fields = read_track_point_fields(pm, points.len())?;

        match first_track_fid {
            None => first_track_fid = Some(fields.track_fid),
            Some(fid) if fid != fields.track_fid => continue,
            Some(_) => {}
        }

        let (lon_e5, lat_e5) = parse_track_point_coords(pm, points.len())?;
        let geo_alt = m_to_dm(fields.ele);
        let time = parse_gpsbabel_time_u32(fields.time).map_err(|reason| KmlError::BadTime {
            index: points.len(),
            reason,
        })?;

        points.push(TrackPoint {
            time,
            lat: lat_e5,
            lon: lon_e5,
            geo_alt,
            pressure_alt: None,
            tas: None,
        });
    }

    if points.is_empty() {
        return Err(KmlError::NoFixes);
    }
    Ok(Track {
        start_time: points[0].time,
        points,
    })
}

struct TrackPointFields<'a> {
    track_fid: &'a str,
    ele: f64,
    time: &'a str,
}

fn read_track_point_fields<'a, 'input: 'a>(
    pm: Node<'a, 'input>,
    index: usize,
) -> Result<TrackPointFields<'a>, KmlError> {
    let schema = pm
        .descendants()
        .find(|n| n.is_element() && n.tag_name().name() == "SchemaData")
        .ok_or(KmlError::MissingElement(
            "track_points/Placemark/ExtendedData/SchemaData",
        ))?;

    let mut track_fid: Option<&str> = None;
    let mut ele: Option<&str> = None;
    let mut time: Option<&str> = None;

    for sd in schema
        .children()
        .filter(|n| n.is_element() && n.tag_name().name() == "SimpleData")
    {
        let Some(name) = sd.attribute("name") else {
            continue;
        };
        let Some(text) = sd.text() else { continue };
        match name {
            "track_fid" => track_fid = Some(text),
            "ele" => ele = Some(text),
            "time" => time = Some(text),
            _ => {}
        }
    }

    let track_fid = track_fid.ok_or(KmlError::MissingElement(
        "track_points/Placemark/SimpleData[@name=\"track_fid\"]",
    ))?;
    let ele_str = ele.ok_or(KmlError::MissingElement(
        "track_points/Placemark/SimpleData[@name=\"ele\"]",
    ))?;
    let time = time.ok_or(KmlError::MissingElement(
        "track_points/Placemark/SimpleData[@name=\"time\"]",
    ))?;

    let ele = ele_str
        .trim()
        .parse::<f64>()
        .map_err(|e| KmlError::BadCoord {
            index,
            reason: format!("bad ele {ele_str:?}: {e}"),
        })?;
    if !ele.is_finite() {
        return Err(KmlError::BadCoord {
            index,
            reason: format!("non-finite ele {ele_str:?}"),
        });
    }

    Ok(TrackPointFields {
        track_fid,
        ele,
        time,
    })
}

fn parse_track_point_coords(pm: Node, index: usize) -> Result<(i32, i32), KmlError> {
    let coords = pm
        .descendants()
        .find(|n| n.is_element() && n.tag_name().name() == "coordinates")
        .and_then(|c| c.text())
        .ok_or(KmlError::MissingElement(
            "track_points/Placemark/Point/coordinates",
        ))?
        .trim();

    // GPSBabel writes 2D `lon,lat`; we accept an optional altitude
    // component for resilience against forks that include it (we'd
    // ignore it since the authoritative altitude is in `ele`).
    let mut parts = coords.split(',');
    let lon = parts.next().ok_or_else(|| KmlError::BadCoord {
        index,
        reason: format!("missing lon in {coords:?}"),
    })?;
    let lat = parts.next().ok_or_else(|| KmlError::BadCoord {
        index,
        reason: format!("missing lat in {coords:?}"),
    })?;
    let lon_deg = lon.trim().parse::<f64>().map_err(|e| KmlError::BadCoord {
        index,
        reason: format!("bad lon {lon:?}: {e}"),
    })?;
    let lat_deg = lat.trim().parse::<f64>().map_err(|e| KmlError::BadCoord {
        index,
        reason: format!("bad lat {lat:?}: {e}"),
    })?;
    if !lon_deg.is_finite() || !lat_deg.is_finite() {
        return Err(KmlError::BadCoord {
            index,
            reason: format!("non-finite coord(s) in {coords:?}"),
        });
    }
    Ok((deg_to_e5(lon_deg), deg_to_e5(lat_deg)))
}

// ── Shared coordinate / time helpers ───────────────────────────────────────

/// Parse a KML `<coordinates>` block: triplets `lon,lat[,alt]`,
/// triplets separated by ASCII whitespace (spaces, tabs, newlines).
/// Altitude is optional in the KML spec; we treat missing as 0 m.
fn parse_coordinates(text: &str) -> Result<Vec<(i32, i32, i32)>, KmlError> {
    text.split_ascii_whitespace()
        .enumerate()
        .map(|(i, tok)| parse_coord_csv(tok, i))
        .collect()
}

fn parse_coord_csv(tok: &str, index: usize) -> Result<(i32, i32, i32), KmlError> {
    let mut parts = tok.split(',');
    let lon = parts.next().ok_or_else(|| KmlError::BadCoord {
        index,
        reason: format!("missing lon in {tok:?}"),
    })?;
    let lat = parts.next().ok_or_else(|| KmlError::BadCoord {
        index,
        reason: format!("missing lat in {tok:?}"),
    })?;
    let alt = parts.next().unwrap_or("0");
    if parts.next().is_some() {
        return Err(KmlError::BadCoord {
            index,
            reason: format!("expected 2 or 3 comma-separated values, got more in {tok:?}"),
        });
    }
    triple_to_e5(lon, lat, alt, index)
}

fn parse_coord_space_separated(tok: &str, index: usize) -> Result<(i32, i32, i32), KmlError> {
    let mut parts = tok.split_ascii_whitespace();
    let lon = parts.next().ok_or_else(|| KmlError::BadCoord {
        index,
        reason: format!("missing lon in {tok:?}"),
    })?;
    let lat = parts.next().ok_or_else(|| KmlError::BadCoord {
        index,
        reason: format!("missing lat in {tok:?}"),
    })?;
    let alt = parts.next().unwrap_or("0");
    if parts.next().is_some() {
        return Err(KmlError::BadCoord {
            index,
            reason: format!("expected 2 or 3 space-separated values, got more in {tok:?}"),
        });
    }
    triple_to_e5(lon, lat, alt, index)
}

fn triple_to_e5(
    lon: &str,
    lat: &str,
    alt: &str,
    index: usize,
) -> Result<(i32, i32, i32), KmlError> {
    let lon_deg = lon.parse::<f64>().map_err(|e| KmlError::BadCoord {
        index,
        reason: format!("bad lon {lon:?}: {e}"),
    })?;
    let lat_deg = lat.parse::<f64>().map_err(|e| KmlError::BadCoord {
        index,
        reason: format!("bad lat {lat:?}: {e}"),
    })?;
    let alt_m = alt.parse::<f64>().map_err(|e| KmlError::BadCoord {
        index,
        reason: format!("bad alt {alt:?}: {e}"),
    })?;
    if !lon_deg.is_finite() || !lat_deg.is_finite() || !alt_m.is_finite() {
        return Err(KmlError::BadCoord {
            index,
            reason: format!("non-finite value(s) in {lon:?},{lat:?},{alt:?}"),
        });
    }
    Ok((deg_to_e5(lon_deg), deg_to_e5(lat_deg), m_to_dm(alt_m)))
}

fn parse_iso8601_z(s: &str, index: usize) -> Result<u32, KmlError> {
    parse_iso8601_u32(s).map_err(|reason| KmlError::BadTime { index, reason })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::DateTime;

    #[test]
    fn rejects_empty() {
        assert!(matches!(parse_str(""), Err(KmlError::Empty)));
        assert!(matches!(parse_str("   \n  "), Err(KmlError::Empty)));
    }

    #[test]
    fn rejects_non_kml() {
        let input = "<root><foo/></root>";
        assert!(matches!(parse_str(input), Err(KmlError::NoTrack)));
    }

    #[test]
    fn parses_gpsdump_minimal() {
        let input = r#"<?xml version="1.0"?>
<Document>
  <Folder>
    <Placemark>
      <Metadata src="GpsDumpAndroid" v="4.72" type="track">
        <FsInfo time_of_first_point="2026-05-02T10:54:33Z">
          <SecondsFromTimeOfFirstPoint>0 1 3</SecondsFromTimeOfFirstPoint>
        </FsInfo>
      </Metadata>
      <LineString>
        <coordinates>
          13.166333,46.765998,1741
          13.166400,46.766000,1742
          13.166500,46.766100,1743
        </coordinates>
      </LineString>
    </Placemark>
  </Folder>
</Document>"#;
        let t = parse_str(input).expect("parse");
        assert_eq!(t.points.len(), 3);
        // 2026-05-02T10:54:33Z = 1_777_805_673 (let chrono be the oracle).
        let expected_start = DateTime::parse_from_rfc3339("2026-05-02T10:54:33Z")
            .unwrap()
            .timestamp() as u32;
        assert_eq!(t.start_time, expected_start);
        assert_eq!(t.points[0].time, expected_start);
        assert_eq!(t.points[1].time, expected_start + 1);
        assert_eq!(t.points[2].time, expected_start + 3);
        // 13.166333° lon, 46.765998° lat → E5
        assert_eq!(t.points[0].lon, 1_316_633);
        assert_eq!(t.points[0].lat, 4_676_600);
        assert_eq!(t.points[0].geo_alt, 17_410);
        assert!(t.points.iter().all(|p| p.pressure_alt.is_none()));
        assert!(t.points.iter().all(|p| p.tas.is_none()));
    }

    #[test]
    fn parses_gpsdump_skips_other_placemarks() {
        // FAI-distance Placemark first; track Placemark second. Parser
        // must ignore the FAI line.
        let input = r#"<?xml version="1.0"?>
<Document>
  <Folder>
    <Placemark>
      <Metadata type="distance_5_point"><FsInfo track_idx="1 2"/></Metadata>
      <LineString><coordinates>0,0,0 1,1,1</coordinates></LineString>
    </Placemark>
    <Placemark>
      <Metadata type="track">
        <FsInfo time_of_first_point="2026-05-02T10:54:33Z">
          <SecondsFromTimeOfFirstPoint>0 1</SecondsFromTimeOfFirstPoint>
        </FsInfo>
      </Metadata>
      <LineString><coordinates>13.0,46.0,1500 13.1,46.1,1501</coordinates></LineString>
    </Placemark>
  </Folder>
</Document>"#;
        let t = parse_str(input).expect("parse");
        assert_eq!(t.points.len(), 2);
        assert_eq!(t.points[0].lat, 4_600_000);
    }

    #[test]
    fn rejects_gpsdump_length_mismatch() {
        let input = r#"<?xml version="1.0"?>
<Placemark>
  <Metadata type="track">
    <FsInfo time_of_first_point="2026-05-02T10:54:33Z">
      <SecondsFromTimeOfFirstPoint>0 1 2</SecondsFromTimeOfFirstPoint>
    </FsInfo>
  </Metadata>
  <LineString><coordinates>13.0,46.0,1500 13.1,46.1,1501</coordinates></LineString>
</Placemark>"#;
        assert!(matches!(
            parse_str(input),
            Err(KmlError::LengthMismatch {
                times: 3,
                coords: 2
            })
        ));
    }

    #[test]
    fn parses_gx_track_paired_when_coord() {
        let input = r#"<?xml version="1.0"?>
<kml xmlns="http://www.opengis.net/kml/2.2" xmlns:gx="http://www.google.com/kml/ext/2.2">
  <Placemark>
    <gx:Track>
      <when>2026-05-02T10:54:33Z</when>
      <gx:coord>13.166333 46.765998 1741</gx:coord>
      <when>2026-05-02T10:54:34Z</when>
      <gx:coord>13.166400 46.766000 1742</gx:coord>
    </gx:Track>
  </Placemark>
</kml>"#;
        let t = parse_str(input).expect("parse");
        assert_eq!(t.points.len(), 2);
        let expected_start = DateTime::parse_from_rfc3339("2026-05-02T10:54:33Z")
            .unwrap()
            .timestamp() as u32;
        assert_eq!(t.start_time, expected_start);
        assert_eq!(t.points[1].time, expected_start + 1);
        assert_eq!(t.points[0].lon, 1_316_633);
        assert_eq!(t.points[0].lat, 4_676_600);
        assert_eq!(t.points[0].geo_alt, 17_410);
    }

    #[test]
    fn rejects_gx_track_length_mismatch() {
        let input = r#"<?xml version="1.0"?>
<kml xmlns:gx="http://www.google.com/kml/ext/2.2">
  <gx:Track>
    <when>2026-05-02T10:54:33Z</when>
    <when>2026-05-02T10:54:34Z</when>
    <gx:coord>13.0 46.0 1500</gx:coord>
  </gx:Track>
</kml>"#;
        assert!(matches!(
            parse_str(input),
            Err(KmlError::LengthMismatch {
                times: 2,
                coords: 1
            })
        ));
    }

    #[test]
    fn rejects_gx_track_no_fixes() {
        let input = r#"<?xml version="1.0"?>
<kml xmlns:gx="http://www.google.com/kml/ext/2.2">
  <gx:Track></gx:Track>
</kml>"#;
        assert!(matches!(parse_str(input), Err(KmlError::NoFixes)));
    }

    #[test]
    fn coord_with_only_two_components_defaults_alt_to_zero() {
        let input = r#"<?xml version="1.0"?>
<Placemark>
  <Metadata type="track">
    <FsInfo time_of_first_point="2026-05-02T10:54:33Z">
      <SecondsFromTimeOfFirstPoint>0</SecondsFromTimeOfFirstPoint>
    </FsInfo>
  </Metadata>
  <LineString><coordinates>13.0,46.0</coordinates></LineString>
</Placemark>"#;
        let t = parse_str(input).expect("parse");
        assert_eq!(t.points.len(), 1);
        assert_eq!(t.points[0].geo_alt, 0);
    }

    #[test]
    fn rejects_invalid_iso_timestamp() {
        let input = r#"<?xml version="1.0"?>
<Placemark>
  <Metadata type="track">
    <FsInfo time_of_first_point="not-a-date">
      <SecondsFromTimeOfFirstPoint>0</SecondsFromTimeOfFirstPoint>
    </FsInfo>
  </Metadata>
  <LineString><coordinates>13.0,46.0,1500</coordinates></LineString>
</Placemark>"#;
        assert!(matches!(parse_str(input), Err(KmlError::BadTime { .. })));
    }

    // ── GPSBabel / OGR `track_points` flavor ──────────────────────────────

    #[test]
    fn parses_gpsbabel_track_points_minimal() {
        let input = r##"<?xml version="1.0" encoding="utf-8" ?>
<kml xmlns="http://www.opengis.net/kml/2.2">
<Document>
  <Folder><name>track_points</name>
    <Placemark>
      <ExtendedData><SchemaData schemaUrl="#track_points">
        <SimpleData name="track_fid">0</SimpleData>
        <SimpleData name="track_seg_id">0</SimpleData>
        <SimpleData name="track_seg_point_id">0</SimpleData>
        <SimpleData name="ele">1620</SimpleData>
        <SimpleData name="time">2026/05/02 10:54:33+00</SimpleData>
      </SchemaData></ExtendedData>
      <Point><coordinates>13.166333,46.765998</coordinates></Point>
    </Placemark>
    <Placemark>
      <ExtendedData><SchemaData schemaUrl="#track_points">
        <SimpleData name="track_fid">0</SimpleData>
        <SimpleData name="track_seg_id">0</SimpleData>
        <SimpleData name="track_seg_point_id">1</SimpleData>
        <SimpleData name="ele">1621.5</SimpleData>
        <SimpleData name="time">2026/05/02 10:54:34+00</SimpleData>
      </SchemaData></ExtendedData>
      <Point><coordinates>13.166400,46.766000</coordinates></Point>
    </Placemark>
  </Folder>
</Document>
</kml>"##;
        let t = parse_str(input).expect("parse");
        assert_eq!(t.points.len(), 2);
        let expected_start = DateTime::parse_from_rfc3339("2026-05-02T10:54:33Z")
            .unwrap()
            .timestamp() as u32;
        assert_eq!(t.start_time, expected_start);
        assert_eq!(t.points[1].time, expected_start + 1);
        assert_eq!(t.points[0].lon, 1_316_633);
        assert_eq!(t.points[0].lat, 4_676_600);
        assert_eq!(t.points[0].geo_alt, 16_200);
        assert_eq!(t.points[1].geo_alt, 16_215);
        assert!(t.points.iter().all(|p| p.pressure_alt.is_none()));
        assert!(t.points.iter().all(|p| p.tas.is_none()));
    }

    #[test]
    fn gpsbabel_keeps_only_first_track_fid() {
        // gpsbabel's IGC→KMZ pipeline emits one track_fid per altitude
        // channel, with otherwise identical fixes. We must drop the
        // duplicate channel and not double-count.
        let input = r##"<?xml version="1.0" encoding="utf-8" ?>
<kml>
<Document>
  <Folder><name>track_points</name>
    <Placemark>
      <ExtendedData><SchemaData schemaUrl="#track_points">
        <SimpleData name="track_fid">0</SimpleData>
        <SimpleData name="track_seg_id">0</SimpleData>
        <SimpleData name="ele">100</SimpleData>
        <SimpleData name="time">2026/05/02 10:00:00+00</SimpleData>
      </SchemaData></ExtendedData>
      <Point><coordinates>13.0,46.0</coordinates></Point>
    </Placemark>
    <Placemark>
      <ExtendedData><SchemaData schemaUrl="#track_points">
        <SimpleData name="track_fid">0</SimpleData>
        <SimpleData name="track_seg_id">0</SimpleData>
        <SimpleData name="ele">101</SimpleData>
        <SimpleData name="time">2026/05/02 10:00:01+00</SimpleData>
      </SchemaData></ExtendedData>
      <Point><coordinates>13.0,46.0</coordinates></Point>
    </Placemark>
    <Placemark>
      <ExtendedData><SchemaData schemaUrl="#track_points">
        <SimpleData name="track_fid">1</SimpleData>
        <SimpleData name="track_seg_id">0</SimpleData>
        <SimpleData name="ele">200</SimpleData>
        <SimpleData name="time">2026/05/02 10:00:00+00</SimpleData>
      </SchemaData></ExtendedData>
      <Point><coordinates>13.0,46.0</coordinates></Point>
    </Placemark>
  </Folder>
</Document>
</kml>"##;
        let t = parse_str(input).expect("parse");
        assert_eq!(t.points.len(), 2, "only track_fid=0 fixes should be kept");
        assert_eq!(t.points[0].geo_alt, 1_000);
        assert_eq!(t.points[1].geo_alt, 1_010);
    }

    #[test]
    fn gpsbabel_concatenates_segments_within_first_track_fid() {
        // Two segments inside track_fid=0: both keep their order.
        let input = r##"<?xml version="1.0" encoding="utf-8" ?>
<kml>
<Document>
  <Folder><name>track_points</name>
    <Placemark>
      <ExtendedData><SchemaData schemaUrl="#track_points">
        <SimpleData name="track_fid">0</SimpleData>
        <SimpleData name="track_seg_id">0</SimpleData>
        <SimpleData name="ele">100</SimpleData>
        <SimpleData name="time">2026/05/02 10:00:00+00</SimpleData>
      </SchemaData></ExtendedData>
      <Point><coordinates>13.0,46.0</coordinates></Point>
    </Placemark>
    <Placemark>
      <ExtendedData><SchemaData schemaUrl="#track_points">
        <SimpleData name="track_fid">0</SimpleData>
        <SimpleData name="track_seg_id">1</SimpleData>
        <SimpleData name="ele">200</SimpleData>
        <SimpleData name="time">2026/05/02 11:00:00+00</SimpleData>
      </SchemaData></ExtendedData>
      <Point><coordinates>13.5,46.5</coordinates></Point>
    </Placemark>
  </Folder>
</Document>
</kml>"##;
        let t = parse_str(input).expect("parse");
        assert_eq!(t.points.len(), 2);
        assert_eq!(t.points[1].lon, 1_350_000);
    }

    #[test]
    fn gpsbabel_rejects_no_fixes() {
        // Folder name is recognised but it holds no Placemarks. We
        // surface NoFixes (we did detect the flavor; it just held nothing).
        let input = r##"<?xml version="1.0" encoding="utf-8" ?>
<kml><Document><Folder><name>track_points</name></Folder></Document></kml>"##;
        assert!(matches!(parse_str(input), Err(KmlError::NoFixes)));
    }

    #[test]
    fn gpsbabel_rejects_bad_time() {
        let input = r##"<?xml version="1.0" encoding="utf-8" ?>
<kml>
<Document>
  <Folder><name>track_points</name>
    <Placemark>
      <ExtendedData><SchemaData schemaUrl="#track_points">
        <SimpleData name="track_fid">0</SimpleData>
        <SimpleData name="track_seg_id">0</SimpleData>
        <SimpleData name="ele">100</SimpleData>
        <SimpleData name="time">not-a-date</SimpleData>
      </SchemaData></ExtendedData>
      <Point><coordinates>13.0,46.0</coordinates></Point>
    </Placemark>
  </Folder>
</Document>
</kml>"##;
        assert!(matches!(parse_str(input), Err(KmlError::BadTime { .. })));
    }
}
