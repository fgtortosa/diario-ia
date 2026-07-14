//! Interop con JavaScript: renderizado de diagramas y navegacion (history API).

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

#[wasm_bindgen]
extern "C" {
    /// Definida en index.html: renderiza mermaid y resalta el codigo del DOM.
    #[wasm_bindgen(js_name = __renderDiagrams)]
    pub fn render_diagrams();
}

/// Ruta actual del navegador (para deep links tipo /entry/42).
pub fn current_path() -> String {
    web_sys::window()
        .and_then(|w| w.location().pathname().ok())
        .unwrap_or_else(|| "/".to_string())
}

/// Cambia la URL sin recargar (history.pushState).
pub fn push_path(path: &str) {
    if let Some(w) = web_sys::window() {
        if let Ok(history) = w.history() {
            let _ = history.push_state_with_url(&JsValue::NULL, "", Some(path));
        }
    }
}

/// Registra un callback para el evento popstate (boton atras/adelante).
pub fn on_popstate<F: Fn() + 'static>(f: F) {
    let closure = Closure::<dyn FnMut()>::new(move || f());
    if let Some(w) = web_sys::window() {
        let _ = w.add_event_listener_with_callback("popstate", closure.as_ref().unchecked_ref());
    }
    closure.forget();
}
