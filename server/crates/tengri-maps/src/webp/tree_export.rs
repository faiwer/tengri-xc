use std::io::Write;
use std::path::PathBuf;

use super::webp_export_adapter::WebpExportAdapter;
use crate::matrix::Raster;
use crate::tree::{
    PassthroughCodec, TileSource, TileTreeError, TileTreeExportReport, TileTreeExporter,
};

pub struct WebpTree;

pub struct WebpTreeBuilder<S> {
    source: S,
    output: Option<PathBuf>,
    threads: usize,
    min_zoom: u8,
    max_zoom: Option<u8>,
    quality: u8,
    passthrough: bool,
    progress: Option<Box<dyn Write + Send>>,
}

pub type WebpTreeExportReport = TileTreeExportReport;

impl WebpTree {
    pub fn builder<S: TileSource<Tile = Raster> + 'static>(source: S) -> WebpTreeBuilder<S> {
        WebpTreeBuilder::new(source)
    }
}

impl<S: TileSource<Tile = Raster> + 'static> WebpTreeBuilder<S> {
    pub fn new(source: S) -> Self {
        Self {
            source,
            output: None,
            threads: default_thread_count(),
            min_zoom: 0,
            max_zoom: None,
            quality: 75,
            passthrough: false,
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

    /// libwebp lossy quality, clamped to 0..=100 at build time.
    pub fn quality(mut self, quality: u8) -> Self {
        self.quality = quality.min(100);
        self
    }

    /// When `true`, source tiles that arrive as already-WebP at the archive's
    /// tile dimensions are copied verbatim into the archive (no decode +
    /// re-encode, no generation loss).
    pub fn passthrough(mut self, passthrough: bool) -> Self {
        self.passthrough = passthrough;
        self
    }

    pub fn progress(mut self, writer: impl Write + Send + 'static) -> Self {
        self.progress = Some(Box::new(writer));
        self
    }

    pub fn build(mut self) -> Result<WebpTreeExportReport, TileTreeError> {
        let output = self
            .output
            .take()
            .ok_or(TileTreeError::MissingBuilderField("output"))?;
        // Cache the source's passthrough codec once: the adapter consults it
        // per-tile and we want to avoid the virtual call on the hot path.
        // `None` here is fine — `read_source_tile` short-circuits.
        let source_passthrough_codec = match self.source.raw_codec() {
            Some(PassthroughCodec::Webp) => Some(PassthroughCodec::Webp),
            // No other codec matches our target; treat as "no fast-path".
            _ => None,
        };
        let mut exporter = TileTreeExporter::new(
            WebpExportAdapter {
                source: self.source,
                quality: self.quality,
                passthrough: self.passthrough,
                source_passthrough_codec,
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
