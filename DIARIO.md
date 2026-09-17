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

## 2026-09-17 20:30 — Exportación automática del diario a los repositorios

El diario pasa a ser la fuente única: el agente llama a `log_task` y el propio servidor
escribe, de forma asíncrona, un fichero markdown nuevo por entrada en `<repo>/diario-ia/`
y en el directorio central de tareas. Sustituye a mantener un `DIARIO.md` a mano.

Implementado con la spec y el plan del mismo día, en `docs/superpowers/`.

**Lo que se añade**

- `migrations/0002_export.sql`: `application.repo_path` y `entry.exported_at`, más un
  índice parcial sobre lo pendiente.
- `crates/server/src/exporter.rs`: nombre de fichero, markdown de la entrada, resolución
  de destinos y `exportar_pendientes`.
- `storage.rs`: `set_repo_path`, `repo_path_de_slug`, `listar_repos`,
  `entradas_pendientes`, `marcar_exportada`, `contar_pendientes_por_aplicacion`.
- `config.rs`: `DIARIO_TAREAS_DIR`.
- `state.rs` + `api.rs` + `main.rs`: el `Notify` y el bucle de fondo.
- CLI: `diario repo set | list | status`.

**Decisiones**

- **Una carpeta y no un fichero.** Añadir al final de un `DIARIO.md` único garantiza
  conflictos: dos ramas que registren tareas tocan las dos la última línea. Un fichero por
  entrada no puede entrar en conflicto nunca, por construcción.
- **El escritor solo crea ficheros nuevos, nunca modifica uno existente.** De esa única
  regla salen las dos propiedades que sostienen el diseño: cero conflictos, y que nada se
  corrompa si falla a mitad.
- **Se marca `exported_at` solo si todos los destinos fueron bien.** Y por eso
  `escribir_si_no_existe` se salta el fichero ya escrito: si un destino fue bien y otro
  falló, al reintentar el bueno se duplicaría.
- **`exported_at` hace de marcado y de cola a la vez.** Lo pendiente es exactamente
  `WHERE exported_at IS NULL`, así que no hace falta tabla de cola.
- **Se procesan todas las pendientes, no solo la recién creada.** Es lo que hace que el
  sistema se recupere solo tras una parada o un repositorio que no existía.
- **`Notify` y no un canal con cola.** Da igual cuántos avisos se pierdan, porque siempre
  se procesan todas las pendientes.
- **El bucle va en `spawn_blocking`.** Escribe a disco y usa SQLite de forma síncrona:
  bloquear un hilo del runtime pararía también el servidor HTTP.
- **La migración marca las entradas anteriores como exportadas.** Su contenido ya está
  escrito a mano en los `DIARIO.md`; volcarlas duplicaría lo que ya está. El corte es el
  día de la migración.
- **`TAREAS-PENDIENTES.md` queda fuera.** El diario y las efectuadas son registros
  append-only; las pendientes son estado, y con ficheros append-only nada saldría nunca de
  la lista.

**Dos desvíos del plan, ambos a mejor**

- Se usó el ayudante `sample_entry` que ya existía en los tests de `storage.rs` en lugar de
  crear uno nuevo: el patrón del repositorio manda sobre el plan.
- El test del reintento cambió de forma. El plan lo resolvía devolviendo la entrada a la
  cola con un `UPDATE` directo, pero `pool` es privado del módulo `storage`. En vez de
  abrir el campo solo para un test, se ataca `escribir_si_no_existe`, que es la función que
  implementa el salto; sale un test mejor, porque comprueba que el contenido del fichero
  **sigue siendo el primero** tras el segundo intento.

**Verificación**

39 tests en verde. Los que cubren el diseño: escribe en los dos destinos, escribe solo en
el central sin `repo_path`, no escribe central si no hay `DIARIO_TAREAS_DIR`, una ruta
inválida deja la entrada pendiente sin romper nada, el reintento no pisa lo ya escrito,
procesa todas las pendientes, dos entradas del mismo segundo no colisionan, y crear una
entrada por la API despierta al exportador.
