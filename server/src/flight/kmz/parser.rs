//! KMZ is just `zip(doc.kml [+ assets])`. We unzip in memory, find the
//! KML entry, and hand its bytes to the existing KML parser.
//!
//! Entry selection rule (matching the KMZ convention shipped by Google
//! Earth, GPSBabel, gpsbabel, ogr2ogr, igc2kmz, …):
//! 1. Prefer an entry literally named `doc.kml` at the archive root.
//! 2. Else take the first entry (in archive order) whose name ends with
//!    `.kml`, case-insensitive.
//! 3. Else fail with `NoKmlEntry`.
//!
//! Asset entries (images, overlays, …) are ignored. Nested zips are not
//! followed.

use std::io::{Cursor, Read};

use zip::ZipArchive;

use crate::flight::Track;
use crate::flight::kml;

use super::error::KmzError;

pub fn parse_bytes(bytes: &[u8]) -> Result<Track, KmzError> {
    let kml_bytes = extract_kml_bytes(bytes)?;
    Ok(kml::parse_bytes(&kml_bytes)?)
}

/// Crack the KMZ open and return the bytes of its KML entry. Exposed as
/// a separate function so ingestion code can store the *inner* KML in
/// `flight_sources` (keeps the source format enum a tidy `igc/gpx/kml`
/// rather than sprouting `kmz` for what is purely a transport wrapper).
pub fn extract_kml_bytes(bytes: &[u8]) -> Result<Vec<u8>, KmzError> {
    if bytes.is_empty() {
        return Err(KmzError::Empty);
    }
    let mut zip = ZipArchive::new(Cursor::new(bytes))?;

    let entry_index = pick_kml_entry_index(&mut zip).ok_or(KmzError::NoKmlEntry)?;
    let mut entry = zip.by_index(entry_index)?;
    let mut buf = Vec::with_capacity(entry.size() as usize);
    entry.read_to_end(&mut buf).map_err(KmzError::ReadEntry)?;
    Ok(buf)
}

fn pick_kml_entry_index<R: std::io::Read + std::io::Seek>(
    zip: &mut ZipArchive<R>,
) -> Option<usize> {
    let mut first_kml: Option<usize> = None;
    for i in 0..zip.len() {
        // Use `name()` (raw archive path). KMZ entries shipped by every
        // toolchain we care about use ASCII-safe paths, so we don't need
        // the heavier `enclosed_name` sandboxing here — we never write
        // these bytes to disk.
        let Ok(file) = zip.by_index(i) else { continue };
        if file.is_dir() {
            continue;
        }
        let name = file.name();
        if name == "doc.kml" {
            return Some(i);
        }
        if first_kml.is_none() && name.to_ascii_lowercase().ends_with(".kml") {
            first_kml = Some(i);
        }
    }
    first_kml
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::ZipWriter;
    use zip::write::SimpleFileOptions;

    fn make_kmz(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut out = Vec::new();
        {
            let mut zw = ZipWriter::new(Cursor::new(&mut out));
            let opts =
                SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
            for (name, body) in entries {
                zw.start_file(*name, opts).unwrap();
                zw.write_all(body).unwrap();
            }
            zw.finish().unwrap();
        }
        out
    }

    /// A minimal valid GpsDumpAndroid-flavored KML; cheaper to embed
    /// here than to wire up a fixture file just for the KMZ tests.
    const SAMPLE_KML: &str = r#"<?xml version="1.0"?>
<Document>
  <Folder>
    <Placemark>
      <Metadata type="track">
        <FsInfo time_of_first_point="2026-05-02T10:54:33Z">
          <SecondsFromTimeOfFirstPoint>0 1</SecondsFromTimeOfFirstPoint>
        </FsInfo>
      </Metadata>
      <LineString>
        <coordinates>13.0,46.0,1500 13.1,46.1,1501</coordinates>
      </LineString>
    </Placemark>
  </Folder>
</Document>"#;

    #[test]
    fn rejects_empty() {
        assert!(matches!(parse_bytes(&[]), Err(KmzError::Empty)));
    }

    #[test]
    fn rejects_non_zip() {
        assert!(matches!(
            parse_bytes(b"not a zip"),
            Err(KmzError::InvalidZip(_))
        ));
    }

    #[test]
    fn parses_doc_kml_entry() {
        let kmz = make_kmz(&[("doc.kml", SAMPLE_KML.as_bytes())]);
        let t = parse_bytes(&kmz).expect("parse");
        assert_eq!(t.points.len(), 2);
    }

    #[test]
    fn prefers_doc_kml_over_other_kml() {
        // doc.kml is the spec-canonical entry; even if another *.kml
        // appears first in the archive, doc.kml wins.
        let kmz = make_kmz(&[
            ("other.kml", b"<bogus/>"),
            ("doc.kml", SAMPLE_KML.as_bytes()),
        ]);
        let t = parse_bytes(&kmz).expect("parse");
        assert_eq!(t.points.len(), 2);
    }

    #[test]
    fn falls_back_to_first_kml_when_no_doc_kml() {
        let kmz = make_kmz(&[
            ("images/icon.png", b"\x89PNG\r\n\x1a\n"),
            ("Track.KML", SAMPLE_KML.as_bytes()),
        ]);
        let t = parse_bytes(&kmz).expect("parse");
        assert_eq!(t.points.len(), 2);
    }

    #[test]
    fn rejects_archive_without_kml() {
        let kmz = make_kmz(&[("images/icon.png", b"\x89PNG\r\n\x1a\n")]);
        assert!(matches!(parse_bytes(&kmz), Err(KmzError::NoKmlEntry)));
    }

    #[test]
    fn surfaces_inner_kml_errors() {
        let kmz = make_kmz(&[("doc.kml", b"<root><foo/></root>")]);
        // <root> isn't any KML flavor we recognise.
        assert!(matches!(
            parse_bytes(&kmz),
            Err(KmzError::InnerKml(crate::flight::KmlError::NoTrack))
        ));
    }

    #[test]
    fn extract_kml_bytes_returns_inner_payload() {
        let kmz = make_kmz(&[("doc.kml", SAMPLE_KML.as_bytes())]);
        let inner = extract_kml_bytes(&kmz).expect("extract");
        assert_eq!(inner, SAMPLE_KML.as_bytes());
    }
}
