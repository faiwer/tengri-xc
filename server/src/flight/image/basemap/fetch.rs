//! Pulling a plan's tiles, all or nothing.

use std::{sync::OnceLock, time::Duration};

use tengri_geo::XyzTile;
use tiny_skia::Pixmap;
use tokio::task::JoinSet;

use super::{
    decode::decode_jpeg,
    mosaic::{Basemap, stitch},
    tiles,
};
use crate::{config::SatelliteMap, flight::image::layout::Layout};

/// What the whole fetch gets before the render goes ahead on white.
const BUDGET: Duration = Duration::from_secs(3);

/// Requests in flight at once.
const CHUNK: usize = 4;

/// The backdrop for `layout`, or `None` if anything at all went wrong — a
/// missing tile would leave a white hole, which reads worse than no imagery.
pub(in crate::flight::image) async fn fetch(
    satellite: &SatelliteMap,
    layout: &Layout,
) -> Option<Basemap> {
    let plan = tiles::plan(layout, satellite.tile_size)?;
    let tiles = tokio::time::timeout(BUDGET, fetch_tiles(&satellite.url, &plan.tiles))
        .await
        .ok()??;
    stitch(&plan, &tiles, satellite.attribution.as_deref())
}

async fn fetch_tiles(template: &str, tiles: &[XyzTile]) -> Option<Vec<(XyzTile, Pixmap)>> {
    let mut fetched = Vec::with_capacity(tiles.len());
    for chunk in tiles.chunks(CHUNK) {
        let mut pending = JoinSet::new();
        for &tile in chunk {
            let url = tile_url(template, tile);
            pending.spawn(async move { fetch_tile(url).await.map(|image| (tile, image)) });
        }
        // Dropping the set on the way out aborts whatever is still in flight.
        while let Some(joined) = pending.join_next().await {
            fetched.push(joined.ok()??);
        }
    }
    Some(fetched)
}

async fn fetch_tile(url: String) -> Option<Pixmap> {
    let response = client().get(url).send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }
    decode_jpeg(&response.bytes().await.ok()?)
}

/// ArcGIS writes the path as `{z}/{y}/{x}` — row before column — so the
/// substitution goes by name, not by position.
fn tile_url(template: &str, tile: XyzTile) -> String {
    template
        .replace("{z}", &tile.z.to_string())
        .replace("{x}", &tile.x.to_string())
        .replace("{y}", &tile.y.to_string())
}

/// Shared so the handful of tiles per render reuse one connection pool.
fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent("tengri-xc")
            .timeout(BUDGET)
            .build()
            .unwrap_or_default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_url_names_the_row_before_the_column() {
        let url = tile_url(
            "https://example.test/{z}/{y}/{x}",
            XyzTile {
                z: 11,
                x: 1058,
                y: 734,
            },
        );

        assert_eq!(url, "https://example.test/11/734/1058");
    }
}
