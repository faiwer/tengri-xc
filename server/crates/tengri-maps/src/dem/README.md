# DEM Tile Notes

This crate builds and serves DEM terrain tiles from tiled GeoTIFF and PMTiles
sources. The runtime artifact is a single indexed tile-tree container whose
DEM payloads are rendered as Terrarium PNGs at serve time.

## Source GeoTIFFs

Only tiled, single-channel grayscale GeoTIFFs are supported. The reader is
`tif::TiledTifReader`; the old whole-image TIFF reader was removed.

Accepted source samples: `i16`, `i32`, `f32`.

`DemChunk` always holds `Vec<i16>`. Sources convert at construction via
`DemChunk::from_i16` / `from_i32` / `from_f32`. Negative values and non-finite
floats become `0`; positive values above `i16::MAX` are capped.

Geo bounds come from GeoTIFF metadata:

- `ModelPixelScaleTag`
- `ModelTiepointTag`
- image width/height

The source does not have to cover the whole earth. One-degree Copernicus tiles
work: for example `Copernicus_DSM_COG_10_N47_00_E010_00_DEM.tif` covers roughly
`lon 10..11`, `lat 47..48`.

## Region Reads

`TiledTifReader::read_region(bounds)` reads the exact source pixels for a
geographic region, then returns a DEM-ready `DemChunk`.

Rules:

- Requested bounds must be finite, well-ordered, and inside the source bounds.
- No silent clamping for malformed/out-of-source requests.
- Edge XYZ tiles are clipped to the export/source bounds before calling
  `read_region`.
- The exact intermediate read is capped at `MAX_DEM_TILE_SIDE * 2` per side.
- The returned `DemChunk` is capped at `MAX_DEM_TILE_SIDE` per side.

If an exact source region is wider/taller than `MAX_DEM_TILE_SIDE`, only the
oversized dimension is downscaled. Downscaling is ordinary area-weighted
resampling across the full source region.

## Tile-Tree Export

The generic tile-tree exporter owns traversal, payload writes, progress, and
temporary raw-cache cleanup. DEM provides the adapter-specific pieces:

- read and resize a leaf `DemChunk`
- encode the final payload with `compress_tile`
- serialize private raw `i16` cache tiles
- build a parent `DemChunk` from raw child chunks

Export is post-order: child subtrees are written before each parent is
materialized. Only shallow subtree roots are spilled to the raw cache for upper
level reduction, so the exporter never writes the full raw leaf level. Clipped
missing child quadrants are filled with zero elevation before parent
downsampling.

## Compression

Shared constants live in `src/dem/constants.rs`:

- `MAX_DEM_TILE_SIDE = 256`
- `DEM_QUANTIZATION_METERS = 8`
- `MIN_DELTA_BITS = 2`
- `MAX_DELTA_BITS = 8`

`compress_tile` stores elevations in quantized units:

```text
stored = round(elevation_m / DEM_QUANTIZATION_METERS)
```

`decompress_tile` reconstructs stored units first, then multiplies by
`DEM_QUANTIZATION_METERS` for consumers. This is intentionally lossy. In the
Copernicus alpine test tile, 8 m quantization had a visible-size win with no
obvious map difference in the quick visual check.

The delta predictor walks row-major:

- The first pixel is stored as `start`.
- For other pixels, the reference is the reconstructed left neighbor.
- At the first column, the reference is the reconstructed upper neighbor.
- If the pixel is a `fix`, the real stored value is used and no delta is read.

`size_per_delta` is selected by a histogram estimate over widths `2..=8`.
Deltas that do not fit the chosen width become `Fix { idx, elevation }`.
`Fix.idx` is `u16`, relying on the `256x256` maximum tile size.

## Compressed Payload Format

Each DEM tile-tree entry stores this payload:

```text
start: i16
width: u16
height: u16
size_per_delta: u8
fix_count: u32
fixes: repeated { idx: u16, elevation: i16 }
delta_len: u32
deltas: zstd level 3 compressed packed-delta bytes
```

Fixed overhead is 15 bytes before fixes and delta bytes. `Box<[T]>` fields in
Rust are never written as pointers; `tile_file.rs` writes only stable contents.

Per-tile zstd is part of this payload. If container-level compression is added
later, change the payload reader and writer together.

## Serving

`serve_tiles.rs` reads payloads from the tile tree and dispatches by `TileKind`.
DEM tiles are decompressed, rendered as fixed `256x256` PNGs, and encoded with
Terrarium RGB. The stored tile may be rectangular or smaller than `256x256`;
PNG rendering resamples it to square at serve time.

The Terrarium encoding must keep fractional elevation when rendering. Earlier
grayscale/integer output looked wrong in MapLibre and caused contour/terrain
mismatches.

## Tile Index

The pyramid is complete and regular, so the index is a flat
`Box<[TileTreeIndexEntry]>` keyed by arithmetic on `(z, x, y)` rather than per-
entry `(z, x, y)` tuples. `WebMercatorTileBounds::slot(z, x, y)` walks the
levels (highest zoom first) and returns the slot in that array.

```text
TileTreeIndexEntry {
  offset: u64,
  length: u32,
}
```

## Open Decisions

- The final response format is not fixed: server-rendered PNGs, a
  PMTiles-compatible terrain source, or frontend-native DEM tiles are all still
  possible.
- The single-file container needs a stable binary spec before building large
  artifacts.
