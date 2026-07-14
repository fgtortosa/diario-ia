//! Servido de la SPA embebida en el binario (rust-embed) con fallback a
//! index.html para las rutas de cliente (history routing de Leptos).

use axum::body::Body;
use axum::http::{header, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "../client/dist"]
struct Assets;

fn serve_asset(path: &str) -> Option<Response> {
    let content = Assets::get(path)?;
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    let body = Body::from(content.data.into_owned());
    Some(
        Response::builder()
            .header(header::CONTENT_TYPE, mime.as_ref())
            .header(header::CACHE_CONTROL, cache_control(path))
            .body(body)
            .unwrap(),
    )
}

fn cache_control(path: &str) -> &'static str {
    // Los bundles de trunk llevan hash en el nombre -> cache larga.
    if path.ends_with(".wasm") || path.ends_with(".js") || path.ends_with(".css") {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    }
}

pub async fn static_handler(uri: Uri) -> Response {
    let raw = uri.path().trim_start_matches('/');
    let path = if raw.is_empty() { "index.html" } else { raw };

    if let Some(resp) = serve_asset(path) {
        return resp;
    }
    // Ruta de cliente desconocida -> servir la SPA.
    serve_asset("index.html")
        .unwrap_or_else(|| (StatusCode::NOT_FOUND, "no encontrado").into_response())
}
