# Instalación del MCP de `diario-ia` en los clientes de IA

`diario-ia` se expone a los agentes de dos maneras: por **MCP sobre stdio** (el
subcomando `diario mcp`) y por **REST**. Este documento cubre la primera: cómo registrar
el servidor MCP en cada cliente.

La pieza clave es que `diario mcp` **no lleva base de datos**: es un proceso puente que
traduce las llamadas MCP del agente a peticiones REST contra el servidor central
(`diario serve`). Por eso la configuración es la misma en todos los clientes —un
ejecutable, el argumento `mcp` y tres variables de entorno— y lo único que cambia es la
sintaxis del fichero.

```
Agente ──stdio──▶ diario mcp ──REST──▶ diario serve ──▶ diario.db
(cliente)         (por persona)        (uno, central)
```

---

## Antes de configurar nada

### 1. El binario

```powershell
cd <repo>\diario-ia
just build                      # compila la SPA y el servidor
# Solo para el binario del servidor/MCP: cargo build --release -p diario-server
```

Queda en `target\release\diario.exe`. Es un único ejecutable estático —SQLite va
*bundled*—, así que se puede copiar donde interese; lo único que importa es que la ruta
que pongas en la configuración sea la ruta real y **absoluta**.

> Un MCP que apunta a un binario que ya no está no da error visible: el cliente
> simplemente no ofrece las herramientas. Si el agente dice que no tiene `log_task`, lo
> primero que hay que comprobar es que esa ruta existe.

### 2. El servidor central

```powershell
.\diario.exe serve              # escucha en 0.0.0.0:8787
```

Tiene que estar arrancado para que `log_task` funcione. Si solo lo usas tú, vale con
`http://127.0.0.1:8787`; si lo comparte el equipo, despliégalo en un host y usa esa URL
en todos los clientes.

### 3. La API key

El servidor arranca en **modo bootstrap**: mientras no exista ninguna key, la escritura
está abierta. En cuanto se crea la primera, **todas** las escrituras pasan a exigir
`Authorization: Bearer`.

```powershell
.\diario.exe key create "claude-code-<nombre>"   # muestra el token UNA sola vez
.\diario.exe key list
.\diario.exe key revoke <id>
```

Consecuencia práctica: el momento de crear la primera key es el momento de configurar
**todos** los clientes a la vez. Si creas la key y dejas un cliente sin `DIARIO_KEY`,
ese cliente empieza a recibir `401` sin avisar de nada.

`key create` no habla con el servidor por HTTP: abre el fichero SQLite directamente
(`--db`, por defecto `diario.db` **relativo al directorio actual**). Ejecútalo desde la
carpeta del servidor, o pasa `--db` con la ruta completa, o acabarás creando la key en
una base de datos nueva y vacía.

### 4. Los tres valores

| Variable       | Qué es                                | Ejemplo                 |
|----------------|---------------------------------------|-------------------------|
| `DIARIO_URL`   | Servidor central al que reenviar      | `http://127.0.0.1:8787` |
| `DIARIO_KEY`   | API key personal (`dk_…`)             | `dk_tu_token`           |
| `DIARIO_AGENT` | Qué agente escribe; sale en el diario | `claude-code`, `codex`  |

`DIARIO_AGENT` es lo que después permite preguntar *"¿esto lo hizo Claude Code o
Codex?"*. Pon un valor distinto en cada cliente aunque compartan la key.

### Dónde NO va la key

La key identifica a una persona. **Nunca** en un fichero versionado.

| Sitio                                                                      | ¿Va la key?                                                      |
|----------------------------------------------------------------------------|------------------------------------------------------------------|
| Configuración global del usuario (`~/.claude.json`, `~/.codex/config.toml`) | Sí                                                               |
| `%USERPROFILE%\.mcp.json` (global de Visual Studio)                         | Sí                                                               |
| `.mcp.json` / `.vscode/mcp.json` dentro de un repo                          | **No** — usa `inputs` (ver VS Code) o déjala en ámbito de usuario |

---

## Claude Code

Tres ámbitos, y la diferencia importa:

| Ámbito    | Fichero                                | Para qué                                                |
|-----------|----------------------------------------|---------------------------------------------------------|
| `local`   | `~/.claude.json`, bajo el proyecto actual | Solo tú, solo este proyecto                          |
| `project` | `.mcp.json` en la raíz del proyecto    | Todo el equipo — **se commitea**, así que sin key       |
| `user`    | `~/.claude.json`, nivel superior       | Solo tú, **todos** los proyectos                        |

Para el diario lo natural es **`user`**: quieres registrar tareas desde cualquier
repositorio, y así la key se queda fuera de todos ellos.

```powershell
claude mcp add --scope user `
  --env DIARIO_URL=http://127.0.0.1:8787 `
  --env DIARIO_KEY=dk_tu_token `
  --env DIARIO_AGENT=claude-code `
  --transport stdio diario -- C:/ruta/a/diario.exe mcp
```

El `--` no es opcional: separa las opciones de `claude` del comando que arranca el
servidor. Todo lo que va detrás es la línea de órdenes del MCP.

O a mano, en `~/.claude.json` (nivel superior, **no** dentro de `projects`):

```json
{
  "mcpServers": {
    "diario": {
      "type": "stdio",
      "command": "C:/ruta/a/diario.exe",
      "args": ["mcp"],
      "env": {
        "DIARIO_URL": "http://127.0.0.1:8787",
        "DIARIO_KEY": "dk_tu_token",
        "DIARIO_AGENT": "claude-code"
      }
    }
  }
}
```

Comprobar: `claude mcp list`, o `/mcp` dentro de una sesión. Hay que **reiniciar** la
sesión para que cargue.

---

## Codex CLI

Fichero global `~/.codex/config.toml`. TOML, no JSON:

```toml
[mcp_servers.diario]
command = 'C:\ruta\a\diario.exe'
args = ["mcp"]

[mcp_servers.diario.env]
DIARIO_URL = "http://127.0.0.1:8787"
DIARIO_KEY = "dk_tu_token"
DIARIO_AGENT = "codex"
```

Dos detalles que se pagan caros:

- La ruta va en **comillas simples**. En TOML las comillas simples son *literal
  strings*: no interpretan `\` como escape. Con comillas dobles, `C:\Users\…` intenta
  leer `\U` como un escape Unicode y el fichero entero deja de parsear.
- Las tablas `[mcp_servers.*]` van **al final** del fichero. Cualquier clave de nivel
  superior (`model`, `approvals_reviewer`…) que quede por debajo de una cabecera de
  tabla se interpreta como parte de esa tabla.

Comprobar el parseo antes de arrancar Codex:

```powershell
python -c "import tomllib; print(tomllib.load(open(r'$env:USERPROFILE\.codex\config.toml','rb'))['mcp_servers'])"
```

---

## GitHub Copilot — VS Code

Copilot usa MCP en **modo agente** (Agent mode). La clave del JSON es `servers`, no
`mcpServers`.

Por interfaz: `Ctrl+Shift+P` → **MCP: Add Server** → *Command (stdio)* → comando,
argumentos, ID del servidor, y ámbito **Global** (configuración de usuario) o
**Workspace** (`.vscode/mcp.json`).

A mano, en `.vscode/mcp.json` del proyecto:

```json
{
  "servers": {
    "diario": {
      "type": "stdio",
      "command": "C:/ruta/a/diario.exe",
      "args": ["mcp"],
      "env": {
        "DIARIO_URL": "http://127.0.0.1:8787",
        "DIARIO_AGENT": "copilot"
      }
    }
  }
}
```

Fíjate en que ahí **no** está `DIARIO_KEY`: `.vscode/mcp.json` suele acabar en el
repositorio. Para eso VS Code tiene `inputs`, que pide el valor una vez y lo guarda en
el almacén de secretos del editor:

```json
{
  "inputs": [
    {
      "type": "promptString",
      "id": "diario-key",
      "description": "API key personal del diario (dk_...)",
      "password": true
    }
  ],
  "servers": {
    "diario": {
      "type": "stdio",
      "command": "C:/ruta/a/diario.exe",
      "args": ["mcp"],
      "env": {
        "DIARIO_URL": "http://127.0.0.1:8787",
        "DIARIO_KEY": "${input:diario-key}",
        "DIARIO_AGENT": "copilot"
      }
    }
  }
}
```

Este es el patrón que hace que un `mcp.json` **se pueda commitear**: el fichero lleva la
forma, y cada persona pone su secreto. Si prefieres no tocar el repositorio, pon la
configuración en el `mcp.json` global del usuario y ahí sí la key directa.

Comprobar: abre Copilot Chat, cambia a **Agent**, pulsa el icono de herramientas y busca
`diario`. Las herramientas de un servidor nuevo llegan **desactivadas**: hay que
marcarlas.

---

## GitHub Copilot — Visual Studio 2022

Requiere **17.14 o posterior**. Mismo formato que VS Code (`servers` + `inputs`), y
además Visual Studio descubre configuraciones de otros editores. Las lee en este orden:

| Orden | Ruta                        | Ámbito                                     |
|-------|-----------------------------|--------------------------------------------|
| 1     | `%USERPROFILE%\.mcp.json`   | Global del usuario — **aquí va la key**    |
| 2     | `<SOLUCION>\.vs\mcp.json`   | Esa solución, solo tú                      |
| 3     | `<SOLUCION>\.mcp.json`      | Esa solución, versionable                  |
| 4     | `<SOLUCION>\.vscode\mcp.json` | Compartido con VS Code                   |
| 5     | `<SOLUCION>\.cursor\mcp.json` | Compartido con Cursor                    |

Ojo al punto inicial: unas ubicaciones piden `.mcp.json` y otras `mcp.json`.

Para el diario, lo suyo es el global:

```json
{
  "inputs": [],
  "servers": {
    "diario": {
      "type": "stdio",
      "command": "C:/ruta/a/diario.exe",
      "args": ["mcp"],
      "env": {
        "DIARIO_URL": "http://127.0.0.1:8787",
        "DIARIO_KEY": "dk_tu_token",
        "DIARIO_AGENT": "copilot-vs"
      }
    }
  }
}
```

También por interfaz: icono de Copilot → **Open Chat Window** → llave inglesa (*Select
Tools*) → **+** → *Add Custom MCP Server*, eligiendo destino **Solution** o **Global**.

Al guardar con sintaxis válida, el agente se reinicia y recarga los servidores. Si
editas la definición de un servidor, Visual Studio lo mata y lo vuelve a arrancar.

---

## GitHub Copilot CLI

Fichero `~/.copilot/mcp-config.json`. Aquí la clave vuelve a ser `mcpServers`:

```json
{
  "mcpServers": {
    "diario": {
      "command": "C:/ruta/a/diario.exe",
      "args": ["mcp"],
      "env": {
        "DIARIO_URL": "http://127.0.0.1:8787",
        "DIARIO_KEY": "dk_tu_token",
        "DIARIO_AGENT": "copilot-cli"
      }
    }
  }
}
```

También se puede añadir con el comando `/mcp add` dentro de una sesión interactiva.

---

## GitHub Copilot coding agent

El agente que trabaja solo sobre un repositorio en github.com. Se configura en
**Settings → Copilot → Coding agent → Model Context Protocol (MCP)**.

Aquí hay una diferencia de fondo: ese agente se ejecuta **en un runner de GitHub
Actions**, no en tu máquina. Un `diario.exe` local no existe allí, y
`http://127.0.0.1:8787` apuntaría al propio runner. Para que tenga sentido hacen falta
tres cosas:

1. El servidor del diario accesible desde el runner (es decir, publicado).
2. El binario instalado en el runner, vía `.github/workflows/copilot-setup-steps.yml`.
3. La key como secreto del repositorio, nunca en el JSON.

```json
{
  "mcpServers": {
    "diario": {
      "type": "local",
      "command": "/ruta/en/el/runner/diario",
      "args": ["mcp"],
      "tools": ["*"],
      "env": {
        "DIARIO_URL": "https://diario.tu-dominio/",
        "DIARIO_AGENT": "copilot-coding-agent"
      }
    }
  }
}
```

Si no vas a exponer el servidor, es bastante más sencillo que ese agente registre por
**REST** desde un paso del workflow (más abajo) que montar el MCP.

---

## Claude Desktop

`%APPDATA%\Claude\claude_desktop_config.json` en Windows,
`~/Library/Application Support/Claude/claude_desktop_config.json` en macOS:

```json
{
  "mcpServers": {
    "diario": {
      "command": "C:/ruta/a/diario.exe",
      "args": ["mcp"],
      "env": {
        "DIARIO_URL": "http://127.0.0.1:8787",
        "DIARIO_KEY": "dk_tu_token",
        "DIARIO_AGENT": "claude-desktop"
      }
    }
  }
}
```

Hay que cerrar y volver a abrir la aplicación: no basta con cerrar la ventana, porque en
Windows queda en la bandeja del sistema.

---

## Cursor

`~/.cursor/mcp.json` (global) o `.cursor/mcp.json` (proyecto). Clave `mcpServers`:

```json
{
  "mcpServers": {
    "diario": {
      "command": "C:/ruta/a/diario.exe",
      "args": ["mcp"],
      "env": {
        "DIARIO_URL": "http://127.0.0.1:8787",
        "DIARIO_KEY": "dk_tu_token",
        "DIARIO_AGENT": "cursor"
      }
    }
  }
}
```

Reinicia Cursor después de guardar.

---

## Resumen: el mismo servidor, tres sintaxis

| Cliente                  | Fichero                                       | Clave raíz                  | Ámbito recomendado           |
|--------------------------|-----------------------------------------------|-----------------------------|------------------------------|
| Claude Code              | `~/.claude.json`                              | `mcpServers`                | `--scope user`               |
| Codex CLI                | `~/.codex/config.toml`                        | `[mcp_servers.diario]`      | global                       |
| Copilot / VS Code        | `.vscode/mcp.json` o el global                | `servers`                   | global, o repo con `inputs`  |
| Copilot / Visual Studio  | `%USERPROFILE%\.mcp.json`                     | `servers`                   | global                       |
| Copilot CLI              | `~/.copilot/mcp-config.json`                  | `mcpServers`                | global                       |
| Copilot coding agent     | Ajustes del repositorio en github.com         | `mcpServers` (`type: local`)| repositorio                  |
| Claude Desktop           | `%APPDATA%\Claude\claude_desktop_config.json` | `mcpServers`                | global                       |
| Cursor                   | `~/.cursor/mcp.json`                          | `mcpServers`                | global                       |

Son tres familias: `mcpServers` (Anthropic y derivados), `servers` (Microsoft) y TOML
(Codex). El contenido —comando, `args: ["mcp"]` y las tres variables— no cambia.

---

## Agentes sin MCP: REST

Cualquier cosa que sepa hacer un POST puede escribir en el diario. Es la vía para
scripts de CI, agentes propios o clientes sin soporte MCP:

```bash
curl -X POST http://127.0.0.1:8787/api/v1/entries \
  -H 'Authorization: Bearer dk_tu_token' \
  -H 'content-type: application/json' \
  -d '{
    "application": "uaRedesIce",
    "agent": "script-ci",
    "title": "Publicación 2.4.1 en preproducción",
    "prompt": "…",
    "task_summary": "…",
    "response_markdown": "# Hecho\n…"
  }'
```

---

## Las herramientas que verás

| Herramienta                                                                            | Para qué                      |
|----------------------------------------------------------------------------------------|-------------------------------|
| `log_task(application, title, prompt, response_markdown, task_summary?, model?, tags?)` | Registrar una tarea terminada |
| `list_applications()`                                                                   | Aplicaciones con entradas     |
| `list_entries(application?, from?, to?, limit?)`                                        | Explorar el diario            |
| `get_entry(id)`                                                                         | Recuperar una entrada concreta|

`application` es la unidad de agrupación y tiene que ser **el mismo nombre siempre**.
`uaRedesIce`, `uaredesice` y `RedesICE` son tres aplicaciones distintas para el diario,
y una vez dispersas ya no se juntan solas.

<!-- PENDIENTE: fijar aquí la lista canónica de nombres de `application`, para que
     ningún agente se los invente. Propuesta de partida según los repositorios:

     | Repositorio                     | `application` |
     |---------------------------------|---------------|
     | redesice-netcore / redesice-mvc | uaRedesIce    |
     | matricula2-netcore              | uaMatricula   |
     | accesibilidad-netcore           | Accesibilidad |
     | directorio-netcore              | Directorio    |
     | otro-noticias-netcore           | OtriNoticias  |
     | componentes/vue/uacloud2026     | uaCloud2026   |

     Decisión abierta: ¿`-mvc` y `-netcore` comparten nombre (historia única de la
     aplicación, que es lo que interesa durante una migración) o van separados
     (historia por base de código)? -->

---

## Que el agente registre solo

Configurar el MCP solo pone la herramienta a mano; no hace que se use. Lo que convierte
el registro en automático es una instrucción en el `CLAUDE.md` / `AGENTS.md` del
proyecto:

```markdown
## Registro de tareas (obligatorio)
Al terminar cada tarea, registra en el MCP `diario` (herramienta log_task):
application "<nombre-de-la-aplicacion>", el prompt original literal, un resumen de
2-3 líneas y la respuesta completa en markdown.
```

Por eso conviene que el servidor se llame **`diario`** en todos los clientes: así esa
instrucción, escrita una vez, vale igual para Claude Code, Codex y Copilot.

---

## Comprobar que funciona

**Que el servidor responde:**

```bash
curl -s -o /dev/null -w "HTTP %{http_code}\n" http://127.0.0.1:8787/api/v1/applications
```

**Que la autenticación está como esperas.** Con keys creadas, sin token debe dar `401`:

```bash
curl -s -o /dev/null -w "%{http_code}\n" -X POST http://127.0.0.1:8787/api/v1/entries \
  -H 'content-type: application/json' -d '{}'
```

**Que el MCP habla**, sin depender de ningún cliente. Un `initialize` seguido de un
`tools/list` por stdio tiene que devolver las cuatro herramientas:

```bash
printf '%s\n' \
 '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"t","version":"1"}}}' \
 '{"jsonrpc":"2.0","method":"notifications/initialized"}' \
 '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}' \
 | DIARIO_URL=http://127.0.0.1:8787 DIARIO_KEY=dk_tu_token ./diario mcp
```

Si esto funciona y el cliente sigue sin ver las herramientas, el problema está en la
configuración del cliente, no en el diario.

---

## Problemas frecuentes

| Síntoma                                  | Causa habitual                                                           |
|------------------------------------------|--------------------------------------------------------------------------|
| El cliente no ofrece `log_task`          | Ruta del ejecutable equivocada, o falta reiniciar el cliente             |
| `401` al registrar                       | Existe alguna key y este cliente no tiene `DIARIO_KEY`, o está revocada  |
| Error de conexión                        | `diario serve` no está arrancado, o `DIARIO_URL` apunta a otro sitio     |
| Codex no arranca                         | Ruta Windows con `\` entre comillas dobles en el TOML — usa simples      |
| VS Code no ve el servidor                | Está en modo *Ask*/*Edit* en vez de **Agent**, o las herramientas están desactivadas |
| Todo va bien pero el diario está vacío   | Falta la instrucción de registro en el `CLAUDE.md` del proyecto          |
| Las entradas salen repartidas            | `application` escrito distinto cada vez                                  |

---

## Referencias

- `README.md` — arquitectura, API REST y compilación.
- [MCP en Visual Studio](https://learn.microsoft.com/visualstudio/ide/mcp-servers)
- [MCP en VS Code](https://code.visualstudio.com/docs/copilot/chat/mcp-servers)
- [MCP en Claude Code](https://docs.claude.com/en/docs/claude-code/mcp)
