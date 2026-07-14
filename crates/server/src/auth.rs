//! Middleware de autenticacion por API key.
//!
//! - Escritura (`POST /api/v1/entries`): requiere una API key valida en cuanto
//!   exista al menos una key activa. Sin keys, el servidor arranca en modo
//!   "bootstrap" y permite escribir (para poder crear la primera key/uso local).
//! - Lectura: si `DIARIO_VIEWER_TOKEN` esta configurado, se exige ese token;
//!   por defecto la lectura es libre (pensado para despliegue interno/proxy).

use axum::extract::{Request, State};
use axum::http::HeaderMap;
use axum::middleware::Next;
use axum::response::Response;

use crate::error::{AppError, AppResult};
use crate::state::{blocking, AppState};

/// Extrae un token de `Authorization: Bearer <t>` o de `X-API-Key: <t>`.
fn bearer_or_apikey(headers: &HeaderMap) -> Option<String> {
    if let Some(v) = headers.get(axum::http::header::AUTHORIZATION) {
        if let Ok(s) = v.to_str() {
            if let Some(rest) = s.strip_prefix("Bearer ") {
                return Some(rest.trim().to_string());
            }
        }
    }
    headers
        .get("x-api-key")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string())
}

/// Middleware para endpoints de escritura.
pub async fn require_write_key(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> AppResult<Response> {
    let store = state.store.clone();
    let count = blocking(move || store.active_key_count()).await?;
    if count == 0 {
        return Ok(next.run(req).await);
    }

    let token = bearer_or_apikey(req.headers()).ok_or(AppError::Unauthorized)?;
    let store = state.store.clone();
    let info = blocking(move || store.verify_api_key(&token)).await?;
    match info {
        Some(key) => {
            tracing::debug!(key = %key.name, id = key.id, "escritura autorizada");
            Ok(next.run(req).await)
        }
        None => Err(AppError::Unauthorized),
    }
}

/// Middleware para endpoints de lectura (activo solo si hay viewer_token).
pub async fn require_viewer(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> AppResult<Response> {
    let Some(expected) = state.config.viewer_token.clone() else {
        return Ok(next.run(req).await);
    };

    // Token via Authorization/X-API-Key o via query ?token=
    let provided = bearer_or_apikey(req.headers()).or_else(|| {
        req.uri().query().and_then(|q| {
            q.split('&')
                .find_map(|kv| kv.strip_prefix("token=").map(|t| t.to_string()))
        })
    });

    match provided {
        Some(t) if t == expected => Ok(next.run(req).await),
        _ => Err(AppError::Unauthorized),
    }
}
