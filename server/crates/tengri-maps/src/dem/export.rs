pub fn export_leaf_tiles(
    options: LeafTileExportOptions,
) -> Result<LeafTileExportReport, ExportError> {
    let reader = TiledTifReader::open(&options.source)?;
    let info = reader.info();
    let zoom = source_backed_leaf_zoom(info.pixel_width_degrees);
fn source_backed_leaf_zoom(pixel_width_degrees: f64) -> u8 {
    let source_tiles_across = ((360.0 / pixel_width_degrees) / 256.0).floor() as u32;
    if source_tiles_across == 0 {
        return 0;
    }

    u32::BITS as u8 - 1 - source_tiles_across.leading_zeros() as u8
}
