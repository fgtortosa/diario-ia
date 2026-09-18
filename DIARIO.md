# Diario de tareas — diario-ia

Registro de las tareas hechas por agentes sobre este repositorio: fecha y hora locales,
qué se hizo y qué se decidió. Sirve para no repetir errores y para no volver a discutir
decisiones ya tomadas.

---

## 2026-09-17 21:30 — Fiabilidad del exportador tras la revisión del PR #4

Se corrigen las observaciones aplicables de Copilot en el exportador y en el filtro web.

**Acciones realizadas**

- La exportación crea primero un temporal único y lo publica por renombrado, sin aceptar
  ficheros parciales como exportaciones completas; además drena los lotes de pendientes.
- La migración 0002 se recupera si quedó interrumpida entre sus dos columnas, la prueba
  de ruta inválida es portable y «Limpiar filtros» borra también la etiqueta activa.

**Verificación**

- `cargo test -p diario-server --quiet`: 40 pruebas correctas.

---

## 2026-09-17 21:37 — Completar las correcciones de la revisión del PR #5

Se incorporan las tres observaciones adicionales de Copilot sobre el parche de fiabilidad.

**Acciones realizadas**

- La publicación usa un hard link desde el temporal, que falla si el destino ya existe,
  y elimina siempre el temporal tras publicar o tras cualquier error de escritura.
- La exportación recorre la cola por id para que las entradas fallidas no bloqueen las
  posteriores; la migración detecta también la ausencia de su índice finalizador.

**Verificación**

- `cargo test -p diario-server --quiet`: 40 pruebas correctas.

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

---

## 2026-09-17 20:25 — Filtro por etiqueta en la web

La API soportaba `tag` desde siempre (`EntryQuery.tag`), pero el cliente no lo usaba: las
etiquetas se pintaban como texto muerto y no había forma de filtrar por ellas desde la web.

**Acciones realizadas**

- `EntryFilters` gana `tag: Option<String>` y lo envía como `&tag=`.
- Las etiquetas de cada tarjeta pasan de `<span>` a `<button>` clicable.
- Un chip en la cabecera muestra la etiqueta activa con una × para quitarla.
- Estilos para la etiqueta clicable, con `:hover` y `:focus-visible`.

**Decisiones**

- **Etiquetas clicables en vez de un control nuevo en la cabecera.** Es más descubrible y no
  añade ruido: ya estaban ahí, solo eran inertes.
- **`ev.stop_propagation()` en el clic.** La tarjeta entera tiene un `on:click` que navega
  al detalle; sin eso, pulsar una etiqueta filtraría y navegaría a la vez.
- **Se usan las variables CSS que existen** (`--text`, `--accent`, `--accent-soft`). En la
  primera versión inventé una `--fg` que no estaba declarada.

**Verificación**

- La SPA compila con `trunk build --release` (Leptos es tipado: eso valida el cableado de
  la señal al filtro y a la URL).
- El filtro del servidor responde: `?tag=exportacion` devuelve 1, `?tag=gitignore` devuelve
  1, `?tag=no-existe` devuelve 0.
- El WASM que sirve el binario contiene las cadenas nuevas, así que lo desplegado es la
  compilación nueva y no una embebida vieja.
- **Comprobado en el navegador con Playwright** (la extensión de Chrome no conectaba):
  pulsar la etiqueta `gitignore` deja el listado en 1 entrada de 23 y aparece el chip
  `#gitignore ×` en la cabecera; la × lo quita y vuelven las 23. Lo importante es que la
  URL sigue siendo `/` tras pulsar la etiqueta: el `stop_propagation` funciona y no navega
  al detalle. Sin errores de consola.

---

## 2026-09-17 23:26 — Exportación manual, marcado, etiquetas y modo log

Cuatro bloques sobre lo construido esta misma tarde: descargar una entrada en md y en PDF,
ver y cambiar su estado de exportación, filtrar por etiqueta desde una lista con recuentos,
y seguir por consola cada operación.

**Decisiones**

- **PDF con `window.print()` y hoja `@media print`, no con una librería.** Meter un
  generador significaría maquetar a mano lo que el navegador ya sabe hacer, o empaquetar un
  Chrome headless en un ejecutable que hoy son 10 MB y presume de no tener dependencias de
  sistema. Lo que sí hacía falta era una hoja de impresión de verdad.
- **`markdown_de` se mueve de `exporter.rs` a `shared`.** Es lo importante del bloque de
  descarga: lo usan el exportador y la web, así que con una sola definición el fichero que
  te bajas y el que se exporta son idénticos **por construcción**, no por coincidencia.
- **La descarga se hace en el cliente con un `Blob`**, sin ruta nueva: el detalle ya tiene
  el markdown, y un endpoint duplicaría lo que ya viajó.
- **La hora de los nombres pasa de UTC a local.** Era el punto que quedaba abierto. Dentro
  de un día el desfase es constante, así que el orden alfabético sigue siendo el
  cronológico.
- **Marcar el estado va con la lectura y no con la escritura.** No crea ni modifica
  contenido, solo cambia una marca de control: quien puede leer el diario entero, con sus
  prompts, puede cambiar una bandera. Ponerlo bajo la API key dejaba el botón inservible
  desde la web.
- **El modo log nunca traza el prompt ni la respuesta**, que es justo lo que no quieres
  volcado en una consola.
- **`traza()` vive en `ServerConfig`** y no repartida por los handlers, para que el formato
  sea uno solo y activarlo sea un único sitio.

**Dos cosas que solo aparecieron al probarlas**

- **El botón de marcar daba 401 desde el navegador.** Lo había puesto en el router de
  escritura, que exige API key, y la web no tiene ninguna ni debe tenerla. Se vio en la
  consola del navegador al probar el ciclo completo con Playwright, no compilando. Queda un
  test de regresión que crea una key y comprueba que la marca sigue funcionando sin ella.
- **`parse_dt` solo aceptaba RFC3339**, pero la migración 0002 marcó las entradas antiguas
  con `datetime('now')` de SQLite. Sin arreglarlo, esas entradas aparecían como *no
  exportadas* en la interfaz, que es exactamente lo contrario de lo que son.

**Verificación**

47 tests. Y en el navegador, con Playwright: la lista de etiquetas filtra (26 → 1); el
conmutador de pendientes responde; descargar produce
`20260917-213731-diario-ia-26.md`; y marcar y desmarcar cambia el indicador en los dos
sentidos. Al desmarcar se vio el comportamiento diseñado: el exportador la recoge
inmediatamente, respeta el fichero que ya existe y la vuelve a marcar.

El modo log se comprobó con un servidor aparte, porque el que corre como tarea programada
lo hace oculto y su consola no la ve nadie —cosa que queda advertida en la documentación—.
Las cinco operaciones salieron trazadas, y la última como `[mcp]` al mandar la cabecera.

**Matiz que conviene saber**

Las entradas exportadas **antes** de este cambio tienen nombre en UTC en disco, mientras que
el botón de descargar usa hora local. Para las entradas nuevas coinciden; para las viejas
no. No se renombra nada: reescribir ficheros ya escritos es justo lo que el exportador
evita por diseño.

---

## 2026-09-17 23:35 — Corregir el control de acceso del marcado

La revisión de seguridad del commit señaló *broken access control* y *privilege escalation*
en `api.rs`, y tenía razón.

**Qué estaba mal**

Había puesto `PUT /entries/{id}/exported` en el router de **lectura**, justificándolo con
que «no crea ni modifica contenido, solo cambia una marca» y que «quien puede leer el
diario entero puede cambiar una bandera». Las dos partes del argumento eran falsas:

- **No es una marca inocua.** Desmarcar hace que el servidor escriba ficheros en
  repositorios git del disco; marcar suprime para siempre la exportación de esa entrada.
- **La comparación no se sostenía.** `require_viewer` no comprueba nada mientras no exista
  `DIARIO_VIEWER_TOKEN`, y el bind por defecto es `0.0.0.0:8787`. Así que no era «quien
  puede leer», era **cualquiera que alcance el puerto**, sin credencial, provocando
  escrituras en disco y pérdida silenciosa de registros.

Lo que pasó de verdad es que dejé que una restricción de interfaz —la web no tiene API
key— decidiera dónde ponía el límite de permisos.

**Arreglo**

El endpoint vuelve a exigir API key, como cualquier escritura. Para que el botón siga
sirviendo en una instancia local hay una opción **explícita y apagada por defecto**,
`--marcado-abierto` / `DIARIO_MARCADO_ABIERTO`, que conviene encender solo junto a
`--bind 127.0.0.1`.

**Verificación**

Dos tests, uno por caso: por defecto el `PUT` da 401 **y el cambio no se aplica**; con la
opción encendida da 200 y sí se aplica. La primera versión del test afirmaba algo que no
demostraba nada —comprobaba que la entrada seguía pendiente, y nacía pendiente—, así que se
cambió a marcar *como exportada*, que es la dirección en la que el cambio se nota.

En marcha: con la opción encendida, el `PUT` sin clave responde 200 y crear entradas sigue
respondiendo 401. La tarea programada la lleva encendida porque escucha en 127.0.0.1.

---

## 2026-09-18 07:55 — Volcar lo pendiente al arrancar, al cerrar y a mano

**Al arrancar ya funcionaba**: la primera vuelta del bucle del exportador corre antes del
primer `notified().await`. No hizo falta tocarlo, y se comprobó en el log al arrancar un
servidor con una entrada pendiente: `exportadas 1 entradas al repositorio`.

**Al cerrar** se añade `with_graceful_shutdown`, escuchando Ctrl-C y, en Windows, cierre de
consola, apagado y cierre de sesión; después una pasada final.

**A mano**, botón *Exportar pendientes* en la barra lateral, contra un
`POST /api/v1/exportar` que devuelve cuántas escribió y cuántas quedan.

**Lo que había que medir, y se midió**

El cierre ordenado **no sirve con la tarea programada**. Prueba: se deja una entrada
pendiente apuntando el repositorio a una ruta rota, se repara el destino sin avisar al
servidor, y se ejecuta `Stop-ScheduledTask`. El fichero **no** aparece: el proceso muere
sin recibir el evento.

En cambio, con una señal real —`taskkill` sin `/F` sobre un servidor arrancado a mano— el
log muestra `cierre solicitado: dejando de aceptar peticiones` seguido de la pasada final.

Por eso el botón no es una comodidad por no ver la consola: es **la vía fiable** en la
configuración habitual, y el cierre ordenado es el añadido que funciona cuando el sistema
lo permite.

**Lo que conviene no olvidar**

Nada de esto evita una pérdida, porque no la hay: como una entrada no se marca exportada
hasta haberse escrito, lo pendiente al morir el proceso lo recupera la pasada de arranque.
Esto ahorra esperar al reinicio.

**Verificación**

50 tests. Y en el navegador: con una entrada pendiente de verdad, pulsar el botón muestra
«1 escrita(s)» y la cola pasa de 1 a 0, sin errores de consola.

---

## 2026-09-18 09:00 — URLs navegables y vista en hilo

Los filtros pasan a la query de la URL, en los dos sentidos, y se añade `/hilo`: la misma
búsqueda mostrada entera en una sola página, para copiársela a un agente.

**Decisiones**

- **Los nombres de los parámetros son los de la API**, no unos propios en castellano. La
  query que ves en el navegador es la que vale para `curl`: `/hilo?application=x&tag=y` y
  `/api/v1/hilo?application=x&tag=y` llevan la misma cadena, y no hay tabla de traducción
  que pueda desincronizarse.
- **`query_de`/`query_a` viven en `shared`.** `crates/client` solo se compila a wasm, así
  que un test ahí no correría con `cargo test`; y con una sola definición la URL del
  navegador y la de la API se construyen igual **por construcción**, como `markdown_de`
  hace con el fichero exportado. La librería es `serde_urlencoded`, que ya estaba en el
  lock porque la arrastra axum: es la misma con la que el servidor lee la query.
- **No se apila nada si la URL ya dice lo mismo.** De eso depende que el botón de atrás
  funcione: al volver, `popstate` repone los filtros, el `Effect` recalcula esa misma URL
  y no empuja una entrada encima de la que se acaba de dejar.
- **La URL del detalle arrastra los filtros aunque no los use.** Sin eso, volver con el
  botón de atrás desde una entrada los borraba, porque `popstate` los lee de la query y
  `/entry/27` no lleva ninguna.
- **El hilo va en orden ascendente y sin recortar nada**: es una historia, y se lee de
  principio a fin. El prompt va desplegado y no dentro de un `<details>` como en el
  detalle, porque el hilo se lee y se copia de un tirón.
- **Tope de 200 entradas, con error en vez de truncado.** El hilo devuelve las entradas
  enteras, no el fragmento del listado, así que sin tope `/api/v1/hilo` sin filtros se
  traía el diario completo a memoria —y es ruta de lectura, con `require_viewer` inactivo
  mientras no exista `DIARIO_VIEWER_TOKEN`—. Cortar en silencio sería peor que el error:
  le daría al agente media historia sin que nadie se entere.
- **Si no hay portapapeles, el botón descarga el fichero.** `navigator.clipboard` solo
  existe en contexto seguro: `localhost` sí, por IP en `http` no. Se comprueba antes y se
  dice lo que pasó, en vez de que el botón no haga nada.

**Verificación**

54 tests, `just clippy` en verde y comprobación en el navegador con Playwright contra la
base real, con 0 errores de consola:

| Qué | Resultado |
|---|---|
| Filtros a la URL | Pulsar aplicación y etiqueta deja `/?application=diario-ia&tag=rust` |
| Atrás | Deshace filtro a filtro: quita la etiqueta (12 tarjetas) y luego la aplicación |
| Enlace directo | `/hilo?application=redes-ice-netcore&from=2026-09-01&q=build` repone barra lateral, fecha y buscador, y muestra 1 entrada |
| Hilo | 2 entradas completas, ascendentes, con el prompt visible |
| Copiar markdown | 5.505 caracteres en el portapapeles, cabecera correcta, 1 separador y 6 secciones |
| Detalle | `/entry/27?tag=rust` conserva el filtro al volver y al pulsar atrás |

**De paso**

`just clippy` estaba rojo desde el PR anterior por dos avisos del exportador (un import y
un método que en producción ya no llama nadie). Corregidos: la puerta de lint del
repositorio vuelve a pasar.
