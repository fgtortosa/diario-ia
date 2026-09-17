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
