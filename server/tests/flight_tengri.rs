//! `.tengri` archive round-trip: build a Track in memory, encode → wrap →
//! write to bytes → read back → decode → assert identity. Plus a couple of
//! header-validation cases.

use tengri_server::flight::{
    Metadata, TengriError, TengriFile, Track, TrackPoint, decode, encode,
    tengri::{MAGIC, VERSION},
};

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
    let compact = encode(&track).unwrap();

    let archive = TengriFile {
        metadata: Metadata::default(),
        track: compact.clone(),
    };

    let mut buf = Vec::new();
    archive.write(&mut buf).unwrap();

    let read = TengriFile::read(buf.as_slice()).unwrap();
    assert_eq!(read.track, compact);
    assert_eq!(decode(&read.track).unwrap(), track);
}

#[test]
fn header_layout_is_stable() {
    // Sanity-check the framing bytes so a future format tweak can't silently
    // change them.
    let archive = TengriFile {
        metadata: Metadata::default(),
        track: encode(&sample_track()).unwrap(),
    };
    let mut buf = Vec::new();
    archive.write(&mut buf).unwrap();

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
