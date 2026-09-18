# Exportación manual, marcado, etiquetas y modo log — Plan de implementación

**Goal:** Que desde la web se pueda descargar una entrada en md y en PDF, ver y cambiar su estado de exportación, filtrar por etiqueta desde una lista con recuentos, y seguir por consola cada operación del diario.

**Architecture:** Cuatro bloques independientes sobre lo ya construido. El estado de exportación (`exported_at`) gana escritura manual por API; las etiquetas ganan un endpoint de recuento; el modo log es una bandera que instrumenta REST y MCP en el mismo sitio; y la descarga se resuelve en el cliente sin dependencias nuevas.

**Tech Stack:** Rust (axum, rusqlite, tokio), Leptos/WASM, CSS de impresión.

## Decisiones tomadas de antemano

Van aquí porque condicionan las tareas y porque son las que un revisor querría discutir.

**PDF con `window.print()` y hoja de impresión, no con una librería.** Meter un generador de PDF en el binario significa o bien `printpdf` —que obliga a maquetar a mano y no entiende el HTML ya renderizado— o bien un Chrome headless, que multiplica por diez el tamaño de un ejecutable que hoy son 10 MB y presume de no tener dependencias de sistema. El navegador ya sabe imprimir a PDF, respeta `@media print` y da un resultado mejor que cualquier maquetación manual. Lo que sí hace falta es una hoja de impresión de verdad: sin barra lateral, sin cabecera, sin botones, con el markdown a ancho completo.

**Descarga del `.md` en el cliente, sin endpoint nuevo.** El detalle ya tiene el markdown de la entrada. Un `Blob` y un `<a download>` bastan; añadir una ruta al servidor sería duplicar lo que ya viaja.

**El nombre del fichero descargado sigue el mismo esquema que el exportado** (`AAAAMMDD-HHMMSS-aplicacion-id.md`), para que descargar a mano y exportar automáticamente produzcan lo mismo.

**La hora de los nombres pasa de UTC a local.** Era el punto abierto: los ficheros decían `181303` para algo de las 20:13, mientras el resto del workspace va en hora local. Dentro de un mismo día el desfase es constante, así que el orden alfabético sigue siendo el cronológico y el cambio es seguro. Las entradas ya exportadas no se reescriben.

**Marcar a mano tiene consecuencias y se asumen explícitamente:** marcar como *no exportada* hace que el exportador la vuelva a escribir en la siguiente pasada —salvo que el fichero exista, en cuyo caso lo respeta—, y marcar como *exportada* sin que el fichero exista la deja sin escribir para siempre. Es la herramienta que se pide; lo que corresponde es que la interfaz lo diga, no impedirlo.

## File Structure

| Fichero | Responsabilidad |
|---|---|
| `crates/shared/src/lib.rs` | `TagCount`; `EntryQuery.exported`; `Entry.exported_at` |
| `crates/server/src/storage.rs` | `contar_etiquetas`, `marcar_exportacion_manual`, filtro `exported` en `query_entries`, `exported_at` en `get_entry` |
| `crates/server/src/api.rs` | `GET /api/v1/tags`, `PUT /api/v1/entries/{id}/exported`, traza de operaciones |
| `crates/server/src/mcp.rs` | Traza de operaciones |
| `crates/server/src/config.rs` | `log_ops` |
| `crates/server/src/exporter.rs` | Hora local en el nombre |
| `crates/client/src/api.rs` | `fetch_tags`, `set_exported`, `EntryFilters.exported` |
| `crates/client/src/app.rs` | Lista de etiquetas, filtro de no exportados, botones de descarga y marcado |
| `crates/client/src/ffi.rs` | `descargar_fichero`, `imprimir` |
| `crates/client/styles.css` | `@media print` |

---

### Tarea 1 — Hora local en el nombre del fichero

- [ ] Cambiar `nombre_fichero` para formatear `created_at` en hora local.
- [ ] Ajustar el test del nombre: la entrada de prueba es UTC `12:58:51`; el nombre esperado pasa a depender de la zona, así que el test compara contra el mismo cálculo (`with_timezone(&Local)`) en vez de contra una cadena fija.
- [ ] `cargo test`, commit.

### Tarea 2 — Etiquetas con recuento

- [ ] `TagCount { tag: String, count: i64 }` en `shared`.
- [ ] `Store::contar_etiquetas()` agrupando por `entry_tag`.
- [ ] Test: tres entradas con etiquetas solapadas dan los recuentos correctos, ordenados por nombre.
- [ ] `GET /api/v1/tags` en `api.rs`.
- [ ] `cargo test`, commit.

### Tarea 3 — Marcado manual de exportación

- [ ] `Store::marcar_exportacion_manual(id, exportada: bool)`: pone `exported_at` a ahora o a `NULL`. Devuelve `false` si la entrada no existe.
- [ ] `EntryQuery.exported: Option<bool>` y su filtro en `query_entries`.
- [ ] `Entry.exported_at: Option<DateTime<Utc>>` y leerlo en `get_entry`.
- [ ] Tests: marcar y desmarcar cambian la cola de pendientes; el filtro devuelve lo esperado en los tres casos (`Some(true)`, `Some(false)`, `None`).
- [ ] `PUT /api/v1/entries/{id}/exported` con cuerpo `{"exported": bool}`, protegido por la misma autenticación de escritura.
- [ ] `cargo test`, commit.

### Tarea 4 — Modo log de operaciones

- [ ] `ServerConfig.log_ops: bool`, de `--log-ops` / `DIARIO_LOG_OPS`.
- [ ] Una función `traza_operacion(config, operacion, detalle)` que emita `tracing::info!` solo si está activo, para que el formato sea uno solo.
- [ ] Invocarla en: crear entrada, consultar entradas, consultar una entrada, listar aplicaciones, listar etiquetas, marcar exportación, y las cuatro herramientas MCP.
- [ ] La traza dice de dónde viene (`rest` o `mcp`), la operación y lo mínimo para identificarla; nunca el prompt ni la respuesta completos.
- [ ] Test: con `log_ops` desactivado no se emite nada.
- [ ] `cargo test`, commit.

### Tarea 5 — Cliente: etiquetas, filtro de no exportados y estado

- [ ] `fetch_tags()` y `EntryFilters.exported` en `api.rs` del cliente.
- [ ] Sección «Etiquetas» en la barra lateral, bajo Aplicaciones: cada una con su recuento, misma mecánica que las aplicaciones; la activa se marca.
- [ ] Un conmutador «Solo sin exportar» junto a las fechas.
- [ ] `Limpiar filtros` limpia también etiqueta y estado de exportación.
- [ ] Compilar con `trunk build --release`, commit.

### Tarea 6 — Cliente: descargar y marcar desde el detalle

- [ ] `ffi.rs`: `descargar_fichero(nombre, contenido)` con `Blob` + `<a download>`, y `imprimir()` con `window.print()`.
- [ ] En el detalle: botón «Descargar .md», botón «PDF», y un indicador del estado de exportación con su botón para cambiarlo.
- [ ] El indicador dice la fecha si está exportada, y «sin exportar» si no.
- [ ] Al cambiar el estado se recarga la entrada para que el indicador no mienta.
- [ ] Compilar, commit.

### Tarea 7 — Hoja de impresión

- [ ] `@media print` en `styles.css`: oculta cabecera, barra lateral y botones; el contenido a ancho completo; sin sombras ni fondos; los enlaces no muestran su URL; evita cortar bloques de código a mitad.
- [ ] Compilar, commit.

### Tarea 8 — Documentación y verificación

- [ ] `INSTALACION-MCP.md`: apartado del modo log y del marcado manual, con su advertencia.
- [ ] `DIARIO.md`: entrada con las decisiones.
- [ ] Verificación con Playwright: la lista de etiquetas filtra; el filtro de no exportados responde; descargar produce fichero; marcar y desmarcar cambia el indicador y la cola.
- [ ] `cargo test` completo, commit y PR.
