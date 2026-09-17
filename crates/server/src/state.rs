//! Estado compartido entre handlers y utilidades comunes.

use std::sync::Arc;

use crate::config::ServerConfig;
use crate::error::{AppError, AppResult};
use crate::storage::Store;

#[derive(Clone)]
pub struct AppState {
    pub store: Store,
    pub config: Arc<ServerConfig>,
    /// Se dispara tras cada entrada creada para despertar al exportador. Es un
    /// Notify y no un canal con cola a proposito: no importa cuantos avisos se
    /// pierdan, porque el exportador siempre procesa TODAS las pendientes.
    pub avisar_exportador: Arc<tokio::sync::Notify>,
}

/// Ejecuta una operacion de BD (sincrona) en el pool de hilos bloqueantes.
pub async fn blocking<F, T>(f: F) -> AppResult<T>
where
    F: FnOnce() -> AppResult<T> + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| AppError::Other(anyhow::anyhow!("tarea bloqueante cancelada: {e}")))?
}
