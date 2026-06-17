# Tiled GeoTIFF Reader Notes

This module is the only supported TIFF input path for DEM building. It reads
tiled, single-channel GeoTIFFs without loading the whole source image into RAM.

## Files

- `mod.rs`: module wiring and public re-exports.
- `reader.rs`: `TiledTifReader`, TIFF tag validation, chunk reads, and
  `read_region` orchestration.
- `types.rs`: `TiledTifInfo`, `TiledTifChunk`, and internal `PixelRegion`.
- `geo.rs`: GeoTIFF bounds derivation and geographic bounds to source pixel
  region conversion.
- `copy.rs`: copying decoded TIFF chunks into an exact region buffer and
  normalizing sample types into `i16`.
- `downscale.rs`: exact-region size validation and area-weighted downscale to a
  DEM-ready tile.

## Supported Sources

`TiledTifReader::open` accepts only:

- grayscale `16` or `32` bit TIFFs
- tiled TIFF layout (`TileWidth` and `TileLength`)
- signed integer or float sample format (`SampleFormat = 2` or `3`)
- GeoTIFF metadata with `ModelPixelScaleTag` and `ModelTiepointTag`
- EPSG:4326 (WGS84 lat/lon, default when no `GeoKeyDirectoryTag` is present)
  or EPSG:3857 (Web Mercator). Anything else fails open with
  `TiffReadError::UnsupportedProjection(epsg)`.

The TIFF's internal tile size is not required to be `256x256`; Copernicus COGs
use `1024x1024` internal tiles and edge tiles may be cut off.

## Bounds And Regions

`TiledTifInfo.bounds` is always lat/lon degrees — for Mercator sources the
metric corners are inverse-projected at open time so callers don't have to
know the source projection. `TiledTifInfo.origin_x` / `origin_y` /
`pixel_width` / `pixel_height` are in the source's *native* units (degrees
for WGS84, metres for Mercator); the projection branch of
`pixel_region_for_bounds` does the matching forward-projection.

`read_region(bounds)` converts geographic bounds to an exact source pixel
rectangle by rounding outward:

- west/east use `floor`/`ceil`
- north/south use `floor`/`ceil` against the north-up origin
- inputs first projected to native units when the source is Mercator
- snap-to-nearest within `1e-6` of a pixel before floor/ceil so an
  XYZ-tile edge that lands exactly on a Mercator pixel boundary doesn't
  acquire a 1-pixel overshoot from `asinh(tan(lat))` ULP error

Requests must already be valid and inside the source bounds. There is no silent
clamping in this layer. The exporter clips edge XYZ tile bounds to the source
or explicit export bounds before calling `read_region`.

## Read Flow

`read_region`:

1. Converts geographic bounds to `PixelRegion`.
2. Rejects regions bigger than `MAX_DEM_TILE_SIDE * 2` per side.
3. Reads every TIFF chunk intersecting that region.
4. Copies adjacent chunk slices into one exact row-major `i16` buffer.
5. Downscales only dimensions larger than `MAX_DEM_TILE_SIDE`.
6. Returns a `DemChunk` suitable for DEM compression.

Chunks are read once before copying. Do not re-read a source TIFF chunk per row.

## Normalization

The returned `DemChunk` always holds `Vec<i16>`.

In `copy.rs`:

- `i16` samples are copied as-is.
- `i32` samples are capped to `i16::MAX` and floored at `0`.
- `f32` samples are rounded, capped to `i16::MAX`, and non-finite/negative
  values become `0`.

In `downscale.rs` the area-average step also floors negative source values to
`0` before weighting (see `area_average`). So `i16` negatives only round-trip
unchanged when the region needs no downscale; otherwise they are clamped to
`0` along with any `i32`/`f32` negatives. The output of this module is
effectively a non-negative `i16` raster.

DEM quantization happens later in the DEM compression layer; this module only
turns source samples into a bounded `i16` raster.

## Downscaling

The exact source region can be up to twice the DEM tile side. If a side is
larger than `MAX_DEM_TILE_SIDE`, only that side is downscaled. The other side is
left unchanged.

The downscale is area-weighted and preserves outer source edges. Edge
preservation is important for adjacent XYZ tiles: rendered tile borders should
line up instead of drifting by a few pixels.
