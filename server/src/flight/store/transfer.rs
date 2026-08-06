//! Flight ownership transfer. The `flights.user_id` swap is one line; the work
//! is carrying a pilot-*private* glider across with the flight so the new owner
//! can still see and edit it. A private model (and, when it too is private, its
//! brand) is re-keyed under the new owner's `<user_id>:<slug>` namespace:
//! copied when another flight still needs the old row, moved (old row reaped)
//! when this was the last reference. This mirrors the
//! `flights_cleanup_orphan_customs` trigger's model-then-brand orphan sweep.

use sqlx::{PgPool, Postgres, Transaction};

/// Result of a [`transfer_flight_owner`] attempt. The two miss cases are caller
/// input problems (unknown flight / unknown user), not internal errors.
pub enum TransferOutcome {
    Transferred,
    FlightNotFound,
    UnknownUser,
}

/// The flight's current owner and glider triple, read under a row lock.
struct FlightGlider {
    owner_id: i32,
    brand_id: String,
    kind: String,
    model_id: String,
}

/// A private model's copyable payload — everything but its id/ownership.
struct PrivateModel {
    name: String,
    class: String,
    is_tandem: bool,
}

/// Reassign `flight_id` to `new_user_id`, carrying a private glider along. Runs
/// in one transaction: lock the flight, swap the owner, then re-key the private
/// glider (if any) under the new owner and reap the old rows when unreferenced.
pub async fn transfer_flight_owner(
    pool: &PgPool,
    flight_id: &str,
    new_user_id: i32,
) -> Result<TransferOutcome, sqlx::Error> {
    let mut tx = pool.begin().await?;

    if !user_exists(&mut tx, new_user_id).await? {
        return Ok(TransferOutcome::UnknownUser);
    }
    let Some(glider) = lock_flight(&mut tx, flight_id).await? else {
        return Ok(TransferOutcome::FlightNotFound);
    };

    reassign_owner(&mut tx, flight_id, new_user_id).await?;
    carry_private_glider(&mut tx, flight_id, new_user_id, &glider).await?;

    tx.commit().await?;
    Ok(TransferOutcome::Transferred)
}

async fn user_exists(
    tx: &mut Transaction<'_, Postgres>,
    user_id: i32,
) -> Result<bool, sqlx::Error> {
    let found: Option<bool> = sqlx::query_scalar("SELECT TRUE FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(&mut **tx)
        .await?;
    Ok(found.is_some())
}

/// "FOR UPDATE" locks the DB record of the flight.
async fn lock_flight(
    tx: &mut Transaction<'_, Postgres>,
    flight_id: &str,
) -> Result<Option<FlightGlider>, sqlx::Error> {
    let row: Option<(i32, String, String, String)> = sqlx::query_as(
        "SELECT user_id, brand_id, kind::text, model_id \
         FROM flights WHERE id = $1 FOR UPDATE",
    )
    .bind(flight_id)
    .fetch_optional(&mut **tx)
    .await?;
    Ok(
        row.map(|(owner_id, brand_id, kind, model_id)| FlightGlider {
            owner_id,
            brand_id,
            kind,
            model_id,
        }),
    )
}

async fn reassign_owner(
    tx: &mut Transaction<'_, Postgres>,
    flight_id: &str,
    new_user_id: i32,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE flights SET user_id = $2 WHERE id = $1")
        .bind(flight_id)
        .bind(new_user_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// Re-key the flight's glider under `new_user_id` when it's private and owned
/// by the previous owner. Canonical gliders are shared and need no work.
async fn carry_private_glider(
    tx: &mut Transaction<'_, Postgres>,
    flight_id: &str,
    new_user_id: i32,
    glider: &FlightGlider,
) -> Result<(), sqlx::Error> {
    let Some(model) = load_private_model(tx, glider).await? else {
        return Ok(());
    };

    let new_brand_id =
        carry_private_brand(tx, &glider.brand_id, glider.owner_id, new_user_id).await?;
    let new_model_id = reslug(&glider.model_id, new_user_id);
    insert_private_model(
        tx,
        &new_brand_id,
        &glider.kind,
        &new_model_id,
        &model,
        new_user_id,
    )
    .await?;

    repoint_flight(tx, flight_id, &new_brand_id, &new_model_id).await?;

    reap_orphan_model(tx, glider).await?;
    reap_orphan_brand(tx, &glider.brand_id, glider.owner_id).await?;
    Ok(())
}

/// The model row if it's private and owned by `glider.owner_id`. Canonical
/// (`user_id IS NULL`) or someone else's rows return `None` — nothing to carry.
async fn load_private_model(
    tx: &mut Transaction<'_, Postgres>,
    glider: &FlightGlider,
) -> Result<Option<PrivateModel>, sqlx::Error> {
    let row: Option<(String, String, bool)> = sqlx::query_as(
        "SELECT name, class::text, is_tandem FROM models \
         WHERE brand_id = $1 AND kind = $2::glider_kind AND id = $3 AND user_id = $4",
    )
    .bind(&glider.brand_id)
    .bind(&glider.kind)
    .bind(&glider.model_id)
    .bind(glider.owner_id)
    .fetch_optional(&mut **tx)
    .await?;
    Ok(row.map(|(name, class, is_tandem)| PrivateModel {
        name,
        class,
        is_tandem,
    }))
}

/// The brand id to hang the carried model on. When the old brand is private and
/// owned by `old_user`, clone it under `new_user_id` and return the new id;
/// otherwise it's a canonical brand — shared — so keep it as-is.
async fn carry_private_brand(
    tx: &mut Transaction<'_, Postgres>,
    brand_id: &str,
    old_user: i32,
    new_user_id: i32,
) -> Result<String, sqlx::Error> {
    let name: Option<String> =
        sqlx::query_scalar("SELECT name FROM brands WHERE id = $1 AND user_id = $2")
            .bind(brand_id)
            .bind(old_user)
            .fetch_optional(&mut **tx)
            .await?;
    let Some(name) = name else {
        return Ok(brand_id.to_owned());
    };

    let new_brand_id = reslug(brand_id, new_user_id);
    sqlx::query(
        "INSERT INTO brands (id, name, user_id) VALUES ($1, $2, $3) \
         ON CONFLICT (id) DO NOTHING",
    )
    .bind(&new_brand_id)
    .bind(&name)
    .bind(new_user_id)
    .execute(&mut **tx)
    .await?;
    Ok(new_brand_id)
}

async fn insert_private_model(
    tx: &mut Transaction<'_, Postgres>,
    brand_id: &str,
    kind: &str,
    model_id: &str,
    model: &PrivateModel,
    user_id: i32,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO models (brand_id, kind, id, name, class, is_tandem, user_id) \
         VALUES ($1, $2::glider_kind, $3, $4, $5::glider_class, $6, $7) \
         ON CONFLICT (brand_id, kind, id) DO NOTHING",
    )
    .bind(brand_id)
    .bind(kind)
    .bind(model_id)
    .bind(&model.name)
    .bind(&model.class)
    .bind(model.is_tandem)
    .bind(user_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn repoint_flight(
    tx: &mut Transaction<'_, Postgres>,
    flight_id: &str,
    brand_id: &str,
    model_id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE flights SET brand_id = $2, model_id = $3 WHERE id = $1")
        .bind(flight_id)
        .bind(brand_id)
        .bind(model_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// Delete the old private model when no flight references it anymore. Mirrors
/// the model half of `cleanup_orphan_custom_glider_data`.
async fn reap_orphan_model(
    tx: &mut Transaction<'_, Postgres>,
    glider: &FlightGlider,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        // At this point the original flight DB-record is already updated to the
        // new owner, so we can rely on this sub-query.
        "DELETE FROM models m \
          WHERE m.user_id = $1 AND m.brand_id = $2 AND m.kind = $3::glider_kind AND m.id = $4 \
            AND NOT EXISTS ( \
                SELECT 1 FROM flights f \
                 WHERE f.brand_id = m.brand_id AND f.kind = m.kind AND f.model_id = m.id \
            )",
    )
    .bind(glider.owner_id)
    .bind(&glider.brand_id)
    .bind(&glider.kind)
    .bind(&glider.model_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Delete the old private brand when no model references it anymore. Mirrors
/// the brand half of `cleanup_orphan_custom_glider_data`; canonical brands are
/// excluded by the `user_id` filter.
async fn reap_orphan_brand(
    tx: &mut Transaction<'_, Postgres>,
    brand_id: &str,
    old_user: i32,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "DELETE FROM brands b \
          WHERE b.user_id = $1 AND b.id = $2 \
            AND NOT EXISTS (SELECT 1 FROM models m WHERE m.brand_id = b.id)",
    )
    .bind(old_user)
    .bind(brand_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

/// Re-key a private `<old_user>:<slug>` id under `new_user_id`, preserving the
/// slug after the `:` prefix. Only ever called on rows already known private.
fn reslug(custom_id: &str, new_user_id: i32) -> String {
    let slug = custom_id
        .split_once(':')
        .map_or(custom_id, |(_, slug)| slug);
    format!("{new_user_id}:{slug}")
}
