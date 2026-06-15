use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::geo::XyzTile;
use crate::tree::{TileKind, TileTreeError, TileTreeReader, XYZBounds};

use super::adapter::{CachedChild, TileTreeExportAdapter};
use super::cache::RawTileCache;
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

    fn try_read_source_tile(
        &self,
        _reader: &mut Self::Reader,
        tile: XyzTile,
    ) -> Result<Option<Self::SourceTile>, TileTreeError> {
        self.state.read_tiles.lock().unwrap().push(tile);
        Ok(self.source_value(tile))
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

#[test]
fn exporter_writes_leaves_and_parents() {
    let path = test_path("generic-export");
    let _ = fs::remove_file(&path);
    let _ = fs::remove_file(temp_path_for(&path));
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
    let _ = fs::remove_file(temp_path_for(&path));
}

#[test]
fn source_intermediate_tile_writes_descendants_without_reducing_that_tile() {
    let path = test_path("source-intermediate");
    let _ = fs::remove_file(&path);
    let _ = fs::remove_file(temp_path_for(&path));
    let bounds = XYZBounds::new(2, 0, 0, 1, 1).unwrap();
    let state = fake_state([(XyzTile { z: 1, x: 0, y: 0 }, 99)].into_iter().chain(
        (0..=1).flat_map(|y| {
            (0..=1).map(move |x| {
                let tile = XyzTile { z: 2, x, y };
                (tile, (x + y * 2 + 1) as u16)
            })
        }),
    ));

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
    assert_eq!(u16_payload(&mut reader, 1, 0, 0), 99);
    assert_eq!(u16_payload(&mut reader, 2, 1, 1), 4);
    assert_eq!(u16_payload(&mut reader, 0, 0, 0), 99);
    let reduced = state.reduced_tiles.lock().unwrap();
    assert!(!reduced.contains(&XyzTile { z: 1, x: 0, y: 0 }));

    let _ = fs::remove_file(&path);
    let _ = fs::remove_file(temp_path_for(&path));
}

#[test]
fn missing_intermediate_source_tile_falls_back_to_child_reduction() {
    let path = test_path("missing-intermediate");
    let _ = fs::remove_file(&path);
    let _ = fs::remove_file(temp_path_for(&path));
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
    let _ = fs::remove_file(temp_path_for(&path));
}

#[test]
fn split_frontier_uses_worker_readers_and_completes_output() {
    let path = test_path("split-frontier");
    let _ = fs::remove_file(&path);
    let _ = fs::remove_file(temp_path_for(&path));
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
    let _ = fs::remove_file(temp_path_for(&path));
}

#[test]
fn cache_consume_removes_tile_file_immediately() {
    let path = test_path("cache-consume");
    let _ = fs::remove_file(&path);
    let bounds = XYZBounds::new(1, 0, 0, 0, 0).unwrap();
    let state = fake_state([]);
    let adapter = FakeAdapter { bounds, state };
    let cache = RawTileCache::new(&path).unwrap();
    let tile = XyzTile { z: 1, x: 0, y: 0 };
    cache.write(&adapter, tile, &42).unwrap();
    let tile_path = cache.path(tile);
    assert!(tile_path.exists());

    assert_eq!(cache.consume(&adapter, tile).unwrap(), Some(42));
    assert!(!tile_path.exists());
    assert_eq!(cache.consume(&adapter, tile).unwrap(), None);

    let _ = fs::remove_file(&path);
}

#[test]
fn progress_duration_format_is_compact() {
    assert_eq!(format_duration(Duration::from_secs(42)), "42s");
    assert_eq!(format_duration(Duration::from_secs(152)), "2m32s");
    assert_eq!(format_duration(Duration::from_secs(4_320)), "1h12m");
}

#[test]
fn progress_eta_uses_recent_window() {
    let mut progress = ProgressWriter::new(Box::new(Vec::new()), 1_000);
    let start = Instant::now();

    assert_eq!(progress.progress_details(start, 100), "");
    assert_eq!(
        progress.progress_details(start + Duration::from_secs(30), 700),
        " 20 tiles/s eta 15s"
    );
    assert_eq!(
        progress.progress_details(start + Duration::from_secs(90), 1_000),
        " 5 tiles/s eta 0s"
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
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/output/tree-tests");
    fs::create_dir_all(&dir).unwrap();
    dir.join(format!("{name}-{}.tengri-dem", std::process::id()))
}

fn temp_path_for(path: &Path) -> PathBuf {
    let mut file_name = path.file_name().unwrap().to_os_string();
    file_name.push(".tmp");
    path.with_file_name(file_name)
}
