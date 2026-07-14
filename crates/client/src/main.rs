//! Punto de entrada de la SPA (CSR). Monta la aplicacion Leptos en <body>.

mod api;
mod app;
mod ffi;

use leptos::mount::mount_to_body;

fn main() {
    console_error_panic_hook::set_once();
    mount_to_body(app::App);
}
