use std::io::{Read, Write};

use super::decode::decode_webp_bytes;
use super::encode::encode_lossy;
use super::peek::peek_webp_header;
use super::pyramid::build_parent_raster;
use crate::dem::constants::MAX_DEM_TILE_SIDE;
use crate::geo::XyzTile;
use crate::matrix::Raster;
use crate::tree::{
    CachedChild, PassthroughCodec, TileKind, TilePayload, TileSource, TileSourceReader,
    TileTreeError, TileTreeExportAdapter, XYZBounds,
};

const PASSTHROUGH_FLAG: u8 = 0x80;

pub(super) struct WebpExportAdapter<S> {
    pub(super) source: S,
    pub(super) quality: u8,
    pub(super) passthrough: bool,
    /// Cached at construction so `read_source_tile` can short-circuit the
    /// passthrough check without a virtual call per tile. `Some` only when the
    /// source's `raw_codec()` matches our target codec (WebP).
    pub(super) source_passthrough_codec: Option<PassthroughCodec>,
}

impl<S: TileSource<Tile = Raster> + 'static> TileTreeExportAdapter for WebpExportAdapter<S> {
    type SourceTile = TilePayload<Raster>;
    type Reader = Box<dyn TileSourceReader<Tile = Raster>>;

    fn tile_kind(&self) -> TileKind {
        TileKind::Webp
    }

    fn bounds(&self) -> XYZBounds {
        self.source.tile_bounds()
    }

    fn open_reader(&self) -> Result<Self::Reader, TileTreeError> {
        self.source.open_reader()
    }

    fn supplies_all_zooms(&self) -> bool {
        self.source.reads_intermediate_tiles()
    }

    fn max_leaf_downsample_steps(&self) -> u8 {
        self.source.max_leaf_downsample_steps()
    }

    fn kind_config(&self) -> u8 {
        // Low 7 bits: configured quality (range 0..=100 enforced at CLI parse /
        // builder). High bit: passthrough flag. Records the operator's intent
        // regardless of whether passthrough actually fired tile-for-tile.
        let q = self.quality.min(100);
        if self.passthrough { q | PASSTHROUGH_FLAG } else { q }
    }

    fn read_source_tile(
        &self,
        reader: &mut Self::Reader,
        tile: XyzTile,
    ) -> Result<Self::SourceTile, TileTreeError> {
        // Passthrough fast-path requirements:
        //   1. operator opted in (`--passthrough`),
        //   2. source advertises WebP raw bytes,
        //   3. dim already matches archive tile-side (no resample needed),
        //   4. source is RGB — RGBA bytes would carry alpha into the
        //      archive, but our pipeline is uniformly RGB (alpha is dead
        //      weight for satellite imagery and is stripped on decode).
        // Anything else falls through to the matrix path, where decode
        // produces 3-channel and the encoder writes RGB.
        if self.passthrough && self.source_passthrough_codec == Some(PassthroughCodec::Webp) {
            if let Some(bytes) = reader.read_raw(tile)? {
                let header = peek_webp_header(&bytes)?;
                if !header.has_alpha
                    && header.width == u32::from(MAX_DEM_TILE_SIDE)
                    && header.height == u32::from(MAX_DEM_TILE_SIDE)
                {
                    return Ok(TilePayload::Passthrough(bytes));
                }
            }
        }
        Ok(TilePayload::Matrix(reader.read(tile)?))
    }

    fn encode_payload(&self, tile: &Self::SourceTile) -> Result<Vec<u8>, TileTreeError> {
        match tile {
            TilePayload::Passthrough(bytes) => Ok(bytes.clone()),
            TilePayload::Matrix(raster) => encode_lossy(raster, self.quality),
        }
    }

    fn write_raw_cache(
        &self,
        writer: &mut dyn Write,
        tile: &Self::SourceTile,
    ) -> Result<(), TileTreeError> {
        let raster = match tile {
            TilePayload::Matrix(raster) => raster.clone(),
            TilePayload::Passthrough(bytes) => decode_webp_bytes(bytes)?,
        };
        writer.write_all(&raster.width.to_le_bytes())?;
        writer.write_all(&raster.height.to_le_bytes())?;
        writer.write_all(&[raster.channels])?;
        writer.write_all(&raster.pixels)?;
        Ok(())
    }

    fn read_raw_cache(&self, reader: &mut dyn Read) -> Result<Self::SourceTile, TileTreeError> {
        let width = read_u16(reader)?;
        let height = read_u16(reader)?;
        let mut channels_buf = [0u8; 1];
        reader.read_exact(&mut channels_buf)?;
        let channels = channels_buf[0];
        if channels != 3 {
            return Err(TileTreeError::CorruptFile(
                "WebP raw cache: channels must be 3 (RGB); alpha is dropped at decode",
            ));
        }
        let len = usize::from(width) * usize::from(height) * usize::from(channels);
        let mut pixels = vec![0u8; len];
        reader.read_exact(&mut pixels)?;
        Ok(TilePayload::Matrix(Raster {
            width,
            height,
            channels,
            pixels,
        }))
    }

    fn reduce_children_to_tile(
        &self,
        tile: XyzTile,
        children: &[CachedChild<Self::SourceTile>],
    ) -> Result<Self::SourceTile, TileTreeError> {
        let mut raster_children: Vec<CachedChild<Raster>> = Vec::with_capacity(children.len());
        for child in children {
            let raster = match &child.raw {
                TilePayload::Matrix(raster) => raster.clone(),
                TilePayload::Passthrough(bytes) => decode_webp_bytes(bytes)?,
            };
            raster_children.push(CachedChild {
                tile: child.tile,
                raw: raster,
            });
        }
        let parent = build_parent_raster(tile, &raster_children)?;
        Ok(TilePayload::Matrix(parent))
    }
}

fn read_u16(reader: &mut dyn Read) -> Result<u16, TileTreeError> {
    let mut bytes = [0; 2];
    reader.read_exact(&mut bytes)?;
    Ok(u16::from_le_bytes(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geo::XyzTile;
    use crate::tree::XYZBounds;

    /// Minimal `TileSource` stand-in. Read paths are unreachable for
    /// tests that only exercise the export adapter's encode/cache
    /// surface; tests that exercise `read_source_tile` plug a custom
    /// reader directly via [`adapter`] + a hand-rolled
    /// `TileSourceReader`.
    struct StubSource {
        codec: Option<PassthroughCodec>,
    }

    impl TileSource for StubSource {
        type Tile = Raster;

        fn tile_bounds(&self) -> XYZBounds {
            XYZBounds::new(0, 0, 0, 0, 0).unwrap()
        }
        fn open_reader(
            &self,
        ) -> Result<Box<dyn TileSourceReader<Tile = Raster>>, TileTreeError> {
            unreachable!("encode-side tests never open a reader")
        }
        fn raw_codec(&self) -> Option<PassthroughCodec> {
            self.codec
        }
    }

    fn adapter(quality: u8, passthrough: bool) -> WebpExportAdapter<StubSource> {
        WebpExportAdapter {
            source: StubSource { codec: Some(PassthroughCodec::Webp) },
            quality,
            passthrough,
            source_passthrough_codec: Some(PassthroughCodec::Webp),
        }
    }

    #[test]
    fn kind_config_packs_passthrough_flag_with_quality() {
        assert_eq!(adapter(75, false).kind_config(), 75);
        assert_eq!(adapter(75, true).kind_config(), 0x80 | 75);
        assert_eq!(adapter(0, true).kind_config(), 0x80);
        assert_eq!(adapter(100, false).kind_config(), 100);
    }

    #[test]
    fn quality_above_100_is_clamped_in_kind_config() {
        // `kind_config()` defends against an out-of-range build — the
        // header byte must never collide with the passthrough flag bit.
        assert_eq!(adapter(200, false).kind_config(), 100);
        assert_eq!(adapter(200, true).kind_config(), 0x80 | 100);
    }

    #[test]
    fn passthrough_returns_source_bytes_verbatim() {
        // Encode a real WebP, hand it to the adapter as a `Passthrough`
        // payload — `encode_payload` must copy the bytes byte-for-byte.
        let raster = Raster {
            width: 8,
            height: 8,
            channels: 3,
            pixels: (0..(8 * 8 * 3)).map(|v| (v * 3) as u8).collect(),
        };
        let source_bytes = encode_lossy(&raster, 75).unwrap();

        let payload = adapter(75, true)
            .encode_payload(&TilePayload::Passthrough(source_bytes.clone()))
            .unwrap();

        assert_eq!(payload, source_bytes);
    }

    #[test]
    fn passthrough_off_skips_read_raw_entirely() {
        // With `passthrough = false`, `read_source_tile` must never
        // consult `read_raw`. Use a stub reader whose `read_raw` panics
        // to make a violation loud.
        struct StubReader;
        impl TileSourceReader for StubReader {
            type Tile = Raster;
            fn read(&mut self, _tile: XyzTile) -> Result<Raster, TileTreeError> {
                Ok(Raster {
                    width: 1,
                    height: 1,
                    channels: 3,
                    pixels: vec![0, 0, 0],
                })
            }
            fn read_raw(&mut self, _tile: XyzTile) -> Result<Option<Vec<u8>>, TileTreeError> {
                panic!("read_raw must not be called when passthrough is off");
            }
        }
        let mut reader: Box<dyn TileSourceReader<Tile = Raster>> = Box::new(StubReader);
        let payload = adapter(60, false)
            .read_source_tile(&mut reader, XyzTile { z: 0, x: 0, y: 0 })
            .unwrap();
        assert!(matches!(payload, TilePayload::Matrix(_)));
    }

    #[test]
    fn read_source_tile_falls_back_to_matrix_when_dims_mismatch() {
        // Source advertises WebP passthrough and `read_raw` returns
        // 512×512 bytes; the archive's tile-side is 256 — so the
        // adapter must reject the fast-path and take the matrix path.
        struct OversizeReader;
        impl TileSourceReader for OversizeReader {
            type Tile = Raster;
            fn read(&mut self, _tile: XyzTile) -> Result<Raster, TileTreeError> {
                Ok(Raster {
                    width: 256,
                    height: 256,
                    channels: 3,
                    pixels: vec![42; 256 * 256 * 3],
                })
            }
            fn read_raw(&mut self, _tile: XyzTile) -> Result<Option<Vec<u8>>, TileTreeError> {
                let big = Raster {
                    width: 512,
                    height: 512,
                    channels: 3,
                    pixels: vec![0; 512 * 512 * 3],
                };
                Ok(Some(encode_lossy(&big, 75)?))
            }
        }
        let mut reader: Box<dyn TileSourceReader<Tile = Raster>> = Box::new(OversizeReader);
        let payload = adapter(75, true)
            .read_source_tile(&mut reader, XyzTile { z: 0, x: 0, y: 0 })
            .unwrap();
        match payload {
            TilePayload::Matrix(r) => {
                assert_eq!(r.width, 256);
                assert_eq!(r.height, 256);
            }
            TilePayload::Passthrough(_) => panic!("dim mismatch must take matrix path"),
        }
    }

    #[test]
    fn raw_cache_round_trips_pixels_shape() {
        // Cache only ever sees 3-channel rasters in the pipeline (decode
        // strips alpha, passthrough is rejected for RGBA sources).
        let raster = Raster {
            width: 4,
            height: 3,
            channels: 3,
            pixels: (0..(4 * 3 * 3)).map(|v| (v * 7) as u8).collect(),
        };
        let original = TilePayload::Matrix(raster.clone());

        let mut buffer = Vec::new();
        adapter(75, false)
            .write_raw_cache(&mut buffer, &original)
            .unwrap();
        let restored = adapter(75, false)
            .read_raw_cache(&mut buffer.as_slice())
            .unwrap();

        match restored {
            TilePayload::Matrix(restored) => {
                assert_eq!(restored.width, raster.width);
                assert_eq!(restored.height, raster.height);
                assert_eq!(restored.channels, raster.channels);
                assert_eq!(restored.pixels, raster.pixels);
            }
            TilePayload::Passthrough(_) => {
                panic!("raw cache round-trip must always restore Matrix shape")
            }
        }
    }

    #[test]
    fn read_source_tile_rejects_passthrough_when_source_has_alpha() {
        // Source ships an RGBA WebP at the right dim; passthrough must
        // refuse it and fall back to the matrix path so the matrix-side
        // decode can strip alpha. Otherwise alpha would leak into the
        // archive verbatim.
        struct RgbaReader;
        impl TileSourceReader for RgbaReader {
            type Tile = Raster;
            fn read(&mut self, _tile: XyzTile) -> Result<Raster, TileTreeError> {
                Ok(Raster {
                    width: u16::from(MAX_DEM_TILE_SIDE),
                    height: u16::from(MAX_DEM_TILE_SIDE),
                    channels: 3,
                    pixels: vec![
                        7;
                        usize::from(MAX_DEM_TILE_SIDE) * usize::from(MAX_DEM_TILE_SIDE) * 3
                    ],
                })
            }
            fn read_raw(&mut self, _tile: XyzTile) -> Result<Option<Vec<u8>>, TileTreeError> {
                let rgba = Raster {
                    width: u16::from(MAX_DEM_TILE_SIDE),
                    height: u16::from(MAX_DEM_TILE_SIDE),
                    channels: 4,
                    pixels: vec![
                        128;
                        usize::from(MAX_DEM_TILE_SIDE) * usize::from(MAX_DEM_TILE_SIDE) * 4
                    ],
                };
                let q = 75.0_f32;
                let memory = ::webp::Encoder::from_rgba(
                    &rgba.pixels,
                    u32::from(rgba.width),
                    u32::from(rgba.height),
                )
                .encode(q);
                Ok(Some(memory.to_vec()))
            }
        }
        let mut reader: Box<dyn TileSourceReader<Tile = Raster>> = Box::new(RgbaReader);
        let payload = adapter(75, true)
            .read_source_tile(&mut reader, XyzTile { z: 0, x: 0, y: 0 })
            .unwrap();
        assert!(
            matches!(payload, TilePayload::Matrix(_)),
            "RGBA source must take matrix path, not passthrough"
        );
    }
}
