//! Interop con JavaScript: renderizado de diagramas y navegacion (history API).

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

#[wasm_bindgen]
extern "C" {
    /// Definida en index.html: renderiza mermaid y resalta el codigo del DOM.
    #[wasm_bindgen(js_name = __renderDiagrams)]
    pub fn render_diagrams();
}

/// Descarga un texto como fichero, sin pasar por el servidor.
///
/// El detalle ya tiene el markdown de la entrada, asi que una ruta de descarga
/// en el servidor solo duplicaria lo que ya viajo. Se hace con un Blob y un
/// ancla temporal, que es la unica forma de disparar una descarga desde el
/// navegador sin navegar fuera.
pub fn descargar_fichero(nombre: &str, contenido: &str) {
    let Some(documento) = web_sys::window().and_then(|w| w.document()) else {
        return;
    };
    let partes = js_sys::Array::new();
    partes.push(&JsValue::from_str(contenido));
    let opciones = web_sys::BlobPropertyBag::new();
    opciones.set_type("text/markdown;charset=utf-8");
    let Ok(blob) = web_sys::Blob::new_with_str_sequence_and_options(&partes, &opciones) else {
        return;
    };
    let Ok(url) = web_sys::Url::create_object_url_with_blob(&blob) else {
        return;
    };
    if let Ok(a) = documento.create_element("a") {
        let a: web_sys::HtmlAnchorElement = a.unchecked_into();
        a.set_href(&url);
        a.set_download(nombre);
        a.click();
    }
    // Sin revoke el blob se queda en memoria hasta recargar la pagina.
    let _ = web_sys::Url::revoke_object_url(&url);
}

/// Abre el dialogo de impresion del navegador, que es tambien el de "guardar
/// como PDF". Ver la hoja @media print de styles.css.
///
/// Antes de imprimir despliega los <details> cerrados. Hace falta hacerlo aqui
/// y no en CSS: el atributo `open` no se puede poner desde una hoja de estilos,
/// y cambiar el `display` del <details> no revela su contenido. Medido en
/// Chrome, un <details> cerrado en medio print tiene altura **cero**, asi que el
/// prompt desaparecia entero del PDF en vez de salir desplegado.
///
/// Se dejan abiertos: quien imprime ha pedido el contenido, y volver a cerrarlos
/// despues obligaria a adivinar cuando termina el dialogo del navegador.
pub fn imprimir() {
    let Some(w) = web_sys::window() else { return };
    if let Some(doc) = w.document() {
        if let Ok(lista) = doc.query_selector_all("details:not([open])") {
            for i in 0..lista.length() {
                if let Some(nodo) = lista.item(i) {
                    let el: web_sys::Element = nodo.unchecked_into();
                    let _ = el.set_attribute("open", "");
                }
            }
        }
    }
    let _ = w.print();
}

/// Ruta actual del navegador (para deep links tipo /entry/42).
pub fn current_path() -> String {
    web_sys::window()
        .and_then(|w| w.location().pathname().ok())
        .unwrap_or_else(|| "/".to_string())
}

/// Query string actual, con el `?` incluido (vacia si no hay).
///
/// Es la otra mitad del deep link: la ruta dice que vista es y la query dice
/// que filtros tenia. Juntas hacen la busqueda reproducible.
pub fn current_search() -> String {
    web_sys::window()
        .and_then(|w| w.location().search().ok())
        .unwrap_or_default()
}

/// true si el navegador ofrece el portapapeles asincrono.
///
/// `navigator.clipboard` solo existe en contexto seguro. localhost cuenta como
/// seguro, que es el caso normal aqui, pero abrir el diario por IP desde otra
/// maquina en http no. Se comprueba antes de usarlo para poder ofrecer otra
/// salida en vez de que el boton no haga nada.
pub fn hay_portapapeles() -> bool {
    web_sys::window()
        .and_then(|w| js_sys::Reflect::get(&w.navigator(), &JsValue::from_str("clipboard")).ok())
        .map(|c| !c.is_undefined() && !c.is_null())
        .unwrap_or(false)
}

/// Copia texto al portapapeles y avisa de si pudo. Requiere hay_portapapeles().
pub fn copiar_al_portapapeles<F: Fn(bool) + 'static>(texto: &str, avisar: F) {
    let Some(w) = web_sys::window() else {
        avisar(false);
        return;
    };
    let promesa = w.navigator().clipboard().write_text(texto);
    wasm_bindgen_futures::spawn_local(async move {
        avisar(wasm_bindgen_futures::JsFuture::from(promesa).await.is_ok());
    });
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
