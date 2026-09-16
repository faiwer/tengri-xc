use axum::{Router, extract::State, http::StatusCode, routing::post};

use crate::AppState;

pub fn public_router() -> Router<AppState> {
    Router::new().route("/html/reload", post(reload))
}

/// Drops everything the server-rendered `index.html` is built from; the next
/// document request re-fetches the shell from `APP_BASE_URL` and re-reads the
/// site settings. Unauthenticated on purpose — it clears caches and nothing
/// else, and the deploy script has no session to present.
async fn reload(State(state): State<AppState>) -> StatusCode {
    state.clear_html_shell();
    state.clear_site_meta();
    StatusCode::NO_CONTENT
}

/// In-crate rather than in `tests/`: the cache is `pub(crate)`, so an
/// integration test could only assert the status code.
#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::{
        body::Body,
        http::{Method, Request, StatusCode},
    };
    use sqlx::postgres::PgPoolOptions;
    use tower::ServiceExt;

    use crate::{AppState, build_root, site::SiteMeta, state::PLACEHOLDER_DB_URL};

    #[tokio::test]
    async fn reload_drops_the_cached_shell() {
        let pool = PgPoolOptions::new()
            .connect_lazy(PLACEHOLDER_DB_URL)
            .expect("build lazy pool");
        let state = AppState::new_for_tests(pool, &[0u8; 32], false);
        state.cache_html_shell(Arc::new(
            crate::html::HtmlShell::parse("<html><head><title>stale</title></head></html>")
                .expect("parse test shell"),
        ));
        state.cache_site_meta(Arc::new(SiteMeta {
            site_name: "Stale".to_owned(),
            site_description: None,
        }));

        let response = build_root(state.clone())
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/html/reload")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert!(state.html_shell().is_none());
        assert!(state.site_meta().is_none());
    }
}
