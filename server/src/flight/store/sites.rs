//! Nearest-site resolution: link a takeoff to a `sites` row at ingest, and
//! recompute those links in bulk for the admin reindex endpoint.

use sqlx::{PgPool, Postgres, Transaction};

/// A flight's takeoff must be within this many metres of a site to be linked to
/// it. Applied at ingest ([`find_nearest_site`]) and by the admin reindex
/// endpoint ([`reindex_takeoff_sites`]).
pub const TAKEOFF_SITE_RADIUS_M: f64 = 15_000.0;

/// The `sites` row nearest a takeoff, as resolved by [`find_nearest_site`].
pub(super) struct NearestSite {
    pub(super) id: i32,
    pub(super) distance_m: i32,
    pub(super) country: Option<String>,
}

/// Nearest `sites` row to a takeoff point within [`TAKEOFF_SITE_RADIUS_M`], or
/// `None` when nothing is in range. Coordinates are E5 micro-degrees, converted
/// to degrees at the bind site exactly like `INSERT_FLIGHT_SQL`.
pub(super) async fn find_nearest_site(
    tx: &mut Transaction<'_, Postgres>,
    takeoff_lat: i32,
    takeoff_lon: i32,
) -> Result<Option<NearestSite>, sqlx::Error> {
    let row: Option<(i32, i32, Option<String>)> = sqlx::query_as(
        "SELECT s.id, \
                ROUND(ST_Distance(ST_SetSRID(ST_MakePoint($1, $2), 4326)::geography, s.point))::int, \
                s.country \
         FROM sites s \
         WHERE ST_DWithin(ST_SetSRID(ST_MakePoint($1, $2), 4326)::geography, s.point, $3) \
         ORDER BY ST_SetSRID(ST_MakePoint($1, $2), 4326)::geography <-> s.point \
         LIMIT 1",
    )
    .bind(takeoff_lon as f64 / 1e5)
    .bind(takeoff_lat as f64 / 1e5)
    .bind(TAKEOFF_SITE_RADIUS_M)
    .fetch_optional(&mut **tx)
    .await?;
    Ok(row.map(|(id, distance_m, country)| NearestSite {
        id,
        distance_m,
        country,
    }))
}

/// Recompute every flight's nearest site (country, id, distance) within
/// [`TAKEOFF_SITE_RADIUS_M`], clearing the columns for flights with no site in
/// range. Returns the number of rows updated.
///
/// This stays set-based (no Rust-side point): the nearest-site `LEFT JOIN
/// LATERAL` runs over a fresh `flights f2` alias, not the UPDATE target —
/// Postgres forbids referencing the target table inside a `FROM` LATERAL
/// subquery.
pub async fn reindex_takeoff_sites(pool: &PgPool) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE flights f \
         SET closest_takeoff_id = n.site_id, \
             closest_takeoff_distance = n.dist, \
             takeoff_country = n.country \
         FROM ( \
             SELECT f2.id, near.site_id, near.dist, near.country \
             FROM flights f2 \
             LEFT JOIN LATERAL ( \
                 SELECT s.id AS site_id, \
                        ROUND(ST_Distance(f2.takeoff_point, s.point))::int AS dist, \
                        s.country \
                 FROM sites s \
                 WHERE ST_DWithin(f2.takeoff_point, s.point, $1) \
                 ORDER BY f2.takeoff_point <-> s.point \
                 LIMIT 1 \
             ) near ON TRUE \
             WHERE f2.takeoff_point IS NOT NULL \
         ) n \
         WHERE f.id = n.id",
    )
    .bind(TAKEOFF_SITE_RADIUS_M)
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}
