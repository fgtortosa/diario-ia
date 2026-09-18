//! Router y handlers de la API REST (`/api/v1`).

use axum::extract::{Path, Query, State};
use axum::middleware::from_fn_with_state;
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde::Deserialize;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use diario_shared::{
    Application, CreatedEntry, DayCount, Entry, EntryPage, EntryQuery, NewEntry, TagCount,
};

use crate::auth::{require_viewer, require_write_key};
use crate::error::{AppError, AppResult};
use crate::state::{blocking, AppState};
use crate::static_files::static_handler;

pub fn router(state: AppState) -> Router {
    let mut read = Router::new()
        .route("/applications", get(list_applications))
        .route("/entries", get(query_entries))
        .route("/entries/{id}", get(get_entry))
        .route("/tags", get(list_tags))
        .route("/stats", get(stats));

    let mut write = Router::new().route("/entries", post(create_entry));

    // Marcar el estado de exportacion exige API key, como cualquier escritura.
    //
    // No es una marca inocua: desmarcar hace que el exportador escriba ficheros
    // en repositorios del disco, y marcar suprime para siempre la exportacion de
    // esa entrada. Con require_viewer inactivo mientras no haya
    // DIARIO_VIEWER_TOKEN, y el bind por defecto en 0.0.0.0, ponerlo del lado de
    // la lectura significaba que cualquiera en la red podia provocar escrituras
    // en disco y perdida silenciosa de registros.
    //
    // El boton de la web necesita que este abierto, asi que hay una opcion
    // explicita para instancias locales. Apagada por defecto: que la interfaz
    // sea comoda no puede decidir donde esta el limite de permisos.
    if state.config.marcado_abierto {
        read = read.route("/entries/{id}/exported", put(set_exported));
    } else {
        write = write.route("/entries/{id}/exported", put(set_exported));
    }

    let read = read.route_layer(from_fn_with_state(state.clone(), require_viewer));
    let write = write.route_layer(from_fn_with_state(state.clone(), require_write_key));

    let api = read.merge(write);

    Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .nest("/api/v1", api)
        .fallback(static_handler)
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(state)
}

#[derive(serde::Deserialize)]
struct EstadoExportacion {
    exported: bool,
}

/// Marca o desmarca a mano el estado de exportacion de una entrada.
///
/// Desmarcarla hace que el exportador la reescriba en la siguiente pasada; el
/// fichero ya existente se respeta. Marcarla sin que el fichero exista la deja
/// sin escribir. La interfaz lo advierte.
async fn set_exported(
    State(state): State<AppState>,
    cabeceras: axum::http::HeaderMap,
    Path(id): Path<i64>,
    Json(cuerpo): Json<EstadoExportacion>,
) -> AppResult<Json<serde_json::Value>> {
    let store = state.store.clone();
    let existe = blocking(move || store.marcar_exportacion_manual(id, cuerpo.exported)).await?;
    state.config.traza(
        origen(&cabeceras),
        "marcar_exportacion",
        &format!("id={id} exportada={}", cuerpo.exported),
    );
    if !existe {
        return Err(AppError::NotFound);
    }
    // Al desmarcar, se despierta al exportador para que la recoja ya.
    if !cuerpo.exported {
        state.avisar_exportador.notify_one();
    }
    Ok(Json(
        serde_json::json!({ "id": id, "exported": cuerpo.exported }),
    ))
}

/// De donde viene la peticion, para el modo log. El puente MCP se identifica
/// con una cabecera; todo lo demas es la web o un script.
fn origen(cabeceras: &axum::http::HeaderMap) -> &'static str {
    match cabeceras
        .get("x-diario-origen")
        .and_then(|v| v.to_str().ok())
    {
        Some("mcp") => "mcp",
        _ => "rest",
    }
}

async fn list_tags(
    State(state): State<AppState>,
    cabeceras: axum::http::HeaderMap,
) -> AppResult<Json<Vec<TagCount>>> {
    let store = state.store.clone();
    let tags = blocking(move || store.contar_etiquetas()).await?;
    state.config.traza(
        origen(&cabeceras),
        "listar_etiquetas",
        &format!("n={}", tags.len()),
    );
    Ok(Json(tags))
}

async fn list_applications(
    State(state): State<AppState>,
    cabeceras: axum::http::HeaderMap,
) -> AppResult<Json<Vec<Application>>> {
    let store = state.store.clone();
    let apps = blocking(move || store.list_applications()).await?;
    state.config.traza(
        origen(&cabeceras),
        "listar_aplicaciones",
        &format!("n={}", apps.len()),
    );
    Ok(Json(apps))
}

async fn query_entries(
    State(state): State<AppState>,
    cabeceras: axum::http::HeaderMap,
    Query(q): Query<EntryQuery>,
) -> AppResult<Json<EntryPage>> {
    let store = state.store.clone();
    let page = blocking(move || store.query_entries(&q)).await?;
    state.config.traza(
        origen(&cabeceras),
        "consultar_entradas",
        &format!("n={}", page.entries.len()),
    );
    Ok(Json(page))
}

async fn get_entry(
    State(state): State<AppState>,
    cabeceras: axum::http::HeaderMap,
    Path(id): Path<i64>,
) -> AppResult<Json<Entry>> {
    let store = state.store.clone();
    let entry = blocking(move || store.get_entry(id)).await?;
    state
        .config
        .traza(origen(&cabeceras), "consultar_entrada", &format!("id={id}"));
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
    cabeceras: axum::http::HeaderMap,
    Json(new): Json<NewEntry>,
) -> AppResult<Json<CreatedEntry>> {
    let store = state.store.clone();
    let id = blocking(move || store.create_entry(&new, chrono::Utc::now())).await?;
    state
        .config
        .traza(origen(&cabeceras), "crear_entrada", &format!("id={id}"));
    // El aviso va despues de que la entrada este commiteada, y no se espera a
    // que el exportador termine: un fallo de disco no puede tumbar el registro.
    state.avisar_exportador.notify_one();
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
                tareas_dir: None,
                log_ops: false,
                marcado_abierto: false,
            }),
            avisar_exportador: Arc::new(tokio::sync::Notify::new()),
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
    async fn crear_una_entrada_despierta_al_exportador() {
        // Prueba el cableado, no la exportacion: que el handler avise. Sin esto
        // el exportador solo correria al arrancar el servidor.
        let state = test_state();
        let app = router(state.clone());
        let aviso = state.avisar_exportador.clone();

        let esperando = tokio::spawn(async move { aviso.notified().await });
        // Margen para que el waiter se registre antes de disparar el aviso.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/entries")
                    .header("content-type", "application/json")
                    .body(Body::from(post_entry_body("mi-app", "Titulo")))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        tokio::time::timeout(std::time::Duration::from_secs(2), esperando)
            .await
            .expect("el exportador no recibio el aviso")
            .unwrap();
    }

    async fn crear_entrada_en_bootstrap(app: &axum::Router) {
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/entries")
                    .header("content-type", "application/json")
                    .body(Body::from(post_entry_body("mi-app", "Titulo")))
                    .unwrap(),
            )
            .await
            .unwrap();
    }

    fn peticion_de_marcado(exportada: bool) -> Request<Body> {
        Request::builder()
            .method("PUT")
            .uri("/api/v1/entries/1/exported")
            .header("content-type", "application/json")
            .body(Body::from(format!(r#"{{"exported":{exportada}}}"#)))
            .unwrap()
    }

    #[tokio::test]
    async fn marcar_exportacion_exige_api_key_por_defecto() {
        // Marcar no es inocuo: desmarcar provoca escrituras en disco y marcar
        // suprime la exportacion. Con la lectura abierta por defecto y el bind
        // en 0.0.0.0, dejarlo sin credencial es acceso roto.
        let state = test_state();
        let app = router(state.clone());
        crear_entrada_en_bootstrap(&app).await;
        state.store.create_api_key("k", "write").unwrap();

        // Se marca como exportada, que es la direccion en la que el cambio se
        // nota: la entrada nace pendiente, asi que desmarcarla no probaria nada.
        let resp = app.oneshot(peticion_de_marcado(true)).await.unwrap();

        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            state.store.entradas_pendientes(10).unwrap().len(),
            1,
            "el 401 no impidio el cambio"
        );
    }

    #[tokio::test]
    async fn con_marcado_abierto_no_exige_api_key() {
        let mut config = (*test_state().config).clone();
        config.marcado_abierto = true;
        let state = AppState {
            store: Store::in_memory().unwrap(),
            config: Arc::new(config),
            avisar_exportador: Arc::new(tokio::sync::Notify::new()),
        };
        let app = router(state.clone());
        crear_entrada_en_bootstrap(&app).await;
        state.store.create_api_key("k", "write").unwrap();

        let resp = app.oneshot(peticion_de_marcado(true)).await.unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        assert!(
            state.store.entradas_pendientes(10).unwrap().is_empty(),
            "con marcado abierto el cambio deberia haberse aplicado"
        );
    }

    #[tokio::test]
    async fn health_ok() {
        let app = router(test_state());
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/healthz")
                    .body(Body::empty())
                    .unwrap(),
            )
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
            .oneshot(
                Request::builder()
                    .uri("/api/v1/applications")
                    .body(Body::empty())
                    .unwrap(),
            )
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
            .oneshot(
                Request::builder()
                    .uri("/alguna/ruta/cliente")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let ct = resp
            .headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        assert!(ct.contains("text/html"));
    }
}
