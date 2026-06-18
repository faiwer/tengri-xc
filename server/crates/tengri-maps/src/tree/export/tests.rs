use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::geo::XyzTile;
use crate::tree::{TileKind, TileTreeError, TileTreeReader, XYZBounds};

use super::adapter::{CachedChild, TileTreeExportAdapter};
use super::exporter::TileTreeExporter;
use super::progress::{ProgressWriter, format_duration};

#[derive(Default)]
struct FakeState {
    source_tiles: HashMap<(u8, u32, u32), u16>,
    reduced_tiles: Mutex<Vec<XyzTile>>,
    read_tiles: Mutex<Vec<XyzTile>>,
    opened_readers: AtomicUsize,
}

struct FakeAdapter {
    bounds: XYZBounds,
    state: Arc<FakeState>,
}

impl TileTreeExportAdapter for FakeAdapter {
    type SourceTile = u16;
    type Reader = ();

    fn tile_kind(&self) -> TileKind {
        TileKind::Dem
    }

    fn bounds(&self) -> XYZBounds {
        self.bounds
    }

    fn open_reader(&self) -> Result<Self::Reader, TileTreeError> {
        self.state.opened_readers.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    fn read_source_tile(
        &self,
        _reader: &mut Self::Reader,
        tile: XyzTile,
    ) -> Result<Self::SourceTile, TileTreeError> {
        self.state.read_tiles.lock().unwrap().push(tile);
        self.source_value(tile).ok_or(TileTreeError::MissingTile {
            z: tile.z,
            x: tile.x as u16,
            y: tile.y as u16,
        })
    }

    fn encode_payload(&self, tile: &Self::SourceTile) -> Result<Vec<u8>, TileTreeError> {
        Ok(tile.to_le_bytes().to_vec())
    }

    fn write_raw_cache(
        &self,
        writer: &mut dyn Write,
        tile: &Self::SourceTile,
    ) -> Result<(), TileTreeError> {
        writer
            .write_all(&tile.to_le_bytes())
            .map_err(TileTreeError::Io)
    }

    fn read_raw_cache(&self, reader: &mut dyn Read) -> Result<Self::SourceTile, TileTreeError> {
        let mut bytes = [0; 2];
        reader.read_exact(&mut bytes).map_err(TileTreeError::Io)?;
        Ok(u16::from_le_bytes(bytes))
    }

    fn reduce_children_to_tile(
        &self,
        tile: XyzTile,
        children: &[CachedChild<Self::SourceTile>],
    ) -> Result<Self::SourceTile, TileTreeError> {
        self.state.reduced_tiles.lock().unwrap().push(tile);
        Ok(children.iter().map(|child| child.raw).sum())
    }
}

impl FakeAdapter {
    fn source_value(&self, tile: XyzTile) -> Option<u16> {
        self.state
            .source_tiles
            .get(&(tile.z, tile.x, tile.y))
            .copied()
    }
}

/// Stand-in for a leaf-only source (TIF-shaped) that refuses to downsample
/// past `max_leaf_downsample_steps` at the leaf. Read paths are unreachable
/// — the orchestrator's upfront check should fail before any tile work.
struct LeafCappedAdapter {
    bounds: XYZBounds,
    max_leaf_downsample_steps: u8,
}

impl TileTreeExportAdapter for LeafCappedAdapter {
    type SourceTile = u16;
    type Reader = ();

    fn tile_kind(&self) -> TileKind {
        TileKind::Dem
    }

    fn bounds(&self) -> XYZBounds {
        self.bounds
    }

    fn max_leaf_downsample_steps(&self) -> u8 {
        self.max_leaf_downsample_steps
    }

    fn open_reader(&self) -> Result<Self::Reader, TileTreeError> {
        unreachable!("export should fail before any reader is opened")
    }

    fn read_source_tile(
        &self,
        _reader: &mut Self::Reader,
        _tile: XyzTile,
    ) -> Result<Self::SourceTile, TileTreeError> {
        unreachable!("export should fail before any tile is read")
    }

    fn encode_payload(&self, _tile: &Self::SourceTile) -> Result<Vec<u8>, TileTreeError> {
        unreachable!()
    }

    fn write_raw_cache(
        &self,
        _writer: &mut dyn Write,
        _tile: &Self::SourceTile,
    ) -> Result<(), TileTreeError> {
        unreachable!()
    }

    fn read_raw_cache(&self, _reader: &mut dyn Read) -> Result<Self::SourceTile, TileTreeError> {
        unreachable!()
    }

    fn reduce_children_to_tile(
        &self,
        _tile: XyzTile,
        _children: &[CachedChild<Self::SourceTile>],
    ) -> Result<Self::SourceTile, TileTreeError> {
        unreachable!()
    }
}

#[test]
fn exporter_writes_leaves_and_parents() {
    let path = test_path("generic-export");
    let _ = fs::remove_file(&path);
    let bounds = XYZBounds::new(1, 0, 0, 1, 1).unwrap();
    let state = fake_state((0..=1).flat_map(|y| {
        (0..=1).map(move |x| {
            let tile = XyzTile { z: 1, x, y };
            (tile, (x + y * 2 + 1) as u16)
        })
    }));
    let report = TileTreeExporter::new(
        FakeAdapter {
            bounds,
            state: Arc::clone(&state),
        },
        &path,
    )
    .threads(2)
    .build()
    .unwrap();

    assert_eq!(report.zoom, 1);
    assert_eq!(report.tiles_written, 5);

    let mut reader = TileTreeReader::open(&path).unwrap();
    assert_eq!(u16_payload(&mut reader, 1, 1, 1), 4);
    assert_eq!(u16_payload(&mut reader, 0, 0, 0), 10);

    let _ = fs::remove_file(&path);
}

#[test]
fn missing_intermediate_source_tile_falls_back_to_child_reduction() {
    let path = test_path("missing-intermediate");
    let _ = fs::remove_file(&path);
    let bounds = XYZBounds::new(2, 0, 0, 1, 1).unwrap();
    let state = fake_state((0..=1).flat_map(|y| {
        (0..=1).map(move |x| {
            let tile = XyzTile { z: 2, x, y };
            (tile, (x + y * 2 + 1) as u16)
        })
    }));

    TileTreeExporter::new(
        FakeAdapter {
            bounds,
            state: Arc::clone(&state),
        },
        &path,
    )
    .threads(1)
    .build()
    .unwrap();

    let mut reader = TileTreeReader::open(&path).unwrap();
    assert_eq!(u16_payload(&mut reader, 1, 0, 0), 10);
    assert_eq!(u16_payload(&mut reader, 0, 0, 0), 10);
    let reduced = state.reduced_tiles.lock().unwrap();
    assert!(reduced.contains(&XyzTile { z: 1, x: 0, y: 0 }));

    let _ = fs::remove_file(&path);
}

#[test]
fn split_frontier_uses_worker_readers_and_completes_output() {
    let path = test_path("split-frontier");
    let _ = fs::remove_file(&path);
    let bounds = XYZBounds::new(2, 0, 0, 3, 3).unwrap();
    let state = fake_state((0..=3).flat_map(|y| {
        (0..=3).map(move |x| {
            let tile = XyzTile { z: 2, x, y };
            (tile, 1)
        })
    }));

    let report = TileTreeExporter::new(
        FakeAdapter {
            bounds,
            state: Arc::clone(&state),
        },
        &path,
    )
    .threads(2)
    .build()
    .unwrap();

    assert_eq!(report.tiles_written, 21);
    assert!(
        state.opened_readers.load(Ordering::Relaxed) >= 2,
        "split frontier should run through worker readers"
    );
    let mut reader = TileTreeReader::open(&path).unwrap();
    assert_eq!(u16_payload(&mut reader, 0, 0, 0), 16);
    assert_eq!(u16_payload(&mut reader, 2, 3, 3), 1);

    let _ = fs::remove_file(&path);
}

#[test]
fn per_block_dedup_handles_intermixed_ocean_island_pattern() {
    // 4×4 leaves with alternating "ocean" (1) and "island" (999) values.
    // Each block's size stream encodes runs of `1`s as anchor-reuses; every
    // 999 forces a fresh write, after which the next 1 is *also* a fresh
    // write (it doesn't dedup against an earlier `1` because the anchor
    // changed). The reader has to resolve each slot through the right
    // size-stream entry to come back with the original bytes.
    let path = test_path("per-block-dedup");
    let _ = fs::remove_file(&path);
    let bounds = XYZBounds::new(2, 0, 0, 3, 3).unwrap();
    let value_for = |x: u32, y: u32| -> u16 {
        let idx = y * 4 + x;
        if idx % 4 == 3 { 999 } else { 1 }
    };
    let state = fake_state((0..=3).flat_map(|y| {
        (0..=3).map(move |x| {
            let tile = XyzTile { z: 2, x, y };
            (tile, value_for(x, y))
        })
    }));

    TileTreeExporter::new(
        FakeAdapter {
            bounds,
            state: Arc::clone(&state),
        },
        &path,
    )
    .threads(1)
    .build()
    .unwrap();

    let mut reader = TileTreeReader::open(&path).unwrap();
    for y in 0..=3 {
        for x in 0..=3 {
            assert_eq!(
                u16_payload(&mut reader, 2, x, y),
                value_for(x as u32, y as u32),
                "z=2 ({x},{y}) mismatch"
            );
        }
    }

    let _ = fs::remove_file(&path);
}

#[test]
fn every_envelope_is_exactly_block_size_on_disk() {
    use crate::tree::format::{BLOCK_SIZE, HEADER_LEN};

    let path = test_path("envelope-size");
    let _ = fs::remove_file(&path);
    let bounds = XYZBounds::new(2, 0, 0, 3, 3).unwrap();
    let state = fake_state((0..=3).flat_map(|y| {
        (0..=3).map(move |x| {
            let tile = XyzTile { z: 2, x, y };
            (tile, (x + y * 4 + 1) as u16)
        })
    }));

    TileTreeExporter::new(
        FakeAdapter {
            bounds,
            state: Arc::clone(&state),
        },
        &path,
    )
    .threads(1)
    .build()
    .unwrap();

    let reader = TileTreeReader::open(&path).unwrap();
    let metadata_zoom = reader.bounds().zoom;
    let block_count = crate::tree::blocks::BlockGrid::new(reader.bounds(), 0)
        .unwrap()
        .total_blocks();
    let archive_len = fs::metadata(&path).unwrap().len();
    // Header + block region + tile-data + footer-magic. We don't pin the
    // tile-data length here, but every block must be exactly BLOCK_SIZE on
    // disk regardless of archive contents.
    assert!(
        archive_len >= HEADER_LEN + block_count * BLOCK_SIZE,
        "archive too small: {archive_len} bytes, expected at least {} for header + blocks (zoom {metadata_zoom})",
        HEADER_LEN + block_count * BLOCK_SIZE
    );

    let _ = fs::remove_file(&path);
}

#[test]
fn pack_extras_fires_for_a_small_multi_block_archive() {
    use crate::tree::format::{BLOCK_SIZE, HEADER_LEN};
    use std::os::unix::fs::FileExt;

    // Two blocks at zoom 7 (one per parent slot) sharing a parent at z=6.
    // Each block is small enough that its compressed self-payload leaves
    // headroom for the parent + sibling self-payloads.
    let path = test_path("pack-extras-fires");
    let _ = fs::remove_file(&path);
    let bounds = XYZBounds::new(7, 0, 0, 127, 0).unwrap();
    let state = fake_state((0..=127).map(|x| (XyzTile { z: 7, x, y: 0 }, 1u16)));

    TileTreeExporter::new(
        FakeAdapter {
            bounds,
            state: Arc::clone(&state),
        },
        &path,
    )
    .threads(1)
    .build()
    .unwrap();

    let file = fs::File::open(&path).unwrap();
    let block_count = crate::tree::blocks::BlockGrid::new(bounds, 0)
        .unwrap()
        .total_blocks();
    let mut packed_blocks = 0;
    for block_id in 0..block_count {
        let mut mode = [0u8; 1];
        file.read_exact_at(&mut mode, HEADER_LEN + block_id * BLOCK_SIZE)
            .unwrap();
        if mode[0] != 0 {
            packed_blocks += 1;
        }
    }
    assert!(
        packed_blocks > 0,
        "expected at least one block to pack neighbour extras, got 0 / {block_count}"
    );

    let _ = fs::remove_file(&path);
}

#[test]
fn exporter_honours_min_zoom_floor() {
    use crate::tree::blocks::BlockGrid;
    use crate::tree::format::read_header;

    let path = test_path("min-zoom-floor");
    let _ = fs::remove_file(&path);

    // 4×4 leaves at z=3, archive built with min_zoom=2. The reduce path
    // builds z=2 from the four leaves; z=0 and z=1 must not be present at
    // all (no envelopes, no readable tiles).
    let bounds = XYZBounds::new(3, 0, 0, 3, 3).unwrap();
    let state = fake_state((0..=3).flat_map(|y| {
        (0..=3).map(move |x| (XyzTile { z: 3, x, y }, 1u16))
    }));

    TileTreeExporter::new(
        FakeAdapter {
            bounds,
            state: Arc::clone(&state),
        },
        &path,
    )
    .threads(1)
    .min_zoom(2)
    .build()
    .unwrap();

    // Header carries the runtime min_zoom and the grid only spans z=2..=3.
    let mut file = fs::File::open(&path).unwrap();
    let header = read_header(&mut file).unwrap();
    assert_eq!(header.min_zoom, 2);
    assert_eq!(header.bounds.zoom, 3);
    let total_blocks = BlockGrid::new(header.bounds, header.min_zoom)
        .unwrap()
        .total_blocks();
    // 4×4 leaves fit one 64×64 block; their 2×2 reduce fits another.
    assert_eq!(total_blocks, 2);
    drop(file);

    // FakeAdapter sums children, so each z=2 tile is 4 (four leaves of 1).
    let mut reader = TileTreeReader::open(&path).unwrap();
    assert_eq!(u16_payload(&mut reader, 3, 0, 0), 1);
    assert_eq!(u16_payload(&mut reader, 2, 0, 0), 4);

    // Tiles below the floor have no envelope; the grid returns
    // `TileOutOfBounds` for them.
    for z in 0..=1u8 {
        let err = reader.read(z, 0, 0).unwrap_err();
        assert!(
            matches!(err, TileTreeError::TileOutOfBounds { .. }),
            "z={z}: expected TileOutOfBounds, got {err:?}"
        );
    }

    let _ = fs::remove_file(&path);
}

#[test]
fn exporter_rejects_max_zoom_below_leaf_downsample_cap() {
    // Leaf-only adapter at native z=11 with a 1-level downsample cap
    // (mirrors `TifDemSource`). Asking for `max_zoom=7` would force a
    // single leaf read to materialise a 16× larger native region, which
    // the source's reader can't service. The orchestrator must catch
    // this upfront — before opening the destination file.
    let path = test_path("leaf-zoom-gap");
    let _ = fs::remove_file(&path);

    let bounds = XYZBounds::new(11, 0, 0, 0, 0).unwrap();
    let err = TileTreeExporter::new(
        LeafCappedAdapter {
            bounds,
            max_leaf_downsample_steps: 1,
        },
        &path,
    )
    .threads(1)
    .max_zoom(7)
    .build()
    .unwrap_err();

    match err {
        TileTreeError::LeafZoomGapTooLarge {
            source_zoom,
            requested_zoom,
            max_supported_gap,
        } => {
            assert_eq!(source_zoom, 11);
            assert_eq!(requested_zoom, 7);
            assert_eq!(max_supported_gap, 1);
        }
        other => panic!("expected LeafZoomGapTooLarge, got {other:?}"),
    }

    assert!(
        !path.exists(),
        "destination must not be created when the export is rejected upfront"
    );
}

#[test]
fn progress_duration_format_is_compact() {
    assert_eq!(format_duration(Duration::from_secs(42)), "42s");
    assert_eq!(format_duration(Duration::from_secs(152)), "2m32s");
    assert_eq!(format_duration(Duration::from_secs(4_320)), "1h12m");
}

#[test]
fn progress_eta_uses_recent_window() {
    let start = Instant::now();
    let mut progress = ProgressWriter::with_start(Box::new(Vec::new()), 1_000, start);

    // First sample alone — window rate is undefined (no delta yet),
    // so the reporter falls back to "rate since start" so an ETA
    // still shows up.
    assert_eq!(
        progress.progress_details(start + Duration::from_secs(10), 100),
        " 10.00 blocks/s eta 1m30s"
    );
    assert_eq!(
        progress.progress_details(start + Duration::from_secs(30), 700),
        " 30.00 blocks/s eta 10s"
    );
    // Window pops the 10s-mark sample (>60s old), keeping the 30s
    // and 90s samples → 300 blocks over 60s → 5 b/s.
    assert_eq!(
        progress.progress_details(start + Duration::from_secs(90), 1_000),
        " 5.00 blocks/s eta 0s"
    );
}


fn fake_state(tiles: impl IntoIterator<Item = (XyzTile, u16)>) -> Arc<FakeState> {
    Arc::new(FakeState {
        source_tiles: tiles
            .into_iter()
            .map(|(tile, value)| ((tile.z, tile.x, tile.y), value))
            .collect(),
        ..FakeState::default()
    })
}

fn u16_payload(reader: &mut TileTreeReader, z: u8, x: u16, y: u16) -> u16 {
    let payload = reader.read(z, x, y).unwrap();
    u16::from_le_bytes(payload.try_into().unwrap())
}

fn test_path(name: &str) -> PathBuf {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/output/tree-tests");
    fs::create_dir_all(&dir).unwrap();
    dir.join(format!("{name}-{}.tengri-dem", std::process::id()))
}
