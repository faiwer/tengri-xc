//! Glider/launch metadata: check a wing is visible to a user before ingest, and
//! overwrite the editable metadata on an existing flight.

use sqlx::PgPool;

/// Does a `models` row `(brand_id, kind, model_id)` visible to `user_id` exist?
/// "Visible" = canonical (`user_id IS NULL`) or owned by the caller. Ingest
/// paths (interactive upload, `tengri add`) check this before inserting a
/// flight so they can report *which* part of the wing triple was wrong instead
/// of a raw FK violation.
pub async fn model_exists(
    pool: &PgPool,
    user_id: i32,
    brand_id: &str,
    kind: &str,
    model_id: &str,
) -> Result<bool, sqlx::Error> {
    let exists: Option<bool> = sqlx::query_scalar(
        "SELECT TRUE FROM models \
         WHERE brand_id = $1 \
           AND kind     = $2::glider_kind \
           AND id       = $3 \
           AND (user_id IS NULL OR user_id = $4) \
         LIMIT 1",
    )
    .bind(brand_id)
    .bind(kind)
    .bind(model_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    Ok(exists.is_some())
}

/// The editable glider/launch metadata on a flight. `kind`, `propulsion`, and
/// `launch_method` are bound as text and cast at the SQL layer, matching
/// `INSERT_FLIGHT_SQL`.
pub struct FlightMetaUpdate<'a> {
    pub kind: &'a str,
    pub brand_id: &'a str,
    pub model_id: &'a str,
    pub propulsion: &'a str,
    pub launch_method: &'a str,
}

/// Overwrite the editable glider/launch metadata on a flight. The caller is
/// responsible for the ownership check and for validating that `(brand_id,
/// kind, model_id)` is a glider visible to the editor (see [`model_exists`]).
pub async fn update_flight_meta(
    pool: &PgPool,
    flight_id: &str,
    meta: &FlightMetaUpdate<'_>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE flights SET \
            kind = $2::glider_kind, \
            brand_id = $3, \
            model_id = $4, \
            propulsion = $5::propulsion, \
            launch_method = $6::launch_method \
         WHERE id = $1",
    )
    .bind(flight_id)
    .bind(meta.kind)
    .bind(meta.brand_id)
    .bind(meta.model_id)
    .bind(meta.propulsion)
    .bind(meta.launch_method)
    .execute(pool)
    .await?;
    Ok(())
}
