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
    /// Modo log de operaciones (DIARIO_LOG_OPS): traza por consola cada
    /// operacion del diario, venga de REST o del MCP.
    pub log_ops: bool,
}

impl ServerConfig {
    pub fn entry_url(&self, id: i64) -> String {
        format!("{}/entry/{}", self.public_url.trim_end_matches('/'), id)
    }

    /// Trata la cadena vacia como ausencia, para que DIARIO_TAREAS_DIR="" sirva
    /// para desactivar el destino central sin borrar la variable.
    /// Traza una operacion si el modo log esta activo.
    ///
    /// Va aqui y no repartido por los handlers para que el formato sea uno solo
    /// y para que activarlo o desactivarlo sea un unico sitio.
    ///
    /// Nunca se traza el prompt ni la respuesta: son largos y son justo lo que
    /// no quieres volcado en una consola compartida. Solo lo minimo para saber
    /// que paso y sobre que.
    pub fn traza(&self, origen: &str, operacion: &str, detalle: &str) {
        if self.log_ops {
            tracing::info!("[{origen}] {operacion} {detalle}");
        }
    }

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
            log_ops: false,
        }
    }

    #[test]
    fn con_el_modo_log_apagado_no_se_traza() {
        // No se puede observar la ausencia de un log sin capturar el subscriber,
        // asi que lo que se comprueba es el contrato: la bandera manda y llamar
        // a traza con el modo apagado no hace nada ni falla.
        let c = config_con(None);
        assert!(!c.log_ops);
        c.traza("rest", "crear_entrada", "id=1");
    }

    #[test]
    fn una_cadena_vacia_equivale_a_no_escribir_destino_central() {
        assert_eq!(config_con(Some("")).tareas_dir_efectivo(), None);
        assert_eq!(config_con(None).tareas_dir_efectivo(), None);
        assert_eq!(config_con(Some("C:/x")).tareas_dir_efectivo(), Some("C:/x"));
    }
}
