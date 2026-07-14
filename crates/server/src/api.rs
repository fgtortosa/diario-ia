//! Router y handlers de la API REST (`/api/v1`).

use axum::extract::{Path, Query, State};
use axum::middleware::from_fn_with_state;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use diario_shared::{
    Application, CreatedEntry, DayCount, Entry, EntryPage, EntryQuery, NewEntry,
};

use crate::auth::{require_viewer, require_write_key};
use crate::error::{AppError, AppResult};
use crate::state::{blocking, AppState};
use crate::static_files::static_handler;

pub fn router(state: AppState) -> Router {
    let read = Router::new()
        .route("/applications", get(list_applications))
        .route("/entries", get(query_entries))
        .route("/entries/{id}", get(get_entry))
        .route("/stats", get(stats))
        .route_layer(from_fn_with_state(state.clone(), require_viewer));

    let write = Router::new()
        .route("/entries", post(create_entry))
        .route_layer(from_fn_with_state(state.clone(), require_write_key));

    let api = read.merge(write);

    Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .nest("/api/v1", api)
        .fallback(static_handler)
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(state)
}

async fn list_applications(State(state): State<AppState>) -> AppResult<Json<Vec<Application>>> {
    let store = state.store.clone();
    let apps = blocking(move || store.list_applications()).await?;
    Ok(Json(apps))
}

async fn query_entries(
    State(state): State<AppState>,
    Query(q): Query<EntryQuery>,
) -> AppResult<Json<EntryPage>> {
    let store = state.store.clone();
    let page = blocking(move || store.query_entries(&q)).await?;
    Ok(Json(page))
}

async fn get_entry(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> AppResult<Json<Entry>> {
    let store = state.store.clone();
    let entry = blocking(move || store.get_entry(id)).await?;
    entry.map(Json).ok_or(AppError::NotFound)
}

#[derive(Debug, Deserialize)]
struct StatsQuery {
    #[serde(default)]
    application: Option<String>,
    #[serde(default)]
    from: Option<String>,
    #[serde(default)]
    to: Option<String>,
}

async fn stats(
    State(state): State<AppState>,
    Query(q): Query<StatsQuery>,
) -> AppResult<Json<Vec<DayCount>>> {
    let store = state.store.clone();
    let counts = blocking(move || {
        store.day_counts(q.application.as_deref(), q.from.as_deref(), q.to.as_deref())
    })
    .await?;
    Ok(Json(counts))
}

async fn create_entry(
    State(state): State<AppState>,
    Json(new): Json<NewEntry>,
) -> AppResult<Json<CreatedEntry>> {
    let store = state.store.clone();
    let id = blocking(move || store.create_entry(&new, chrono::Utc::now())).await?;
    Ok(Json(CreatedEntry {
        id,
        url: state.config.entry_url(id),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ServerConfig;
    use crate::storage::Store;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use serde::de::DeserializeOwned;
    use std::sync::Arc;
    use tower::ServiceExt;

    fn test_state() -> AppState {
        AppState {
            store: Store::in_memory().unwrap(),
            config: Arc::new(ServerConfig {
                bind: "127.0.0.1:0".parse().unwrap(),
                db_path: ":memory:".into(),
                public_url: "http://test".into(),
                viewer_token: None,
            }),
        }
    }

    async fn json_body<T: DeserializeOwned>(resp: axum::response::Response) -> T {
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&bytes).unwrap()
    }

    fn post_entry_body(app: &str, title: &str) -> String {
        serde_json::json!({
            "application": app,
            "agent": "claude-code",
            "title": title,
            "prompt": "haz algo",
            "response_markdown": "# ok\n\n```mermaid\ngraph TD; A-->B;\n```",
            "tags": ["api"]
        })
        .to_string()
    }

    #[tokio::test]
    async fn health_ok() {
        let app = router(test_state());
        let resp = app
            .oneshot(Request::builder().uri("/healthz").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn create_and_query_entry_flow() {
        let state = test_state();
        let app = router(state.clone());

        // Crear (bootstrap: sin keys, permitido).
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/entries")
                    .header("content-type", "application/json")
                    .body(Body::from(post_entry_body("Portal Alumnos", "Tarea 1")))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let created: CreatedEntry = json_body(resp).await;
        assert_eq!(created.url, format!("http://test/entry/{}", created.id));

        // Listar aplicaciones.
        let resp = app
            .clone()
            .oneshot(Request::builder().uri("/api/v1/applications").body(Body::empty()).unwrap())
            .await
            .unwrap();
        let apps: Vec<Application> = json_body(resp).await;
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].slug, "portal-alumnos");

        // Consultar entradas filtrando por app.
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/entries?application=portal-alumnos")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let page: EntryPage = json_body(resp).await;
        assert_eq!(page.entries.len(), 1);

        // Obtener detalle con HTML renderizado + mermaid.
        let resp = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/entries/{}", created.id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let entry: Entry = json_body(resp).await;
        assert!(entry.response_html.contains("language-mermaid"));
    }

    #[tokio::test]
    async fn write_requires_key_once_keys_exist() {
        let state = test_state();
        // En cuanto existe una key, la escritura exige autorizacion.
        let (_, token) = state.store.create_api_key("agente", "write").unwrap();
        let app = router(state);

        // Sin token -> 401.
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/entries")
                    .header("content-type", "application/json")
                    .body(Body::from(post_entry_body("App", "t")))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

        // Con token valido -> 200.
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/entries")
                    .header("content-type", "application/json")
                    .header("authorization", format!("Bearer {token}"))
                    .body(Body::from(post_entry_body("App", "t")))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn spa_fallback_serves_index() {
        let app = router(test_state());
        let resp = app
            .oneshot(Request::builder().uri("/alguna/ruta/cliente").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let ct = resp.headers().get("content-type").unwrap().to_str().unwrap().to_string();
        assert!(ct.contains("text/html"));
    }
}
