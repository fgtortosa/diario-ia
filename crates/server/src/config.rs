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
    /// Directorio central donde se acumulan las tareas de todas las
    /// aplicaciones (DIARIO_TAREAS_DIR). Vacio = no se escribe destino central.
    /// Es configurable porque la ruta habitual es de la maquina de desarrollo:
    /// un servidor desplegado en otro host no la tiene.
    pub tareas_dir: Option<String>,
}

impl ServerConfig {
    pub fn entry_url(&self, id: i64) -> String {
        format!("{}/entry/{}", self.public_url.trim_end_matches('/'), id)
    }

    /// Trata la cadena vacia como ausencia, para que DIARIO_TAREAS_DIR="" sirva
    /// para desactivar el destino central sin borrar la variable.
    pub fn tareas_dir_efectivo(&self) -> Option<&str> {
        self.tareas_dir.as_deref().filter(|t| !t.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config_con(tareas: Option<&str>) -> ServerConfig {
        ServerConfig {
            bind: "127.0.0.1:8787".parse().unwrap(),
            db_path: "diario.db".into(),
            public_url: "http://localhost:8787".into(),
            viewer_token: None,
            tareas_dir: tareas.map(|s| s.to_string()),
        }
    }

    #[test]
    fn una_cadena_vacia_equivale_a_no_escribir_destino_central() {
        assert_eq!(config_con(Some("")).tareas_dir_efectivo(), None);
        assert_eq!(config_con(None).tareas_dir_efectivo(), None);
        assert_eq!(config_con(Some("C:/x")).tareas_dir_efectivo(), Some("C:/x"));
    }
}
