//! `.tengri` envelope round-trip: build a Track in memory, encode → wrap →
//! write to bytes → read back → decode → assert identity. Plus a couple of
//! header-validation cases.

use tengri_server::flight::{
    Metadata, TengriError, TengriFile, Track, TrackPoint, decode, encode,
    tengri::{MAGIC, VERSION},
};

fn make_envelope(track_pts: Track) -> TengriFile {
    TengriFile::new(Metadata::default(), encode(&track_pts).unwrap())
}

fn pt(time: u32, lat: i32, lon: i32, geo_alt: i32, pressure_alt: Option<i32>) -> TrackPoint {
    TrackPoint {
        time,
        lat,
        lon,
        geo_alt,
        pressure_alt,
    }
}

fn sample_track() -> Track {
    Track {
        start_time: 1_700_000_000,
        points: vec![
            pt(1_700_000_000, 4_677_248, 1_314_815, 17_350, Some(16_640)),
            pt(1_700_000_001, 4_677_251, 1_314_817, 17_355, Some(16_645)),
            pt(1_700_000_002, 4_677_254, 1_314_819, 17_360, Some(16_650)),
        ],
    }
}

#[test]
fn round_trip_through_bytes() {
    let track = sample_track();
    let envelope = make_envelope(track.clone());

    let mut buf = Vec::new();
    envelope.write(&mut buf).unwrap();

    let read = TengriFile::read(buf.as_slice()).unwrap();
    assert_eq!(read.version, VERSION);
    assert_eq!(read.track, envelope.track);
    assert_eq!(decode(&read.track).unwrap(), track);
}

#[test]
fn http_form_round_trip() {
    let track = sample_track();
    let envelope = make_envelope(track.clone());

    let bytes = envelope.to_http_bytes().unwrap();
    // HTTP form is a bare gzip stream — it must start with the gzip magic
    // (\x1f\x8b) so browsers / Content-Encoding: gzip handle it natively.
    assert_eq!(&bytes[0..2], &[0x1f, 0x8b], "expected gzip magic");

    let read = TengriFile::read_http(bytes.as_slice()).unwrap();
    assert_eq!(read.version, VERSION);
    assert_eq!(decode(&read.track).unwrap(), track);
}

#[test]
fn header_layout_is_stable() {
    let envelope = make_envelope(sample_track());
    let mut buf = Vec::new();
    envelope.write(&mut buf).unwrap();

    assert_eq!(&buf[0..4], &MAGIC, "magic must be at offset 0");
    assert_eq!(&buf[4..6], &VERSION.to_le_bytes(), "version is u16 LE");
}

#[test]
fn rejects_bad_magic() {
    let mut buf = b"NOPE".to_vec();
    buf.extend(VERSION.to_le_bytes());
    let err = TengriFile::read(buf.as_slice()).unwrap_err();
    assert!(matches!(err, TengriError::BadMagic), "got {err:?}");
}

#[test]
fn rejects_unsupported_version() {
    let mut buf = MAGIC.to_vec();
    buf.extend((VERSION + 1).to_le_bytes());
    let err = TengriFile::read(buf.as_slice()).unwrap_err();
    assert!(
        matches!(err, TengriError::UnsupportedVersion { .. }),
        "got {err:?}"
    );
}
