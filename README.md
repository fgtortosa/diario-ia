# Diario-IA

Diario de tareas de agentes de IA. Los agentes (Claude Code y otros) registran
cada tarea que realizan —**por aplicación**, con su **prompt**, un **resumen** y la
**respuesta o documento markdown** (con diagramas)— y las personas del equipo lo
exploran por aplicación y por fechas desde una interfaz web.

Todo el stack es **Rust**. El resultado es **un único ejecutable estático**, sin
runtime (nada de .NET/Node/JVM), que sirve la API REST, el servidor MCP, la base
de datos SQLite embebida y la SPA WebAssembly.

## Arquitectura

```
Agentes IA ──(MCP stdio / REST)──▶ ┌─────────── binario `diario` ───────────┐
                                   │  Axum HTTP  ─ REST /api/v1              │
Navegador  ──(HTTP)──────────────▶ │             ─ SPA Leptos/WASM embebida │
                                   │  MCP (rmcp) ─ log_task, list, get…     │
                                   │  Storage    ─ SQLite (bundled, WAL)    │
                                   └────────────────────────────────────────┘
                                                    │  diario.db (1 fichero)
```

- **Servidor** (`crates/server`): Axum + SQLite (rusqlite *bundled*, sin librería
  del sistema) + servidor MCP (rmcp) + SPA embebida (rust-embed).
- **Cliente** (`crates/client`): SPA en Leptos compilada a WASM; markdown
  renderizado en el servidor (comrak + sanitizado con ammonia) y diagramas
  **mermaid** + resaltado de código (**highlight.js**) en el navegador.
- **Compartido** (`crates/shared`): DTOs serde comunes a servidor y cliente.

## Requisitos de desarrollo

```bash
# Rust estable + target WASM + bundler de la SPA
rustup target add wasm32-unknown-unknown
cargo install trunk --locked
# (opcional) cross-compilar: cargo install cargo-zigbuild && brew install zig
cargo install just    # atajos del justfile (opcional)
```

## Arrancar en local

```bash
just build      # compila la SPA y el binario con la SPA embebida
./target/release/diario serve
# Abre http://localhost:8787
```

Variables de entorno del servidor:

| Variable              | Por defecto                | Descripción                          |
|-----------------------|----------------------------|--------------------------------------|
| `DIARIO_BIND`         | `0.0.0.0:8787`             | Dirección de escucha                 |
| `DIARIO_DB`           | `diario.db`               | Fichero SQLite                       |
| `DIARIO_PUBLIC_URL`   | `http://localhost:8787`   | URL base para los enlaces a entradas |
| `DIARIO_VIEWER_TOKEN` | (vacío)                    | Si se define, exige token de lectura |

Desarrollo con recarga en caliente de la SPA (proxya la API al servidor central):

```bash
just dev          # terminal 1: servidor central en :8787
just dev-client   # terminal 2: trunk serve (SPA en :8080 con proxy)
```

## Autenticación por API key

El servidor arranca en modo *bootstrap*: mientras no exista ninguna API key, la
escritura está abierta (útil para empezar). En cuanto creas la primera key, la
escritura pasa a exigir `Authorization: Bearer <token>`.

```bash
./target/release/diario key create "claude-code-pedro"   # muestra el token una vez
./target/release/diario key list
./target/release/diario key revoke 1
```

## Integración con agentes

### MCP (recomendado para Claude Code)

Cada persona registra el binario como servidor MCP por stdio, apuntando al
servidor central. `.mcp.json` del proyecto (o config global de Claude Code):

```json
{
  "mcpServers": {
    "diario": {
      "command": "/ruta/a/diario",
      "args": ["mcp"],
      "env": {
        "DIARIO_URL": "http://servidor-central:8787",
        "DIARIO_KEY": "dk_tu_token",
        "DIARIO_AGENT": "claude-code"
      }
    }
  }
}
```

Herramientas expuestas al agente:

- `log_task(application, title, prompt, response_markdown, task_summary?, model?, tags?)`
- `list_applications()`
- `list_entries(application?, from?, to?, limit?)`
- `get_entry(id)`

### REST (cualquier agente)

```bash
curl -X POST http://servidor:8787/api/v1/entries \
  -H 'Authorization: Bearer dk_tu_token' \
  -H 'content-type: application/json' \
  -d '{
    "application": "Portal Alumnos",
    "agent": "claude-code",
    "title": "Refactor del login CAS",
    "prompt": "Refactoriza el login…",
    "task_summary": "Migrado a AuthorizationPolicies",
    "response_markdown": "# Hecho\n\n```mermaid\ngraph TD; A-->B;\n```",
    "tags": ["auth", "refactor"]
  }'
```

| Método | Ruta                          | Descripción                                  |
|--------|-------------------------------|----------------------------------------------|
| POST   | `/api/v1/entries`             | Crea una entrada (auth de escritura)         |
| GET    | `/api/v1/applications`        | Aplicaciones con contadores                  |
| GET    | `/api/v1/entries`             | Lista filtrable: `application,from,to,tag,q,limit,cursor` |
| GET    | `/api/v1/entries/{id}`        | Entrada completa (markdown + html + adjuntos)|
| GET    | `/api/v1/stats`               | Recuentos por día (heatmap)                  |

## Compilación multiplataforma (sin runtime)

```bash
just build           # binario nativo (macOS/Linux/Windows del host)
just build-linux     # x86_64-unknown-linux-musl (estático, requiere zig)
just build-windows   # x86_64-pc-windows-gnu (.exe, requiere zig)
```

SQLite se compila *bundled* dentro del binario, así que no hay dependencias de
sistema: copia el ejecutable y ejecútalo.

## Tests

```bash
just test    # storage (SQLite en memoria) + API (oneshot) + MCP (proxy real)
```
# diario-ia
