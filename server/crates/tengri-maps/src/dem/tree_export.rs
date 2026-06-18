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
    min_zoom: u8,
    max_zoom: Option<u8>,
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
            min_zoom: 0,
            max_zoom: None,
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

    pub fn min_zoom(mut self, min_zoom: u8) -> Self {
        self.min_zoom = min_zoom;
        self
    }

    pub fn max_zoom(mut self, max_zoom: u8) -> Self {
        self.max_zoom = Some(max_zoom);
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
        .threads(self.threads)
        .min_zoom(self.min_zoom);
        if let Some(max_zoom) = self.max_zoom {
            exporter = exporter.max_zoom(max_zoom);
        }
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
        assert_eq!(tile.width, 1);
        assert_eq!(tile.height, 1);
        assert!(tile.pixels.iter().all(|&elevation| elevation == 32));

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
