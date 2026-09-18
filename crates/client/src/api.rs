//! Cliente HTTP contra la API REST del servidor central (mismo origen).

use diario_shared::{query_de, Application, Entry, EntryPage, EntryQuery, TagCount};
use gloo_net::http::Request;

/// Fuerza una pasada del exportador. Devuelve (escritas, pendientes).
pub async fn exportar_ahora() -> Result<(i64, i64), String> {
    let resp = Request::post("/api/v1/exportar")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.ok() {
        return Err(format!("el servidor respondio {}", resp.status()));
    }
    let v: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    Ok((
        v["escritas"].as_i64().unwrap_or(0),
        v["pendientes"].as_i64().unwrap_or(0),
    ))
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

/// Entradas que cumplen los filtros (resumen, con fragmento).
///
/// El limite se pone aqui y no en los filtros de la vista, para que no salga en
/// la URL del navegador: es un detalle de la peticion, no parte de la busqueda
/// que el usuario comparte.
pub async fn fetch_entries(q: &EntryQuery) -> Result<EntryPage, String> {
    let mut q = q.clone();
    q.limit = Some(200);
    Request::get(&format!("/api/v1/entries?{}", query_de(&q)))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())
}

/// Entradas enteras que cumplen los filtros, en orden ascendente.
///
/// Mismos filtros que el listado, otra ruta. El servidor responde 400 cuando la
/// busqueda se pasa de tamano: ese mensaje se devuelve tal cual, porque dice
/// por donde acotar.
pub async fn fetch_hilo(q: &EntryQuery) -> Result<Vec<Entry>, String> {
    let resp = Request::get(&format!("/api/v1/hilo?{}", query_de(q)))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !resp.ok() {
        let detalle: serde_json::Value = resp.json().await.unwrap_or_default();
        return Err(detalle["error"]
            .as_str()
            .unwrap_or("no se pudo cargar el hilo")
            .to_string());
    }
    resp.json().await.map_err(|e| e.to_string())
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
