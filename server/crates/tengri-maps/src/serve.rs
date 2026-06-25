use crate::geo::XyzTile;
use crate::tree::TileKind;

pub type TileServeError = Box<dyn std::error::Error + 'static>;

pub struct ServedTile {
    pub content_type: &'static str,
    pub body: Vec<u8>,
}

pub trait TileServeFormat: Send + Sync {
    /// E.g. `/dem/` from '/dem/8/123/45.png'.
    fn route_prefix(&self) -> &'static str;
    /// E.g. `.png`.
    fn file_extension(&self) -> &'static str;
    /// Render the tile from the payload. Served to the client.
    fn render(&self, payload: &[u8]) -> Result<ServedTile, TileServeError>;
    /// E.g. `/dem/8/123/45.png` -> `XyzTile { z: 8, x: 123, y: 45 }`.
    fn parse_path(&self, path: &str) -> Option<XyzTile> {
        parse_tile_path(path, self.route_prefix(), self.file_extension())
    }
}

pub fn tile_serve_format(tile_kind: TileKind) -> Box<dyn TileServeFormat> {
    match tile_kind {
        TileKind::Dem => Box::new(crate::dem::serve::DemTerrariumServeFormat),
        TileKind::Webp => Box::new(crate::webp::serve::WebpServeFormat),
    }
}

fn parse_tile_path(path: &str, route_prefix: &str, file_extension: &str) -> Option<XyzTile> {
    let path = path.strip_prefix(route_prefix)?;
    let parts: Vec<_> = path.split('/').collect();
    if parts.len() != 3 {
        return None;
    }

    Some(XyzTile {
        z: parts[0].parse().ok()?,
        x: parts[1].parse().ok()?,
        y: parts[2].strip_suffix(file_extension)?.parse().ok()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct PngTiles;

    impl TileServeFormat for PngTiles {
        fn route_prefix(&self) -> &'static str {
            "/dem/"
        }

        fn file_extension(&self) -> &'static str {
            ".png"
        }

        fn render(&self, _payload: &[u8]) -> Result<ServedTile, TileServeError> {
            Ok(ServedTile {
                content_type: "image/png",
                body: Vec::new(),
            })
        }
    }

    #[test]
    fn parses_format_tile_path() {
        assert_eq!(
            PngTiles.parse_path("/dem/8/123/45.png"),
            Some(XyzTile {
                z: 8,
                x: 123,
                y: 45
            })
        );
    }

    #[test]
    fn rejects_wrong_prefix_or_extension() {
        assert_eq!(PngTiles.parse_path("/tiles/8/123/45.png"), None);
        assert_eq!(PngTiles.parse_path("/dem/8/123/45.webp"), None);
    }
}
