//! Tipo de error de la aplicacion y su conversion a respuestas HTTP.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("no encontrado")]
    NotFound,

    #[error("no autorizado")]
    Unauthorized,

    #[error("peticion invalida: {0}")]
    BadRequest(String),

    #[error(transparent)]
    Db(#[from] rusqlite::Error),

    #[error(transparent)]
    Pool(#[from] r2d2::Error),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::NotFound => (StatusCode::NOT_FOUND, "no encontrado".to_string()),
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, "no autorizado".to_string()),
            AppError::BadRequest(m) => (StatusCode::BAD_REQUEST, m.clone()),
            AppError::Db(e) => {
                tracing::error!(error = %e, "error de base de datos");
                (StatusCode::INTERNAL_SERVER_ERROR, "error interno".to_string())
            }
            AppError::Pool(e) => {
                tracing::error!(error = %e, "error de pool de conexiones");
                (StatusCode::INTERNAL_SERVER_ERROR, "error interno".to_string())
            }
            AppError::Other(e) => {
                tracing::error!(error = %e, "error interno");
                (StatusCode::INTERNAL_SERVER_ERROR, "error interno".to_string())
            }
        };
        (status, Json(json!({ "error": message }))).into_response()
    }
}

pub type AppResult<T> = Result<T, AppError>;
