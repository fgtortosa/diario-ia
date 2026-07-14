# Diario-IA — Diario de tareas de agentes de IA

## Context

El equipo usa agentes de IA (Claude Code y otros) para trabajar sobre varias
aplicaciones. Hoy no queda registro estructurado de **qué hizo cada agente, para
qué aplicación, con qué prompt y con qué resultado**. El objetivo es un producto
interno donde los agentes registren cada tarea (prompt + resumen de tarea +
respuesta/documento markdown) y donde las personas puedan **explorar por
aplicación y por fechas**, y **visualizar los documentos markdown con diagramas**.

Requisitos duros del usuario: ejecutable **rápido**, compilable en **Linux y
Windows**, **sin runtime** (nada de .NET/Node/JVM/Python en producción → binario
estático). El directorio está vacío: es un proyecto nuevo (greenfield).

Decisiones tomadas con el usuario:
- **Cliente**: SPA en Rust→WASM con **Leptos**, servida por el propio binario.
- **Integración de agentes**: **MCP** (nativo para Claude Code) **+ API REST/JSON**.
- **Almacenamiento**: **SQLite embebido** (bundled, sin servidor de BD).
- **Despliegue**: **servidor central compartido** con **auth por API key**.

Resultado: un stack 100% Rust, un único binario estático que sirve la API REST,
el servidor MCP, la BD SQLite embebida y la SPA WASM embebida.

---

## Arquitectura

```
Agentes IA ──(MCP stdio / REST)──▶ ┌─────────────── binario `diario` ───────────────┐
                                   │  Axum HTTP  ─ REST /api/v1                       │
Navegador  ──(HTTP)──────────────▶ │             ─ SPA Leptos/WASM embebida          │
                                   │  MCP (rmcp) ─ herramientas log_task, list...     │
                                   │  Storage    ─ SQLite (rusqlite bundled, WAL)     │
                                   └─────────────────────────────────────────────────┘
                                                     │  diario.db (fichero único)
```

Un solo ejecutable con subcomandos (clap):
- `diario serve` — servidor central (REST + SPA embebida). **[por defecto]**
- `diario mcp` — servidor MCP por **stdio** que actúa de proxy contra el central
  (`DIARIO_URL` + `DIARIO_KEY`). Es lo que cada desarrollador añade a la config de
  su agente (Claude Code lanza servidores MCP como subproceso stdio).
- `diario key {create|list|revoke}` — gestión de API keys.
- `diario migrate` / `diario export` — utilidades.

### Justificación del stack (todo Rust)
- **Binario estático sin runtime**: target `x86_64-unknown-linux-musl` (Linux) y
  `x86_64-pc-windows-msvc`/`-gnu` (Windows). `rusqlite` con feature `bundled`
  compila SQLite dentro del binario → cero dependencias de sistema.
- **rmcp 0.16** (SDK oficial de MCP en Rust) implementa el protocolo actual
  (2025-11-25) con transportes stdio y HTTP, y macros para definir herramientas.
- **Leptos** (reactividad fina, WASM pequeño, orientado a web) para la SPA.
- **Axum + rust-embed/axum-embed** es el patrón estándar para servir una SPA
  embebida desde un único binario, con fallback a `index.html` para rutas de cliente.

---

## Estructura del repositorio (workspace Cargo)

```
diario-ia/
├── Cargo.toml                 # workspace
├── justfile                   # orquesta build cliente→servidor y cross-compile
├── rust-toolchain.toml
├── crates/
│   ├── shared/                # DTOs serde compartidos server↔client (Entry, Application, filtros)
│   ├── server/                # Axum + REST + MCP + SQLite; embebe crates/client/dist
│   │   ├── src/main.rs        # clap: serve|mcp|key|migrate|export
│   │   ├── src/api/           # handlers REST /api/v1
│   │   ├── src/mcp/           # servidor rmcp + herramientas
│   │   ├── src/storage/       # rusqlite + pool r2d2 + migraciones + FTS5
│   │   ├── src/render.rs      # markdown→HTML (comrak) + sanitizado (ammonia)
│   │   └── src/auth.rs        # API keys (hash argon2/sha256), middleware bearer
│   └── client/                # SPA Leptos CSR (WASM), build con Trunk → dist/
│       ├── index.html         # incluye assets locales: mermaid.js, highlight.js/css
│       ├── src/               # componentes: Layout, Sidebar, Timeline, EntryDetail
│       └── assets/            # mermaid, highlight, katex (vendored, sin CDN)
└── migrations/                # *.sql embebidos
```

---

## Modelo de datos (SQLite, modo WAL)

- `application(id, slug UNIQUE, name, description, created_at)`
- `entry(id, application_id FK, agent_name, model, title, prompt, task_summary,
  response_markdown, response_html, status, created_at, tokens_input,
  tokens_output, duration_ms, metadata_json)`
- `attachment(id, entry_id FK, filename, kind, content_markdown, content_html)`
  — para "documentos markdown" adicionales por tarea.
- `entry_tag(entry_id FK, tag)` — filtrado por etiqueta.
- `api_key(id, name, key_hash, active, created_at, last_used_at)` — auth.
- `entry_fts` — tabla virtual **FTS5** sobre prompt/task_summary/response para búsqueda.

Índices clave: `entry(application_id, created_at)` y `entry(created_at)` para las
consultas "por aplicación y por fechas".

`response_html`/`content_html` se calculan **al insertar** (comrak + ammonia),
dejando los bloques ` ```mermaid ` como `<pre class="mermaid">` para que mermaid.js
los renderice en el cliente. Así el WASM se mantiene pequeño y el render es consistente.

---

## API REST (`/api/v1`)

Auth: escritura requiere `Authorization: Bearer <key>` (tabla `api_key`). Lectura
(la SPA) protegida por token de visor configurable o por red/reverse-proxy.

- `POST /entries` — crea entrada. Auto-crea `application` por slug si no existe.
  Body: `{application, agent, model?, title, prompt, task_summary,
  response_markdown, tags?, attachments?, tokens?, duration_ms?, metadata?}`.
- `GET /applications` — apps con nº de entradas y última actividad.
- `GET /entries?application=&from=&to=&tag=&q=&limit=&cursor=` — lista paginada (resúmenes).
- `GET /entries/{id}` — entrada completa (markdown + html + attachments).
- `GET /stats?application=&from=&to=` — recuentos por día (calendario/heatmap por app).
- `GET /healthz`; resto de rutas → SPA embebida (fallback `index.html`).

## Servidor MCP (rmcp, transporte stdio)

Herramientas expuestas al agente:
- `log_task(application, title, prompt, task_summary, response_markdown, tags?,
  model?, metadata?)` → crea entrada; devuelve `id` + URL de visualización.
- `list_applications()` / `list_entries(application?, from?, to?)` /
  `get_entry(id)` → permiten al agente **recuperar** su propio historial.

`diario mcp` reenvía estas llamadas al servidor central vía REST usando
`DIARIO_URL` + `DIARIO_KEY`. Cada persona lo registra una vez en su agente.

---

## Cliente SPA (Leptos CSR → WASM)

- **Layout**: sidebar con lista de aplicaciones (+contadores), selector de rango de
  fechas / mini-calendario tipo heatmap, filtro por etiqueta y búsqueda (FTS).
- **Timeline**: entradas agrupadas por fecha (y por app si no hay filtro). Cada
  tarjeta: badge de app, agente/modelo, título, hora, tags, snippet.
- **Detalle**: prompt (colapsable), resumen de tarea, y **markdown renderizado**
  (HTML pre-generado en servidor) + attachments. Tras montar el HTML se invoca
  `mermaid.run()` (interop JS vía wasm-bindgen) para los diagramas; `highlight.js`
  para código; opcional KaTeX para fórmulas. Todos los assets JS/CSS **vendored**
  y embebidos (sin CDN → funciona offline/aislado).
- Datos vía `gloo-net` (fetch) contra `/api/v1`.

---

## Build y cross-compilación (sin runtime)

`justfile`:
1. `trunk build --release` en `crates/client` → `crates/client/dist`.
2. `cargo build --release -p server` — `rust-embed` embebe `dist/` en tiempo de compilación.
3. Targets:
   - Linux estático: `x86_64-unknown-linux-musl` (con `rusqlite` bundled).
   - Windows: `x86_64-pc-windows-msvc` (nativo) o cross desde Linux con
     **`cargo-zigbuild`** (`x86_64-pc-windows-gnu`).
- CI GitHub Actions (matriz linux-musl + windows) publicando los binarios como artefactos de release.

## Dependencias principales

- **server**: `axum`, `tokio`, `tower-http` (cors, trace, compression, fallback),
  `rusqlite` (bundled) + `r2d2_sqlite`, `serde`/`serde_json`, `comrak`, `ammonia`,
  `rmcp`, `clap`, `tracing`+`tracing-subscriber`, `thiserror`/`anyhow`,
  `rust-embed`/`axum-embed`, `time`, `argon2` o `sha2`, `uuid`.
- **shared**: `serde`, `time`.
- **client**: `leptos`, `leptos_router`, `gloo-net`, `wasm-bindgen`, `web-sys`,
  `js-sys`, `serde`; build con **Trunk**.

DB async: `rusqlite` es síncrono → operaciones en `tokio::task::spawn_blocking`
sobre un pool r2d2; SQLite en modo **WAL** para lectores concurrentes.

---

## Roadmap de implementación (por fases)

1. **Andamiaje**: workspace, `shared` DTOs, `storage` con migraciones + modelo +
   pool + WAL, tests unitarios de storage con SQLite en memoria.
2. **REST + auth**: handlers `/api/v1`, middleware de API key, render markdown→HTML
   (comrak+ammonia) al insertar, tests de integración con `oneshot`.
3. **MCP**: servidor rmcp con `log_task`/`list_*`/`get_entry`; modo `diario mcp`
   proxy stdio→REST; probar el registro contra el central.
4. **SPA Leptos**: layout, sidebar (apps/fechas/tags/búsqueda), timeline, detalle
   con mermaid/highlight; assets vendored; build con Trunk.
5. **Empaquetado**: `rust-embed` de la SPA, subcomando `key`, `justfile`,
   cross-compile musl + windows, CI de releases.
6. **Pulido**: heatmap por app (`/stats`), export a markdown/JSON, README con guía
   de integración MCP para Claude Code.

---

## Verificación (end-to-end)

- **Storage/REST**: `cargo test` (unitarios de storage + integración de API con
  `tower::ServiceExt::oneshot`, incluyendo auth 401/200 y filtros app/fecha).
- **Flujo real de agente**: arrancar `diario serve`; crear una API key; hacer
  `POST /api/v1/entries` con `curl` (con y sin Bearer) y verificar consulta por
  `application` + `from/to`; comprobar que el markdown con un bloque ```mermaid```
  devuelve `response_html` con `<pre class="mermaid">`.
- **MCP**: registrar `diario mcp` como servidor MCP en Claude Code y ejecutar
  `log_task`; confirmar que la entrada aparece en el central y en la SPA.
- **SPA**: `trunk serve` en dev; validar navegación por app/fechas, render de
  markdown y **renderizado de diagramas mermaid** en el detalle.
- **Binario sin runtime**: `cargo build --release --target x86_64-unknown-linux-musl`,
  ejecutar el binario en un contenedor mínimo (p.ej. `scratch`/`alpine`) sin
  dependencias; repetir el smoke test REST. Verificar también el `.exe` de Windows.

---

## Notas / alternativas descartadas

- **Go**: buen binario estático, pero peor historia de GUI/WASM y no permite
  compartir tipos con el cliente. Rust cubre servidor + cliente WASM con un modelo
  de datos único.
- **.NET / Vue (stack habitual UA)**: descartado explícitamente por el requisito
  "sin runtime" para este producto.
- **Leptos SSR (cargo-leptos)**: integraría todo en un binario, pero mezcla las
  server-functions con el servidor MCP/REST. Se opta por **CSR + Axum** para separar
  limpiamente cliente (WASM) y servidor (REST+MCP+DB), acorde al modelo mental del usuario.
- **Dioxus**: alternativa válida (modelo tipo React, más cercano a Vue); se elige
  Leptos por WASM más pequeño y mejor rendimiento web. Fácil de reconsiderar en fase 4.
