
use super::compress::compress_tile;
use super::progress::ProgressWriter;
use super::pyramid::write_parent_levels;
use super::resolution::{resize_dem_matrix, target_dem_resolution};
use super::source::DemSource;
use super::tile_file::write_tile;
pub struct DemTree;

pub struct DemTreeBuilder<S> {
    source: S,
    output: Option<PathBuf>,
    threads: usize,
    progress: Option<Box<dyn Write + Send>>,
    log: Option<Box<dyn Write + Send>>,
}

pub struct DemTreeExportReport {
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
            log: None,
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

    pub fn build(mut self) -> Result<DemTreeExportReport, DemTreeError> {
        let output = self
            .output
            .take()
            .ok_or(DemTreeError::MissingBuilderField("output"))?;
        let tree_bounds = self.source.tile_bounds();
        let tiles = tree_bounds.tiles_at(tree_bounds.zoom)?;
        let total = usize::try_from(tree_bounds.total_index_entries()?)
            .map_err(|_| DemTreeError::InvalidBounds("index is too large"))?;
        let mut progress = self.progress.take().map(ProgressWriter::new);

        log(
            &mut self.log,
            format_args!(
                "exporting {total} DEM tree tiles at z={} with {worker_count} workers",
                tree_bounds.zoom
            ),
        );

        let started = Instant::now();
        let mut tree = DemTreeFile::create(output)
            .tile_kind(TileKind::Dem)
            .bounds(tree_bounds)
            .finish_header()?;
fn write_source_tiles<S: DemSource + 'static>(
    tree: &mut DemTreeFile,
    source: S,
    tiles: Vec<XyzTile>,
    worker_count: usize,
    progress: &mut Option<ProgressWriter>,
    total: usize,
) -> Result<usize, DemTreeError> {
    let tiles = Arc::new(tiles);
    let source = Arc::new(source);
    let next = Arc::new(AtomicUsize::new(0));
    let cancel = Arc::new(AtomicBool::new(false));
    let (send, receive) = mpsc::channel();
    let mut workers = Vec::with_capacity(worker_count);

    for _ in 0..worker_count {
        let source = Arc::clone(&source);
        let tiles = Arc::clone(&tiles);
        let next = Arc::clone(&next);
        let cancel = Arc::clone(&cancel);
        let send = send.clone();
        workers.push(thread::spawn(move || {
            source_worker(source, tiles, next, cancel, send)
        }));
    }
    drop(send);

    let mut done = 0;
    for _ in 0..tiles.len() {
        match receive
            .recv()
            .map_err(|_| DemTreeError::CorruptFile("leaf workers stopped"))?
        {
            Ok(result) => {
                tree.add(
                    result.tile.z,
                    result.tile.x as u16,
                    result.tile.y as u16,
                    &result.payload,
                )?;
                done += 1;
                if let Some(progress) = progress.as_mut() {
                    progress.update(done, total);
                }
            }
            Err(error) => {
                cancel.store(true, Ordering::Relaxed);
                join_workers(workers)?;
                return Err(error);
            }
        }
    }

    join_workers(workers)?;
    Ok(done)
}
