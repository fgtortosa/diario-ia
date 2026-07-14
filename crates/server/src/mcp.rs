//! Servidor MCP por stdio. Actua de proxy: cada herramienta reenvia la
//! peticion al servidor central via la API REST.
//!
//! Los agentes (Claude Code, etc.) lanzan este proceso como subproceso stdio y
//! obtienen herramientas para registrar y consultar el diario.

use rmcp::handler::server::{router::tool::ToolRouter, wrapper::Parameters};
use rmcp::model::{CallToolResult, Content, ServerCapabilities, ServerInfo};
use rmcp::transport::stdio;
use rmcp::{schemars, tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler, ServiceExt};

use diario_shared::{CreatedEntry, NewEntry};

#[derive(Clone)]
pub struct DiarioMcp {
    http: reqwest::Client,
    base_url: String,
    api_key: Option<String>,
    agent: String,
    tool_router: ToolRouter<Self>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct LogTaskArgs {
    /// Aplicacion sobre la que se ha trabajado (nombre o slug). Se crea si no existe.
    pub application: String,
    /// Titulo corto y descriptivo de la tarea.
    pub title: String,
    /// Prompt o instruccion que recibio el agente.
    pub prompt: String,
    /// Respuesta o documento markdown resultante (admite diagramas ```mermaid```).
    pub response_markdown: String,
    /// Resumen breve de lo realizado (opcional).
    #[serde(default)]
    pub task_summary: Option<String>,
    /// Modelo usado, p.ej. "fable-5" (opcional).
    #[serde(default)]
    pub model: Option<String>,
    /// Etiquetas para clasificar la tarea (opcional).
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ListEntriesArgs {
    /// Slug de aplicacion para filtrar (opcional).
    #[serde(default)]
    pub application: Option<String>,
    /// Fecha desde (YYYY-MM-DD, opcional).
    #[serde(default)]
    pub from: Option<String>,
    /// Fecha hasta (YYYY-MM-DD, opcional).
    #[serde(default)]
    pub to: Option<String>,
    /// Numero maximo de entradas (opcional).
    #[serde(default)]
    pub limit: Option<i64>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct GetEntryArgs {
    /// Identificador de la entrada.
    pub id: i64,
}

#[tool_router]
impl DiarioMcp {
    pub fn new(base_url: String, api_key: Option<String>, agent: String) -> Self {
        Self {
            http: reqwest::Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
            agent,
            tool_router: Self::tool_router(),
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    #[tool(description = "Registra una tarea realizada por el agente en el diario \
        (aplicacion, prompt, resumen y respuesta/documento markdown).")]
    async fn log_task(
        &self,
        Parameters(a): Parameters<LogTaskArgs>,
    ) -> Result<CallToolResult, McpError> {
        let body = NewEntry {
            application: a.application,
            agent: self.agent.clone(),
            model: a.model,
            title: a.title,
            prompt: a.prompt,
            task_summary: a.task_summary,
            response_markdown: a.response_markdown,
            tags: a.tags,
            attachments: Vec::new(),
            tokens_input: None,
            tokens_output: None,
            duration_ms: None,
            metadata: None,
        };
        let mut req = self.http.post(self.url("/api/v1/entries")).json(&body);
        if let Some(k) = &self.api_key {
            req = req.bearer_auth(k);
        }
        let resp = req.send().await.map_err(net_err)?;
        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(McpError::internal_error(
                format!("el servidor respondio {status}: {text}"),
                None,
            ));
        }
        let created: CreatedEntry = resp.json().await.map_err(net_err)?;
        Ok(CallToolResult::success(vec![Content::text(format!(
            "Entrada #{} registrada. Puedes verla en: {}",
            created.id, created.url
        ))]))
    }

    #[tool(description = "Lista las aplicaciones del diario con su numero de tareas.")]
    async fn list_applications(&self) -> Result<CallToolResult, McpError> {
        let json = self.get_json("/api/v1/applications").await?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "Lista entradas del diario, opcionalmente filtradas por \
        aplicacion y rango de fechas, para recuperar trabajo previo.")]
    async fn list_entries(
        &self,
        Parameters(a): Parameters<ListEntriesArgs>,
    ) -> Result<CallToolResult, McpError> {
        let mut query: Vec<(String, String)> = Vec::new();
        if let Some(v) = a.application {
            query.push(("application".into(), v));
        }
        if let Some(v) = a.from {
            query.push(("from".into(), v));
        }
        if let Some(v) = a.to {
            query.push(("to".into(), v));
        }
        if let Some(v) = a.limit {
            query.push(("limit".into(), v.to_string()));
        }
        let mut req = self.http.get(self.url("/api/v1/entries")).query(&query);
        if let Some(k) = &self.api_key {
            req = req.bearer_auth(k);
        }
        let resp = req.send().await.map_err(net_err)?;
        let json = resp.text().await.map_err(net_err)?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "Obtiene una entrada completa del diario por su id.")]
    async fn get_entry(
        &self,
        Parameters(a): Parameters<GetEntryArgs>,
    ) -> Result<CallToolResult, McpError> {
        let json = self
            .get_json(&format!("/api/v1/entries/{}", a.id))
            .await?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    async fn get_json(&self, path: &str) -> Result<String, McpError> {
        let mut req = self.http.get(self.url(path));
        if let Some(k) = &self.api_key {
            req = req.bearer_auth(k);
        }
        let resp = req.send().await.map_err(net_err)?;
        resp.text().await.map_err(net_err)
    }
}

#[tool_handler]
impl ServerHandler for DiarioMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "Diario de tareas de agentes de IA. Usa log_task para registrar cada tarea \
                 (prompt, resumen y respuesta markdown), y list_entries/get_entry para \
                 recuperar trabajo previo por aplicacion y fecha."
                    .into(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}

fn net_err<E: std::fmt::Display>(e: E) -> McpError {
    McpError::internal_error(format!("error de red hacia el servidor central: {e}"), None)
}

/// Arranca el servidor MCP por stdio.
pub async fn run(url: String, key: Option<String>) -> anyhow::Result<()> {
    let agent = std::env::var("DIARIO_AGENT").unwrap_or_else(|_| "claude-code".to_string());
    let service = DiarioMcp::new(url, key, agent).serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ServerConfig;
    use crate::state::AppState;
    use crate::storage::Store;
    use std::sync::Arc;

    /// Arranca el servidor Axum real en un puerto efimero y devuelve su base URL.
    async fn spawn_server() -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let base = format!("http://{addr}");
        let state = AppState {
            store: Store::in_memory().unwrap(),
            config: Arc::new(ServerConfig {
                bind: addr,
                db_path: ":memory:".into(),
                public_url: base.clone(),
                viewer_token: None,
            }),
        };
        let app = crate::api::router(state);
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        base
    }

    #[tokio::test]
    async fn mcp_log_task_and_list_via_rest() {
        let base = spawn_server().await;
        let mcp = DiarioMcp::new(base, None, "claude-code".into());

        let res = mcp
            .log_task(Parameters(LogTaskArgs {
                application: "Portal Alumnos".into(),
                title: "Tarea via MCP".into(),
                prompt: "haz X".into(),
                response_markdown: "# Resultado\n\n```mermaid\ngraph TD; A-->B;\n```".into(),
                task_summary: Some("resumen".into()),
                model: Some("fable-5".into()),
                tags: vec!["mcp".into()],
            }))
            .await
            .expect("log_task deberia funcionar");

        let text = format!("{:?}", res.content);
        assert!(text.contains("registrada"), "respuesta: {text}");

        let apps = mcp.list_applications().await.unwrap();
        let apps_text = format!("{:?}", apps.content);
        assert!(apps_text.contains("portal-alumnos"), "apps: {apps_text}");

        let entry = mcp.get_entry(Parameters(GetEntryArgs { id: 1 })).await.unwrap();
        let entry_text = format!("{:?}", entry.content);
        assert!(entry_text.contains("Tarea via MCP"));
    }
}
