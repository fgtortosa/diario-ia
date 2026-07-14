//! DTOs compartidos entre el servidor (Axum/MCP) y el cliente (Leptos/WASM).
//!
//! Todo lo que viaja por la API REST y por las herramientas MCP vive aqui,
//! de modo que servidor y cliente comparten un unico modelo de datos.

use chrono::{DateTime, Utc};
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
}

/// Pagina de resultados de entradas.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EntryPage {
    pub entries: Vec<EntrySummary>,
    pub next_cursor: Option<i64>,
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
