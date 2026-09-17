//! Cliente HTTP contra la API REST del servidor central (mismo origen).

use diario_shared::{Application, Entry, EntryPage, TagCount};
use gloo_net::http::Request;

fn enc(s: &str) -> String {
    js_sys::encode_uri_component(s)
        .as_string()
        .unwrap_or_default()
}

pub async fn fetch_tags() -> Result<Vec<TagCount>, String> {
    Request::get("/api/v1/tags")
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())
}

/// Marca o desmarca a mano el estado de exportacion de una entrada.
pub async fn set_exported(id: i64, exportada: bool) -> Result<(), String> {
    let resp = Request::put(&format!("/api/v1/entries/{id}/exported"))
        .json(&serde_json::json!({ "exported": exportada }))
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if resp.ok() {
        Ok(())
    } else {
        Err(format!("el servidor respondio {}", resp.status()))
    }
}

pub async fn fetch_applications() -> Result<Vec<Application>, String> {
    Request::get("/api/v1/applications")
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())
}

#[derive(Clone, Default)]
pub struct EntryFilters {
    pub application: Option<String>,
    pub from: String,
    pub to: String,
    pub search: String,
    pub tag: Option<String>,
    /// Some(false) = solo las que aun no se han exportado.
    pub exported: Option<bool>,
}

pub async fn fetch_entries(f: EntryFilters) -> Result<EntryPage, String> {
    let mut url = String::from("/api/v1/entries?limit=200");
    if let Some(a) = f.application.filter(|s| !s.is_empty()) {
        url.push_str(&format!("&application={}", enc(&a)));
    }
    if !f.from.is_empty() {
        url.push_str(&format!("&from={}", enc(&f.from)));
    }
    if !f.to.is_empty() {
        url.push_str(&format!("&to={}", enc(&f.to)));
    }
    if !f.search.is_empty() {
        url.push_str(&format!("&q={}", enc(&f.search)));
    }
    if let Some(tag) = f.tag.filter(|s| !s.is_empty()) {
        url.push_str(&format!("&tag={}", enc(&tag)));
    }
    if let Some(exportada) = f.exported {
        url.push_str(&format!("&exported={exportada}"));
    }
    Request::get(&url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())
}

pub async fn fetch_entry(id: i64) -> Result<Entry, String> {
    Request::get(&format!("/api/v1/entries/{id}"))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())
}
