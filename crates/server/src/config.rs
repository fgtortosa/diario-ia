//! Configuracion del servidor, leida de argumentos CLI y variables de entorno.

use std::net::SocketAddr;

#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Direccion de escucha (DIARIO_BIND, por defecto 0.0.0.0:8787).
    pub bind: SocketAddr,
    /// Ruta del fichero SQLite (DIARIO_DB).
    pub db_path: String,
    /// URL publica base para construir enlaces a entradas (DIARIO_PUBLIC_URL).
    pub public_url: String,
    /// Token de lectura opcional para proteger la SPA/GET (DIARIO_VIEWER_TOKEN).
    /// Si esta vacio, la lectura es libre (util tras un reverse-proxy interno).
    pub viewer_token: Option<String>,
}

impl ServerConfig {
    pub fn entry_url(&self, id: i64) -> String {
        format!("{}/entry/{}", self.public_url.trim_end_matches('/'), id)
    }
}
