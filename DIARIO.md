# Diario de tareas — diario-ia

Registro de las tareas hechas por agentes sobre este repositorio: fecha y hora locales,
qué se hizo y qué se decidió. Sirve para no repetir errores y para no volver a discutir
decisiones ya tomadas.

---

## 2026-09-17 07:55 — Configuración del MCP en Claude Code y Codex, y documento de instalación por cliente

Se pone el diario a funcionar de verdad en los dos agentes que se usan a diario, y se
documenta la instalación para el resto de clientes.

**Acciones realizadas**

- Creadas dos API keys personales (`claude-code-fgtortosa` id=1, `codex-fgtortosa` id=2).
  Con esto el servidor sale de *modo bootstrap* y toda escritura pasa a exigir
  `Authorization: Bearer`.
- **Claude Code**: servidor MCP `diario` añadido en ámbito `user`
  (`claude mcp add --scope user`), por lo que queda disponible desde cualquier
  repositorio y la key no entra en ninguno.
- **Codex**: corregido `~/.codex/config.toml`. Estaba apuntando a
  `source\repos\herramientas-dotnet-vue\diario-ia\...`, ruta que **ya no existe** desde
  la mudanza del workspace a `~\codigo`; el MCP llevaba tiempo roto sin dar error
  visible. Copia de seguridad en `config.toml.bak-20260917`.
- Nuevo `INSTALACION-MCP.md`: instalación en Claude Code, Codex CLI, GitHub Copilot
  (VS Code, Visual Studio 2022, Copilot CLI y coding agent), Claude Desktop y Cursor,
  más la vía REST para agentes sin MCP. Enlazado desde el `README.md`.

**Decisiones**

- **Ámbito `user`, no `project`, en los dos agentes.** El diario se usa desde todos los
  repositorios, y el ámbito de usuario es el único que garantiza que la key nunca acabe
  dentro de uno. Codex ya estaba así (config global), y ahora hay paridad.
- **El servidor MCP se llama `diario` en todos los clientes.** Codex lo tenía como
  `diarioia`. La instrucción de registro del `CLAUDE.md` dice literalmente *«el MCP
  `diario`»*, así que un nombre único hace que esa instrucción valga igual para
  cualquier agente sin reescribirla.
- **Una key por agente, no una por persona.** `DIARIO_AGENT` ya distingue quién escribe,
  pero con keys separadas se puede revocar un cliente sin tocar el otro.
- **`DIARIO_URL` sigue en `http://127.0.0.1:8787`.** Es el único valor configurado y no
  hay ningún servidor central desplegado ni documentado en el workspace. Implica que
  `diario serve` tiene que estar arrancado para que `log_task` funcione; cuando se
  publique un servidor central, es el único valor a cambiar en los dos clientes.
- **La lista canónica de nombres de `application` queda pendiente**, marcada como
  comentario en `INSTALACION-MCP.md` con una propuesta de partida. Es una decisión de
  equipo, y la duda de fondo (si `-mvc` y `-netcore` comparten nombre o no durante la
  migración) no debe resolverla un agente por su cuenta.

**Verificación**

- REST sin token → `HTTP 401`; con la key → `HTTP 200`.
- MCP por stdio: `initialize` correcto y `tools/list` devuelve las cuatro herramientas
  (`get_entry`, `list_applications`, `list_entries`, `log_task`);
  `tools/call list_applications` responde con datos reales del servidor.
- `~/.codex/config.toml` parsea con `tomllib` y las 33 líneas anteriores quedan intactas.

**Pendiente**

- Borrar la entrada de prueba `_verificacion-mcp`, creada al comprobar la autenticación.
  El API no expone borrado, así que hay que hacerlo contra el SQLite.
- Fijar la lista canónica de `application`.

---

## 2026-09-17 08:15 — El servidor arranca al iniciar sesión: tarea programada `DiarioIA`

Cerrado el PR #1 e integrado en `main`, se monta el arranque automático del servidor, que
era la pieza que faltaba: el MCP de los clientes ya estaba configurado, pero `diario mcp`
es solo un puente y sin `diario serve` levantado `log_task` falla.

**Acciones realizadas**

- Rama local `ConfiguracionMcpClientes` borrada y `main` actualizada a `dd6d677` (incluye
  las correcciones de `b1a980e` y `c814a0d` sobre las rutas de build y de `serve`).
- Tarea programada **`DiarioIA`**, disparada al iniciar sesión del usuario, registrada
  sin permisos de administrador.
- Documentada en `INSTALACION-MCP.md`, sección *Arrancar el servidor al iniciar sesión
  (Windows)*: el comando que la crea, el porqué de cada ajuste, el manejo diario y las
  trampas. Enlazada desde el `README.md` y desde la sección del servidor central.

**Decisiones**

- **`LogonType Interactive`.** De las tres opciones, `S4U` —sin ventana y sin contraseña—
  es la buena, pero devuelve `Acceso denegado` con la cuenta de dominio `CAMPUS\`, porque
  necesita privilegios que no tiene. `Password` almacenaría la contraseña y queda
  descartada. Quedaba `Interactive`, con la duda de si dejaría una consola abierta:
  **comprobado que no** (`MainWindowHandle = 0`), así que no hace falta envoltorio
  `.vbs` ni `Start-Process -WindowStyle Hidden`. Se evita además su coste, que es que la
  tarea pasaría a gestionar el envoltorio y `Stop-ScheduledTask` dejaría de parar el
  servidor.
- **`--db` y `--bind` absolutos en la línea de órdenes**, no confiando en el directorio de
  trabajo. `--db` por defecto es relativo al cwd: con otro cwd, el servidor crearía una
  base vacía en otro sitio, arrancaría sin un solo error y el diario aparecería en blanco.
- **`--bind 127.0.0.1:8787`, cambiando el `0.0.0.0` por defecto.** Un servidor que se
  queda escuchando todo el día es otra cosa que uno levantado a mano un rato: con
  `0.0.0.0` el diario queda accesible desde toda la red y, aunque la escritura exige API
  key, la **lectura es libre** mientras `DIARIO_VIEWER_TOKEN` esté vacío. Compartirlo con
  el equipo es desplegar un servidor, no abrir el del portátil.
- **Límite de ejecución ilimitado** (`PT0S`). El valor por defecto son 3 días: sin
  tocarlo, el servidor moriría solo al tercer día.
- **No se hace servicio de Windows.** Pide administrador y que el binario hable con el
  Service Control Manager, cosa que `diario` no hace. Una tarea al iniciar sesión da casi
  lo mismo sin privilegios; lo que se pierde es que el diario solo está disponible con la
  sesión iniciada, que para un servidor personal es lo deseable.
- **La tarea apunta al binario del repositorio**, no a una copia en `%LOCALAPPDATA%`. Es
  reversible y evita ejecutar una versión vieja sin enterarse, a cambio de que Windows
  bloquee el `.exe`: hay que parar la tarea antes de recompilar. Queda documentado con el
  comando y en la tabla de problemas frecuentes.

**Verificación**

- `Get-ScheduledTask` → `Ready`; tras `Start-ScheduledTask`, servidor respondiendo `200`.
- `MainWindowHandle = 0` y título vacío: sin consola visible.
- `netstat` muestra `127.0.0.1:8787`, ya no `0.0.0.0:8787`.
- El servidor usa la base correcta: `/api/v1/applications` devuelve las dos aplicaciones
  existentes, no una base nueva.
- `LastTaskResult = 267009` = `0x41301`, *en ejecución*, que es el valor correcto para un
  proceso que no termina.

**Pendiente**

- Sigue pendiente de la tarea anterior: borrar la entrada de prueba `_verificacion-mcp` y
  fijar la lista canónica de `application`.
- La prueba definitiva del disparador es cerrar sesión y volver a entrar.

---

## 2026-09-17 13:55 — Fijada la regla de nombres de `application`

El documento llevaba desde su creación un bloque `PENDIENTE` con una propuesta de lista
canónica y una pregunta abierta. Queda resuelto, y no con una lista sino con una regla.

**La regla**

`application` = el nombre de la carpeta del repositorio, tal cual.

Se eligió una regla y no una tabla a propósito: una lista hay que mantenerla y se queda
vieja en cuanto aparece una aplicación nueva; la regla se aplica sola y no puede
desincronizarse del disco.

**Lo que había que aclarar**

Conviven dos nombres para la misma cosa, y confundirlos era el riesgo real:

- `uaRedesIce` — el nombre **real** de la aplicación: `.csproj`, `IdApp`, `Web.config`,
  destino de despliegue. No se toca nunca.
- `redes-ice-netcore` — el nombre **interno**: carpeta del repositorio y `application`
  del diario.

Renombrar la carpeta no afecta al primero, y por eso no rompe despliegues: `build.ps1`
deduce el nombre de la aplicación del `.csproj`, no de la carpeta.

**Decisiones**

- **`-mvc` y `-netcore` son aplicaciones distintas en el diario.** Es la consecuencia
  directa de atar el nombre a la carpeta, y es la buscada: son bases de código distintas,
  con commits y despliegues propios, y durante una migración interesa mirarlas por
  separado. La vista unificada, si hace falta, se consigue con `tags`, que sí se cruzan.
- **Los nugets, uno por paquete**, con su `PackageId`. Cuando el `PackageId` y la carpeta
  no coinciden manda el `PackageId`, que es lo que ve quien lo consume desde el feed.
- **Queda escrito el precio de la regla**: renombrar una carpeta parte en dos la historia
  de esa aplicación en el diario y no hay forma de volver a juntarla.

**Contexto**

La regla se fijó el mismo día en que las carpetas de `aplicaciones/` se aplanaron y
renombraron (fuera el prefijo `ua`, mayúsculas a guiones, sufijo `-netcore`/`-mvc`),
aprovechando que el diario estaba prácticamente vacío. Era el momento más barato: después,
cada renombrado cuesta un corte en la historia.

---

## 2026-09-17 21:25 — Correcciones de la guía de nombres de `application`

Se incorporan las observaciones válidas de la revisión del PR #3.

**Acciones realizadas**

- El ejemplo REST usa ahora `redes-ice-netcore`, el nombre de carpeta, en vez del nombre
  real `uaRedesIce` que la propia regla prohíbe usar como `application`.
- La entrada anterior del diario habla de dos nombres, los dos que enumera, y deja de
  afirmar incorrectamente que son tres.

**Verificación**

- Comprobado que la rama no contiene errores de espacios y que la búsqueda de la guía ya
  no encuentra el ejemplo REST contradictorio.
