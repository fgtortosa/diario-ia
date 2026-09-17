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
.\target\release\diario.exe serve              # escucha en 0.0.0.0:8787
```

Tiene que estar arrancado para que `log_task` funcione. Si solo lo usas tú, vale con
`http://127.0.0.1:8787`; si lo comparte el equipo, despliégalo en un host y usa esa URL
en todos los clientes.

Para no tener que acordarse de arrancarlo, ver
[Arrancar el servidor al iniciar sesión](#arrancar-el-servidor-al-iniciar-sesión-windows).

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

## Arrancar el servidor al iniciar sesión (Windows)

Configurar el MCP en los clientes no sirve de nada si `diario serve` no está levantado:
`diario mcp` es solo un puente, y sin servidor al otro lado `log_task` falla. Lo natural
es que arranque con la sesión.

### Lo que queda montado

Una **tarea programada** llamada `DiarioIA`, en el perfil del usuario, disparada al
iniciar sesión:

| Ajuste | Valor | Por qué |
|---|---|---|
| Disparador | Al iniciar sesión (`AtLogOn`), del usuario actual | Arranca contigo, no con la máquina |
| Acción | `…\target\release\diario.exe serve --db "…\diario.db" --bind 127.0.0.1:8787` | Rutas absolutas, ver abajo |
| Tipo de inicio de sesión | `Interactive` | Ver abajo |
| Nivel | `Limited` | No pide elevación ni permisos de administrador |
| Límite de ejecución | Ilimitado (`PT0S`) | Por defecto son 3 días: el servidor moriría el jueves |
| Varias instancias | `IgnoreNew` | Un segundo arranque no pelea por el puerto |
| Reinicio | 3 intentos cada minuto | Si el proceso se cae, vuelve solo |
| Batería | No se detiene ni se impide el arranque | Portátiles |

### El comando que la crea

Se ejecuta **sin privilegios de administrador**. Es idempotente: `-Force` reemplaza la
tarea si ya existe, así que se puede volver a lanzar tal cual para cambiar cualquier
parámetro.

```powershell
$exe = 'C:\Users\<usuario>\codigo\herramientas\diario-ia\target\release\diario.exe'
$db  = 'C:\Users\<usuario>\codigo\herramientas\diario-ia\diario.db'
$dir = 'C:\Users\<usuario>\codigo\herramientas\diario-ia'
$usuario = "$env:USERDOMAIN\$env:USERNAME"

$accion = New-ScheduledTaskAction -Execute $exe `
  -Argument "serve --db `"$db`" --bind 127.0.0.1:8787" -WorkingDirectory $dir

$disparador = New-ScheduledTaskTrigger -AtLogOn -User $usuario

$principal = New-ScheduledTaskPrincipal -UserId $usuario -LogonType Interactive -RunLevel Limited

$ajustes = New-ScheduledTaskSettingsSet -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries `
  -StartWhenAvailable -ExecutionTimeLimit ([TimeSpan]::Zero) `
  -RestartCount 3 -RestartInterval (New-TimeSpan -Minutes 1) -MultipleInstances IgnoreNew

Register-ScheduledTask -TaskName 'DiarioIA' -Action $accion -Trigger $disparador `
  -Principal $principal -Settings $ajustes -Force `
  -Description 'Servidor central de diario-ia. Arranca al iniciar sesion.'
```

### Por qué cada decisión

**`--db` y `--bind` absolutos, no el directorio de trabajo.** El valor por defecto de
`--db` es `diario.db` **relativo al directorio actual**. Si la tarea arrancara con otro
cwd —y el cwd de una tarea programada no siempre es el que crees—, el servidor crearía
una base de datos nueva y vacía en otro sitio, arrancaría perfectamente y el diario
aparecería en blanco. Sin ningún error. Pasar la ruta completa elimina esa clase de
fallo.

**`--bind 127.0.0.1:8787`, no el `0.0.0.0` por defecto.** Un servidor que arranca solo y
se queda todo el día escuchando es otra cosa que uno que levantas a mano un rato. Con
`0.0.0.0` el diario queda accesible desde toda la red, y aunque la escritura exige API
key, **la lectura es libre** mientras `DIARIO_VIEWER_TOKEN` esté vacío: cualquiera en la
red podría leer los prompts. Si algún día lo compartes con el equipo, eso se hace
desplegando el servidor en un host, no abriendo el de tu portátil.

**`Interactive` como tipo de inicio de sesión.** Hay tres opciones y dos se caen:

| Tipo | Ventana visible | Contraseña | Aquí |
|---|---|---|---|
| `S4U` | No | No | **Falla**: `Acceso denegado` con cuenta de dominio |
| `Password` | No | **Sí, almacenada** | Descartada |
| `Interactive` | Depende | No | La que queda, y funciona |

`S4U` sería lo ideal —sin ventana y sin contraseña— pero necesita privilegios que una
cuenta de dominio corriente no tiene. Lo que quedaba era `Interactive`, con la duda de
si dejaría una consola negra abierta en el escritorio a cada arranque.

No la deja. Comprobado: el proceso lanzado por el Programador de tareas sale con
`MainWindowHandle = 0` y sin título de ventana, es decir, sin consola visible. Así que
no hace falta ningún envoltorio (`.vbs`, `Start-Process -WindowStyle Hidden`…), que
además tienen un coste: la tarea pasaría a gestionar el envoltorio y no el servidor, y
`Stop-ScheduledTask` dejaría de pararlo.

### Manejo diario

```powershell
Get-ScheduledTask     -TaskName 'DiarioIA'   # estado (Ready / Running)
Get-ScheduledTaskInfo -TaskName 'DiarioIA'   # ultima ejecucion y resultado
Start-ScheduledTask   -TaskName 'DiarioIA'   # arrancar ahora, sin reiniciar sesion
Stop-ScheduledTask    -TaskName 'DiarioIA'   # parar
Unregister-ScheduledTask -TaskName 'DiarioIA' -Confirm:$false   # desinstalar
```

También por interfaz: `taskschd.msc`, **Biblioteca del Programador de tareas**.

**`LastTaskResult: 267009` no es un error.** Es `0x00041301`, *"la tarea está ejecutándose
actualmente"*, que es exactamente lo que se espera de un proceso que no termina nunca. El
`0` que uno busca instintivamente solo aparecería si el servidor se hubiera parado.

### La trampa: recompilar con el servidor arrancado

Windows bloquea los ejecutables en uso. Con la tarea corriendo desde
`target\release\diario.exe`, un `cargo build --release` falla al enlazar, porque no puede
sobrescribir el `.exe`. El error habla de acceso denegado y no menciona la tarea
programada por ningún sitio.

Antes de recompilar:

```powershell
Stop-ScheduledTask -TaskName 'DiarioIA'
cargo build --release -p diario-server
Start-ScheduledTask -TaskName 'DiarioIA'
```

La alternativa es copiar el binario a una ubicación estable (`%LOCALAPPDATA%\diario\`) y
apuntar ahí la tarea, desacoplándola del repositorio. Tiene su propio inconveniente: al
recompilar hay que acordarse de copiar, o seguirás ejecutando la versión vieja sin
enterarte.

### Comprobar que quedó bien

```powershell
Get-ScheduledTask -TaskName 'DiarioIA' | Select-Object TaskName, State
(Invoke-WebRequest 'http://127.0.0.1:8787/api/v1/applications' -UseBasicParsing).StatusCode
Get-Process diario | Select-Object Id, MainWindowHandle   # el handle debe ser 0
netstat -ano | Select-String '8787'                       # debe decir 127.0.0.1, no 0.0.0.0
```

La prueba de verdad es cerrar sesión y volver a entrar: el servidor tiene que responder
sin que tú hayas hecho nada.

### Por qué no un servicio de Windows

Sería lo canónico para algo que escucha en un puerto, pero pide dos cosas que aquí no
hay: permisos de administrador para registrarlo, y que el binario implemente el
protocolo del Service Control Manager, que `diario` no hace —arrancado como servicio, el
SCM lo mataría por no responder al *start pending*—. Una tarea al iniciar sesión da casi
lo mismo sin privilegios. Lo que se pierde es que el diario solo está disponible cuando
tú has iniciado sesión, que para un servidor personal en tu propio equipo es justo lo
que quieres.

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
| `cargo build` falla al enlazar           | La tarea `DiarioIA` tiene el `.exe` bloqueado — párala antes de compilar |
| El diario aparece vacío de golpe         | El servidor arrancó con otro `--db` y creó una base nueva                |
| `LastTaskResult` no es `0`               | `267009` (`0x41301`) significa *en ejecución*; es lo correcto aquí       |

---

## Referencias

- `README.md` — arquitectura, API REST y compilación.
- [MCP en Visual Studio](https://learn.microsoft.com/visualstudio/ide/mcp-servers)
- [MCP en VS Code](https://code.visualstudio.com/docs/copilot/chat/mcp-servers)
- [MCP en Claude Code](https://docs.claude.com/en/docs/claude-code/mcp)
