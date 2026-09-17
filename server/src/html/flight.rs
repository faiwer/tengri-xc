use chrono::{DateTime, Utc};
use chrono_tz::Tz;
use tengri_scoring::RouteType;

use crate::{
    AppError, AppState,
    html::{Page, meta::PageMeta},
    site::SiteMeta,
};

/// Metadata for `/flight/:id`. An unknown id still renders the shell — the SPA
/// shows its own not-found screen — but the crawler gets a 404.
pub(super) async fn resolve(state: &AppState, id: &str, site: &SiteMeta) -> Result<Page, AppError> {
    match fetch(state.pool(), id).await? {
        Some(row) => Ok(Page::ok(build(
            &row,
            site,
            preview_url(state.api_public_url(), id),
        ))),
        None => Ok(Page::not_found(PageMeta::site(site))),
    }
}

/// The flight's own preview, rendered on demand by `GET /tracks/{id}/og.jpg`.
/// Built from `API_PUBLIC_URL` because crawlers won't resolve a relative one,
/// and the image comes from the API rather than the static host.
fn preview_url(api_public_url: &str, id: &str) -> String {
    format!("{api_public_url}/tracks/{id}/og.jpg")
}

/// Deliberately not `fetch_track_md`: that one runs a second query for the full
/// route list and decodes far more than a headline needs.
async fn fetch(pool: &sqlx::PgPool, id: &str) -> Result<Option<FlightRow>, AppError> {
    sqlx::query_as::<_, FlightRow>(
        "SELECT u.name AS pilot, f.takeoff_at, f.takeoff_timezone, f.duration, \
                f.main_route_type, f.main_score::float8 AS main_score, f.main_distance, \
                s.name AS takeoff_site \
         FROM flights f \
         JOIN users u ON u.id = f.user_id \
         LEFT JOIN sites s ON s.id = f.closest_takeoff_id \
         WHERE f.id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|e| AppError::Internal(e.into()))
}

#[derive(sqlx::FromRow)]
struct FlightRow {
    pilot: String,
    takeoff_at: DateTime<Utc>,
    takeoff_timezone: String,
    /// Whole seconds, from the generated column.
    duration: i32,
    main_route_type: Option<RouteType>,
    main_score: Option<f64>,
    /// Metres.
    main_distance: Option<i32>,
    takeoff_site: Option<String>,
}

fn build(row: &FlightRow, site: &SiteMeta, image: String) -> PageMeta {
    let date = format_date(row);

    let headline = match (row.main_distance, row.main_route_type) {
        (Some(distance), Some(route_type)) => format!(
            // TODO: find a way to detect if we need to use imperial or metric units
            "{:.1} km {}",
            f64::from(distance) / 1000.0,
            route_label(route_type)
        ),
        // Not scored yet: the date is the only thing left that distinguishes
        // this flight from the pilot's others.
        _ => date.clone(),
    };

    let mut description = date;
    if let Some(ref takeoff_site) = row.takeoff_site {
        description.push_str(" from ");
        description.push_str(takeoff_site);
    }
    description.push_str(". ");
    description.push_str(&format_duration(row.duration));
    if let Some(score) = row.main_score {
        description.push_str(&format!(", {score:.2} points"));
    }
    description.push('.');

    PageMeta {
        title: format!("{} - {} | {}", row.pilot, headline, site.site_name),
        description: Some(description),
        image: Some(image),
    }
}

/// The pilot's local date, not the viewer's: a flight belongs to the day it was
/// flown on the hill.
fn format_date(row: &FlightRow) -> String {
    let tz: Tz = row.takeoff_timezone.parse().unwrap_or(Tz::UTC);
    row.takeoff_at
        .with_timezone(&tz)
        .format("%-d %b %Y")
        .to_string()
}

fn format_duration(seconds: i32) -> String {
    let minutes = seconds.max(0) / 60;
    let (hours, minutes) = (minutes / 60, minutes % 60);
    if hours > 0 {
        format!("{hours}h {minutes:02}m")
    } else {
        format!("{minutes}m")
    }
}

fn route_label(route_type: RouteType) -> &'static str {
    match route_type {
        RouteType::FreeDistance => "free distance",
        RouteType::FreeTriangle => "free triangle",
        RouteType::FaiTriangle => "FAI triangle",
        RouteType::Task => "task",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn site() -> SiteMeta {
        SiteMeta {
            site_name: "Tengri XC".to_owned(),
            site_description: Some("Ignored on flight pages".to_owned()),
        }
    }

    fn row() -> FlightRow {
        FlightRow {
            pilot: "Alexey".to_owned(),
            // 2026-09-14 04:30 UTC = 10:30 in Bishkek.
            takeoff_at: DateTime::from_timestamp(1_789_360_200, 0).unwrap(),
            takeoff_timezone: "Asia/Bishkek".to_owned(),
            duration: 4 * 3600 + 12 * 60,
            main_route_type: Some(RouteType::FaiTriangle),
            main_score: Some(245.673),
            main_distance: Some(123_400),
            takeoff_site: Some("Kadji-Sai".to_owned()),
        }
    }

    fn build_meta(row: &FlightRow) -> PageMeta {
        build(
            row,
            &site(),
            preview_url("https://tengri.test/api", "abc123"),
        )
    }

    #[test]
    fn a_scored_flight_reads_as_a_headline() {
        let meta = build_meta(&row());

        assert_eq!(meta.title, "Alexey - 123.4 km FAI triangle | Tengri XC");
        assert_eq!(
            meta.description.unwrap(),
            "14 Sep 2026 from Kadji-Sai. 4h 12m, 245.67 points."
        );
    }

    #[test]
    fn the_preview_is_the_flight_s_own_image() {
        let meta = build_meta(&row());

        assert_eq!(
            meta.image.unwrap(),
            "https://tengri.test/api/tracks/abc123/og.jpg"
        );
    }

    #[test]
    fn an_unscored_flight_falls_back_to_the_date() {
        let meta = build_meta(&FlightRow {
            main_route_type: None,
            main_score: None,
            main_distance: None,
            ..row()
        });

        assert_eq!(meta.title, "Alexey - 14 Sep 2026 | Tengri XC");
        assert_eq!(
            meta.description.unwrap(),
            "14 Sep 2026 from Kadji-Sai. 4h 12m."
        );
    }

    #[test]
    fn an_unknown_takeoff_site_drops_the_clause() {
        let meta = build_meta(&FlightRow {
            takeoff_site: None,
            duration: 47 * 60,
            ..row()
        });

        assert_eq!(
            meta.description.unwrap(),
            "14 Sep 2026. 47m, 245.67 points."
        );
    }

    #[test]
    fn the_takeoff_timezone_decides_the_date() {
        // 22:30 UTC on the 14th is already the 15th in Bishkek (UTC+6).
        let meta = build_meta(&FlightRow {
            takeoff_at: DateTime::from_timestamp(1_789_425_000, 0).unwrap(),
            ..row()
        });

        assert!(meta.description.unwrap().starts_with("15 Sep 2026"));
    }
}
