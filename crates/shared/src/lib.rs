//! DTOs compartidos entre el servidor (Axum/MCP) y el cliente (Leptos/WASM).
//!
//! Todo lo que viaja por la API REST y por las herramientas MCP vive aqui,
//! de modo que servidor y cliente comparten un unico modelo de datos.

use chrono::{DateTime, Local, Utc};
use serde::{Deserialize, Serialize};

/// Una aplicacion sobre la que trabajan los agentes (p.ej. "portal-alumnos").
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Application {
    pub id: i64,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub entry_count: i64,
    pub last_activity: Option<DateTime<Utc>>,
}

/// Documento markdown adicional adjunto a una entrada (entrada de entrada).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Attachment {
    pub id: i64,
    pub filename: String,
    pub kind: Option<String>,
    pub content_markdown: String,
    pub content_html: String,
}

/// Payload para adjuntar un documento markdown al crear una entrada.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NewAttachment {
    pub filename: String,
    #[serde(default)]
    pub kind: Option<String>,
    pub content_markdown: String,
}

/// Payload que envia un agente para registrar una tarea en el diario.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NewEntry {
    /// Slug o nombre de la aplicacion; se crea automaticamente si no existe.
    pub application: String,
    /// Identificador del agente (p.ej. "claude-code").
    pub agent: String,
    #[serde(default)]
    pub model: Option<String>,
    pub title: String,
    pub prompt: String,
    #[serde(default)]
    pub task_summary: Option<String>,
    pub response_markdown: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub attachments: Vec<NewAttachment>,
    #[serde(default)]
    pub tokens_input: Option<i64>,
    #[serde(default)]
    pub tokens_output: Option<i64>,
    #[serde(default)]
    pub duration_ms: Option<i64>,
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
}

/// Resumen de una entrada, para la vista de timeline/listado.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EntrySummary {
    pub id: i64,
    pub application_slug: String,
    pub application_name: String,
    pub agent_name: String,
    pub model: Option<String>,
    pub title: String,
    pub snippet: String,
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
}

/// Entrada completa, con markdown renderizado y adjuntos.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Entry {
    pub id: i64,
    pub application_slug: String,
    pub application_name: String,
    pub agent_name: String,
    pub model: Option<String>,
    pub title: String,
    pub prompt: String,
    pub task_summary: Option<String>,
    pub response_markdown: String,
    pub response_html: String,
    pub status: Option<String>,
    pub tags: Vec<String>,
    pub tokens_input: Option<i64>,
    pub tokens_output: Option<i64>,
    pub duration_ms: Option<i64>,
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    /// Cuando se escribio en los repositorios. None = pendiente de exportar.
    #[serde(default)]
    pub exported_at: Option<DateTime<Utc>>,
    /// Nombre del fichero con el que se exporta esta entrada, **calculado en el
    /// servidor**.
    ///
    /// Viaja en la respuesta en vez de calcularse en el navegador porque el
    /// nombre lleva la hora local: si el navegador y el servidor estan en zonas
    /// distintas, calcularlo en los dos sitios da nombres distintos y se rompe
    /// la garantia de que descargar a mano y exportar produzcan lo mismo.
    #[serde(default)]
    pub export_filename: String,
    pub attachments: Vec<Attachment>,
}

/// Filtros de consulta de entradas (query string en REST).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct EntryQuery {
    /// Slug de aplicacion.
    #[serde(default)]
    pub application: Option<String>,
    /// Fecha desde (YYYY-MM-DD, inclusive).
    #[serde(default)]
    pub from: Option<String>,
    /// Fecha hasta (YYYY-MM-DD, inclusive).
    #[serde(default)]
    pub to: Option<String>,
    /// Etiqueta exacta.
    #[serde(default)]
    pub tag: Option<String>,
    /// Busqueda de texto completo.
    #[serde(default)]
    pub q: Option<String>,
    /// Tamano de pagina (por defecto 50, maximo 200).
    #[serde(default)]
    pub limit: Option<i64>,
    /// Cursor de paginacion: id maximo devuelto en la pagina anterior.
    #[serde(default)]
    pub cursor: Option<i64>,
    /// Estado de exportacion: Some(true) solo exportadas, Some(false) solo
    /// pendientes, None todas.
    #[serde(default)]
    pub exported: Option<bool>,
}

/// Pagina de resultados de entradas.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EntryPage {
    pub entries: Vec<EntrySummary>,
    pub next_cursor: Option<i64>,
}

/// Recuento de entradas por etiqueta, para la lista lateral.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TagCount {
    pub tag: String,
    pub count: i64,
}

/// Recuento de entradas por dia, para el heatmap/calendario.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DayCount {
    pub day: String,
    pub count: i64,
}

/// Respuesta al crear una entrada.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CreatedEntry {
    pub id: i64,
    pub url: String,
}

/// El markdown de una entrada.
///
/// Vive aqui y no en el servidor porque lo usan los dos lados: el exportador
/// para escribir el fichero del repositorio, y la web para el boton de
/// descargar. Con una sola definicion, lo que te bajas y lo que se exporta son
/// identicos por construccion y no por coincidencia.
pub fn markdown_de(entry: &Entry) -> String {
    let mut s = String::new();
    s.push_str(&format!("# {}\n\n", entry.title));
    s.push_str(&format!("- Aplicacion: {}\n", entry.application_name));
    s.push_str(&format!(
        "- Agente: {} ({})\n",
        entry.agent_name,
        entry.model.as_deref().unwrap_or("-")
    ));
    s.push_str(&format!("- Fecha: {}\n", entry.created_at.to_rfc3339()));
    s.push_str(&format!("- Entrada: {}\n", entry.id));
    if !entry.tags.is_empty() {
        s.push_str(&format!("- Etiquetas: {}\n", entry.tags.join(", ")));
    }
    s.push('\n');
    s.push_str(&format!("## Prompt\n\n{}\n\n", entry.prompt));
    if let Some(resumen) = &entry.task_summary {
        s.push_str(&format!("## Resumen\n\n{}\n\n", resumen));
    }
    s.push_str(&format!("## Respuesta\n\n{}\n", entry.response_markdown));
    s
}

/// `20260917-125851-redes-ice-netcore-19.md`
///
/// Fecha y hora primero para que el orden alfabetico sea el cronologico; la
/// aplicacion para que el fichero se explique solo fuera de su carpeta; y el id
/// porque varias entradas seguidas caen en el mismo segundo y sin el se pisan.
pub fn nombre_fichero(entry: &Entry) -> String {
    format!(
        "{}-{}-{}.md",
        entry
            .created_at
            .with_timezone(&Local)
            .format("%Y%m%d-%H%M%S"),
        entry.application_slug,
        entry.id
    )
}

/// Filtros -> query string, sin el `?`. Cadena vacia si no hay ninguno.
///
/// La misma funcion construye la URL del navegador y la de la API, asi que
/// `/hilo?application=x&tag=y` y `/api/v1/hilo?application=x&tag=y` llevan
/// **la misma** query por construccion: lo que ves en la barra de direcciones
/// es lo que le puedes pasar a curl o a un agente.
pub fn query_de(q: &EntryQuery) -> String {
    serde_urlencoded::to_string(q).unwrap_or_default()
}

/// Query string -> filtros. Acepta el `?` inicial y perdona lo que no entienda:
/// una URL escrita a mano con un parametro de mas no debe dejar la pagina en
/// blanco.
pub fn query_a(s: &str) -> EntryQuery {
    serde_urlencoded::from_str(s.trim_start_matches('?')).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_query_omite_los_filtros_vacios() {
        let q = EntryQuery {
            application: Some("redes-ice-netcore".into()),
            ..Default::default()
        };
        assert_eq!(query_de(&q), "application=redes-ice-netcore");
        assert_eq!(query_de(&EntryQuery::default()), "");
    }

    #[test]
    fn la_query_va_y_vuelve_sin_perder_nada() {
        // Es lo que hace navegable la vista: la URL tiene que reconstruir la
        // busqueda entera, con espacios y acentos incluidos.
        let q = EntryQuery {
            application: Some("diario-ia".into()),
            tag: Some("build".into()),
            from: Some("2026-09-01".into()),
            to: Some("2026-09-18".into()),
            q: Some("exportacion & marcado".into()),
            exported: Some(false),
            ..Default::default()
        };

        assert_eq!(query_a(&format!("?{}", query_de(&q))), q);
    }

    #[test]
    fn una_query_con_basura_no_rompe_la_pagina() {
        let q = query_a("?tag=build&loquesea=1");
        assert_eq!(q.tag.as_deref(), Some("build"));
    }
}
