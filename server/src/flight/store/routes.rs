//! The `routes` rows: persist scoring outcomes, keep each flight's `main_*`
//! summary in sync, and read routes back. Also owns the text ↔ enum mapping for
//! the `route_type` / `route_sub_type` Postgres enums.

use anyhow::Context;
use sqlx::{PgPool, Postgres, Transaction};

use crate::flight::{Route, RouteEvaluation, RouteSubType, RouteType, ScoringOutcome};

pub async fn fetch_scored_routes(pool: &PgPool, flight_id: &str) -> anyhow::Result<Vec<Route>> {
    let rows = sqlx::query_as::<_, StoredRouteRow>(
        "SELECT id, flight_id, type::text AS route_type, sub_type::text AS sub_type, \
                turnpoints::text AS turnpoints, leg_distances, distance, \
                score::float8 AS score, factor::float8 AS factor, optimal, closure::text AS closure, \
                scored_ms \
         FROM routes \
         WHERE flight_id = $1",
    )
    .bind(flight_id)
    .fetch_all(pool)
    .await
    .with_context(|| format!("fetching scored routes for flight {flight_id}"))?;

    rows.into_iter().map(StoredRouteRow::into_route).collect()
}

pub async fn upsert_scored_routes(
    tx: &mut Transaction<'_, Postgres>,
    flight_id: &str,
    evaluation: &RouteEvaluation,
) -> anyhow::Result<u64> {
    let mut saved = 0;
    for outcome in &evaluation.routes {
        if let ScoringOutcome::Answer(route) = outcome {
            upsert_scored_route(tx, flight_id, route).await?;
            saved += 1;
        }
    }
    if saved > 0 {
        update_flight_main_route(tx, flight_id).await?;
    }
    Ok(saved)
}

async fn update_flight_main_route(
    tx: &mut Transaction<'_, Postgres>,
    flight_id: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE flights f \
         SET main_route_id   = r.id, \
             main_route_type = r.type, \
             main_score      = r.score, \
             main_distance   = r.distance \
         FROM ( \
             SELECT id, type, score, distance \
             FROM routes \
             WHERE flight_id = $1 \
             ORDER BY score DESC \
             LIMIT 1 \
         ) r \
         WHERE f.id = $1",
    )
    .bind(flight_id)
    .execute(&mut **tx)
    .await
    .context("updating flight main route")?;
    Ok(())
}

pub async fn upsert_scored_route(
    tx: &mut Transaction<'_, Postgres>,
    flight_id: &str,
    route: &Route,
) -> anyhow::Result<()> {
    let turnpoints = serde_json::to_string(&route.turnpoints).context("serializing turnpoints")?;
    let closure = route
        .closure
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .context("serializing closure")?;
    let leg_distances = route
        .leg_distances
        .iter()
        .copied()
        .map(i32::try_from)
        .collect::<Result<Vec<_>, _>>()
        .context("converting leg distances to Postgres integer[]")?;
    let distance =
        i32::try_from(route.distance).context("converting distance to Postgres integer")?;
    let score = format!("{:.2}", route.score);
    let factor = format!("{:.1}", route.factor);

    sqlx::query(
        "INSERT INTO routes \
         (flight_id, type, sub_type, turnpoints, leg_distances, distance, score, factor, optimal, closure, scored_ms) \
         VALUES ($1, $2::route_type, $3::route_sub_type, $4::jsonb, $5, $6, $7::numeric, $8::numeric, $9, $10::jsonb, $11) \
         ON CONFLICT (flight_id, type, sub_type) DO UPDATE SET \
         turnpoints = EXCLUDED.turnpoints, \
         leg_distances = EXCLUDED.leg_distances, \
         distance = EXCLUDED.distance, \
         score = EXCLUDED.score, \
         factor = EXCLUDED.factor, \
         optimal = EXCLUDED.optimal, \
         closure = EXCLUDED.closure, \
         scored_ms = EXCLUDED.scored_ms",
    )
    .bind(flight_id)
    .bind(route_type_value(route.route_type))
    .bind(route_sub_type_value(route.sub_type))
    .bind(turnpoints)
    .bind(&leg_distances)
    .bind(distance)
    .bind(score)
    .bind(factor)
    .bind(route.optimal)
    .bind(closure)
    .bind(route.scored_ms as i32)
    .execute(&mut **tx)
    .await
    .with_context(|| {
        format!(
            "upserting {:?}/{:?} route for flight {flight_id}",
            route.route_type, route.sub_type
        )
    })?;
    Ok(())
}

#[derive(sqlx::FromRow)]
struct StoredRouteRow {
    id: i64,
    flight_id: String,
    route_type: String,
    sub_type: String,
    turnpoints: String,
    leg_distances: Vec<i32>,
    distance: i32,
    score: f64,
    factor: f64,
    optimal: bool,
    closure: Option<String>,
    scored_ms: i32,
}

impl StoredRouteRow {
    fn into_route(self) -> anyhow::Result<Route> {
        Ok(Route {
            id: self.id,
            flight_id: self.flight_id,
            route_type: route_type_from_value(&self.route_type)?,
            sub_type: route_sub_type_from_value(&self.sub_type)?,
            turnpoints: serde_json::from_str(&self.turnpoints)
                .context("parsing route turnpoints")?,
            leg_distances: self
                .leg_distances
                .into_iter()
                .map(u32::try_from)
                .collect::<Result<Vec<_>, _>>()
                .context("converting stored leg distances")?,
            distance: u32::try_from(self.distance).context("converting stored route distance")?,
            score: self.score,
            factor: self.factor,
            optimal: self.optimal,
            closure: self
                .closure
                .map(|closure| serde_json::from_str(&closure))
                .transpose()
                .context("parsing route closure")?,
            scored_ms: self.scored_ms as u32,
        })
    }
}

fn route_type_value(route_type: RouteType) -> &'static str {
    match route_type {
        RouteType::FreeDistance => "free_distance",
        RouteType::FaiTriangle => "fai_triangle",
        RouteType::FreeTriangle => "free_triangle",
        RouteType::Task => "task",
    }
}

fn route_type_from_value(value: &str) -> anyhow::Result<RouteType> {
    match value {
        "free_distance" => Ok(RouteType::FreeDistance),
        "fai_triangle" => Ok(RouteType::FaiTriangle),
        "free_triangle" => Ok(RouteType::FreeTriangle),
        "task" => Ok(RouteType::Task),
        _ => Err(anyhow::anyhow!("unknown route_type {value:?}")),
    }
}

fn route_sub_type_value(sub_type: RouteSubType) -> &'static str {
    match sub_type {
        RouteSubType::None => "none",
        RouteSubType::OlcClosed => "olc_closed",
        RouteSubType::OlcOpen => "olc_open",
        RouteSubType::FaiCylinders => "fai_cylinders",
    }
}

fn route_sub_type_from_value(value: &str) -> anyhow::Result<RouteSubType> {
    match value {
        "none" => Ok(RouteSubType::None),
        "olc_closed" => Ok(RouteSubType::OlcClosed),
        "olc_open" => Ok(RouteSubType::OlcOpen),
        "fai_cylinders" => Ok(RouteSubType::FaiCylinders),
        _ => Err(anyhow::anyhow!("unknown route_sub_type {value:?}")),
    }
}
