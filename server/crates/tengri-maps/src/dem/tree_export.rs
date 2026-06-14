use std::io::Write;
use std::path::PathBuf;

use super::dem_export_adapter::DemExportAdapter;
use super::source::DemSource;
use crate::tree::{TileTreeError, TileTreeExportReport, TileTreeExporter};

pub struct DemTree;

pub struct DemTreeBuilder<S> {
    source: S,
    output: Option<PathBuf>,
    threads: usize,
    progress: Option<Box<dyn Write + Send>>,
}

pub type DemTreeExportReport = TileTreeExportReport;

impl DemTree {
    pub fn builder<S: DemSource + 'static>(source: S) -> DemTreeBuilder<S> {
        DemTreeBuilder::new(source)
    }
}

impl<S: DemSource + 'static> DemTreeBuilder<S> {
    pub fn new(source: S) -> Self {
        Self {
            source,
            output: None,
            threads: default_thread_count(),
            progress: None,
        }
    }

    pub fn output(mut self, output: impl Into<PathBuf>) -> Self {
        self.output = Some(output.into());
        self
    }

    pub fn threads(mut self, threads: usize) -> Self {
        self.threads = threads.max(1);
        self
    }

    pub fn progress(mut self, writer: impl Write + Send + 'static) -> Self {
        self.progress = Some(Box::new(writer));
        self
    }

    pub fn build(mut self) -> Result<DemTreeExportReport, TileTreeError> {
        let output = self
            .output
            .take()
            .ok_or(TileTreeError::MissingBuilderField("output"))?;
        let mut exporter = TileTreeExporter::new(
            DemExportAdapter {
                source: self.source,
            },
            output,
        )
        .threads(self.threads);
        if let Some(progress) = self.progress.take() {
            exporter = exporter.progress(progress);
        }
        exporter.build()
    }
}

fn default_thread_count() -> usize {
    std::thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use super::*;
    use crate::dem::DemChunk;
    use crate::dem::decompress::decompress_tile;
    use crate::dem::source::DemSourceReader;
    use crate::dem::tile_file::read_tile;
    use crate::geo::XyzTile;
    use crate::tree::{TileTreeReader, XYZBounds};

    struct FakeSource {
        bounds: XYZBounds,
    }

    struct FakeReader;

    struct MissingIntermediateSource {
        bounds: XYZBounds,
    }

    struct MissingIntermediateReader;

    impl DemSource for FakeSource {
        fn tile_bounds(&self) -> XYZBounds {
            self.bounds
        }

        fn open_reader(&self) -> Result<Box<dyn DemSourceReader>, TileTreeError> {
            Ok(Box::new(FakeReader))
        }
    }

    impl DemSourceReader for FakeReader {
        fn read(&mut self, tile: XyzTile) -> Result<DemChunk, TileTreeError> {
            let elevation = (tile.x + tile.y * 2 + 1) as i16 * 8;
            Ok(DemChunk::from_i16(1, 1, vec![elevation]))
        }
    }

    impl DemSource for MissingIntermediateSource {
        fn tile_bounds(&self) -> XYZBounds {
            self.bounds
        }

        fn open_reader(&self) -> Result<Box<dyn DemSourceReader>, TileTreeError> {
            Ok(Box::new(MissingIntermediateReader))
        }

        fn reads_intermediate_tiles(&self) -> bool {
            true
        }
    }

    impl DemSourceReader for MissingIntermediateReader {
        fn read(&mut self, tile: XyzTile) -> Result<DemChunk, TileTreeError> {
            if tile.z < 2 {
                return Err(TileTreeError::MissingTile {
                    z: tile.z,
                    x: tile.x as u16,
                    y: tile.y as u16,
                });
            }

            let elevation = (tile.x + tile.y * 2 + 1) as i16 * 8;
            Ok(DemChunk::from_i16(1, 1, vec![elevation]))
        }
    }

    #[test]
    fn builder_uses_source_abstraction_for_leaf_tiles() {
        let path = test_path("tengri-fake-source");
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(temp_path_for(&path));
        let bounds = XYZBounds::new(1, 0, 0, 1, 1).unwrap();

        let report = DemTree::builder(FakeSource { bounds })
            .output(&path)
            .threads(2)
            .build()
            .unwrap();

        assert_eq!(report.zoom, 1);
        assert_eq!(report.tiles_written, 5);

        let mut reader = TileTreeReader::open(&path).unwrap();
        let compressed = read_tile(reader.read(1, 1, 1).unwrap().as_slice()).unwrap();
        let tile = decompress_tile(&compressed).unwrap();
        assert_eq!(tile.width, 16);
        assert_eq!(tile.height, 16);
        assert!(tile.pixels.iter().all(|&elevation| elevation == 32));

        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(temp_path_for(&path));
    }

    #[test]
    fn missing_intermediate_source_tiles_fall_back_to_reduction() {
        let path = test_path("tengri-missing-intermediate");
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(temp_path_for(&path));
        let bounds = XYZBounds::new(2, 0, 0, 1, 1).unwrap();

        let report = DemTree::builder(MissingIntermediateSource { bounds })
            .output(&path)
            .threads(1)
            .build()
            .unwrap();

        assert_eq!(report.zoom, 2);
        assert_eq!(report.tiles_written, 6);

        let mut reader = TileTreeReader::open(&path).unwrap();
        let compressed = read_tile(reader.read(1, 0, 0).unwrap().as_slice()).unwrap();
        let tile = decompress_tile(&compressed).unwrap();
        assert_eq!(tile.width, 32);
        assert_eq!(tile.height, 32);
        assert!(tile.pixels.contains(&8));
        assert!(tile.pixels.contains(&32));

        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(temp_path_for(&path));
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
}
