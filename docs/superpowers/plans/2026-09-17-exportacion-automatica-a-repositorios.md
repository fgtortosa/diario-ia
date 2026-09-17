# Exportación automática del diario a los repositorios — Plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Que una llamada a `log_task` deje, sin ninguna acción más del agente, un fichero markdown nuevo en `<repo>/diario-ia/` y en el directorio central de tareas.

**Architecture:** Dos columnas nuevas en SQLite (`application.repo_path`, `entry.exported_at`) que hacen a la vez de destino y de cola de trabajo. Un módulo `exporter` con la lógica de escritura, invocado por una tarea de fondo que se despierta tras cada `create_entry` y procesa **todas** las entradas pendientes. El escritor solo crea ficheros nuevos y nunca modifica uno existente.

**Tech Stack:** Rust, rusqlite (bundled) + r2d2, tokio, clap, chrono, axum.

**Spec:** `docs/superpowers/specs/2026-09-17-exportacion-automatica-a-repositorios-design.md`

## Global Constraints

- **El escritor nunca modifica un fichero existente.** Solo crea. Si el fichero destino ya existe, lo salta.
- **`log_task` nunca falla por un problema de exportación.** Un fallo deja `exported_at` en `NULL` y se reintenta.
- **`exported_at` se marca solo si todos los destinos de esa entrada se escribieron sin error.**
- Nombre de fichero: `YYYYMMDD-HHMMSS-<slug-aplicacion>-<id>.md`, con segundos.
- Los comentarios del código van en castellano sin tildes, como el resto del repositorio.
- Toda la implementación es TDD: test que falla, implementación mínima, test que pasa, commit.

## File Structure

| Fichero | Responsabilidad |
|---|---|
| `migrations/0002_export.sql` (crear) | Las dos columnas y el marcado de lo ya existente |
| `crates/server/src/storage.rs` (modificar) | `migrate()` aplica 0002; métodos de `repo_path`, pendientes y marcado |
| `crates/server/src/exporter.rs` (crear) | Nombre de fichero, markdown de una entrada, resolución de destinos, `export_pending` |
| `crates/server/src/config.rs` (modificar) | `tareas_dir` |
| `crates/server/src/state.rs` (modificar) | Canal para despertar al escritor |
| `crates/server/src/api.rs` (modificar) | Avisar al escritor tras crear una entrada |
| `crates/server/src/main.rs` (modificar) | Subcomando `repo`; arrancar la tarea de fondo |
| `crates/server/Cargo.toml` (modificar) | `tempfile` en dev-dependencies |

`exporter.rs` es fichero nuevo a propósito: `storage.rs` ya tiene 697 líneas y es el fichero más grande del servidor. La escritura a disco no es acceso a datos y no tiene por qué vivir ahí.

---

### Task 1: Migración 0002 — las dos columnas

**Files:**
- Create: `migrations/0002_export.sql`
- Modify: `crates/server/src/storage.rs` (constante junto a `MIGRATION_0001` en la línea 18, y `migrate()` en la 75)
- Test: `crates/server/src/storage.rs` (bloque `#[cfg(test)]`)

**Interfaces:**
- Consumes: nada.
- Produces: las columnas `application.repo_path TEXT` y `entry.exported_at TEXT`, disponibles para las tareas 2 y 3.

- [ ] **Step 1: Escribir el test que falla**

En el bloque `#[cfg(test)] mod tests` de `crates/server/src/storage.rs`:

```rust
#[test]
fn migracion_0002_anade_las_columnas_de_exportacion() {
    let store = Store::in_memory().unwrap();
    let conn = store.pool.get().unwrap();

    let cols_app: Vec<String> = conn
        .prepare("SELECT name FROM pragma_table_info('application')").unwrap()
        .query_map([], |r| r.get(0)).unwrap()
        .collect::<Result<_, _>>().unwrap();
    assert!(cols_app.contains(&"repo_path".to_string()));

    let cols_entry: Vec<String> = conn
        .prepare("SELECT name FROM pragma_table_info('entry')").unwrap()
        .query_map([], |r| r.get(0)).unwrap()
        .collect::<Result<_, _>>().unwrap();
    assert!(cols_entry.contains(&"exported_at".to_string()));
}
```

- [ ] **Step 2: Ejecutar el test y comprobar que falla**

Run: `cargo test -p diario-server migracion_0002 -- --nocapture`
Expected: FAIL — `assertion failed: cols_app.contains(...)`

- [ ] **Step 3: Escribir la migración**

`migrations/0002_export.sql`:

```sql
-- Exportacion automatica del diario a los repositorios.
--
-- repo_path:   donde escribir los ficheros de esa aplicacion. NULL = no se
--              exporta al repositorio, solo al directorio central.
-- exported_at: NULL = pendiente de exportar. Hace de marcado y de cola: lo
--              pendiente es exactamente WHERE exported_at IS NULL.
ALTER TABLE application ADD COLUMN repo_path TEXT;
ALTER TABLE entry ADD COLUMN exported_at TEXT;

-- Las entradas que ya existian tienen su contenido escrito a mano en los
-- DIARIO.md y en TAREAS-EFECTUADAS.md. Marcarlas evita volcar duplicados en la
-- primera pasada. El corte es el dia de la migracion.
UPDATE entry SET exported_at = datetime('now') WHERE exported_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_entry_pendiente ON entry(exported_at) WHERE exported_at IS NULL;
```

- [ ] **Step 4: Aplicarla desde `migrate()`**

En `crates/server/src/storage.rs`, junto a la línea 18:

```rust
const MIGRATION_0002: &str = include_str!("../../../migrations/0002_export.sql");
```

Y en `migrate()`:

```rust
    fn migrate(&self) -> anyhow::Result<()> {
        let conn = self.pool.get()?;
        conn.execute_batch(MIGRATION_0001)?;
        // 0002 no es idempotente: ALTER TABLE ADD COLUMN falla si la columna ya
        // existe. Se comprueba antes en vez de tragarse el error, para no ocultar
        // fallos reales de la migracion.
        if !self.tiene_columna(&conn, "application", "repo_path")? {
            conn.execute_batch(MIGRATION_0002)?;
        }
        Ok(())
    }

    fn tiene_columna(&self, conn: &rusqlite::Connection, tabla: &str, columna: &str) -> anyhow::Result<bool> {
        let n: i64 = conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info(?1) WHERE name = ?2",
            rusqlite::params![tabla, columna],
            |r| r.get(0),
        )?;
        Ok(n > 0)
    }
```

- [ ] **Step 5: Ejecutar el test y comprobar que pasa**

Run: `cargo test -p diario-server migracion_0002`
Expected: PASS

- [ ] **Step 6: Comprobar que abrir dos veces la misma base no rompe**

```rust
#[test]
fn migrar_dos_veces_no_falla() {
    let store = Store::in_memory().unwrap();
    store.migrate().unwrap();   // segunda pasada sobre la misma conexion
}
```

Run: `cargo test -p diario-server migrar_dos_veces`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add migrations/0002_export.sql crates/server/src/storage.rs
git commit -m "Anadir las columnas repo_path y exported_at"
```

---

### Task 2: Storage — asociar una aplicación a un repositorio

**Files:**
- Modify: `crates/server/src/storage.rs`
- Test: `crates/server/src/storage.rs` (bloque `#[cfg(test)]`)

**Interfaces:**
- Consumes: las columnas de la Task 1.
- Produces:
  - `pub fn set_repo_path(&self, aplicacion: &str, ruta: Option<&str>) -> AppResult<bool>` — `false` si la aplicación no existe.
  - `pub fn repo_path_de_slug(&self, slug: &str) -> AppResult<Option<String>>` — **por slug, no por id**: `Entry` expone `application_slug` y `application_name`, pero **no** lleva `application_id`, así que el exportador solo dispone del slug.
  - `pub fn listar_repos(&self) -> AppResult<Vec<(String, Option<String>)>>` — `(nombre_aplicacion, repo_path)`.

- [ ] **Step 1: Escribir los tests que fallan**

```rust
#[test]
fn set_repo_path_guarda_y_lee_la_ruta() {
    let store = Store::in_memory().unwrap();
    store.create_entry(&nueva_entrada("mi-app", "Titulo"), Utc::now()).unwrap();

    assert!(store.set_repo_path("mi-app", Some("C:/repos/mi-app")).unwrap());

    let repos = store.listar_repos().unwrap();
    assert_eq!(repos, vec![("mi-app".to_string(), Some("C:/repos/mi-app".to_string()))]);
}

#[test]
fn set_repo_path_de_aplicacion_inexistente_devuelve_false() {
    let store = Store::in_memory().unwrap();
    assert!(!store.set_repo_path("no-existe", Some("C:/x")).unwrap());
}

#[test]
fn set_repo_path_con_none_borra_la_ruta() {
    let store = Store::in_memory().unwrap();
    store.create_entry(&nueva_entrada("mi-app", "Titulo"), Utc::now()).unwrap();
    store.set_repo_path("mi-app", Some("C:/repos/mi-app")).unwrap();

    store.set_repo_path("mi-app", None).unwrap();

    assert_eq!(store.listar_repos().unwrap(), vec![("mi-app".to_string(), None)]);
}
```

Con este ayudante en el mismo bloque de tests, si no existe ya uno equivalente:

```rust
// NewEntry NO deriva Default: se construye entero.
fn nueva_entrada(app: &str, titulo: &str) -> NewEntry {
    NewEntry {
        application: app.to_string(),
        agent: "test".to_string(),
        model: None,
        title: titulo.to_string(),
        prompt: "prompt".to_string(),
        task_summary: None,
        response_markdown: "respuesta".to_string(),
        tags: vec![],
        attachments: vec![],
        tokens_input: None,
        tokens_output: None,
        duration_ms: None,
        metadata: None,
    }
}
```

- [ ] **Step 2: Ejecutar y comprobar que fallan**

Run: `cargo test -p diario-server repo_path`
Expected: FAIL — `no method named 'set_repo_path'`

- [ ] **Step 3: Implementar**

En `impl Store`, tras `list_applications`:

```rust
    /// Asocia una aplicacion a la carpeta de su repositorio. None la desasocia.
    /// Devuelve false si la aplicacion no existe todavia en el diario.
    pub fn set_repo_path(&self, aplicacion: &str, ruta: Option<&str>) -> AppResult<bool> {
        let conn = self.pool.get()?;
        let slug = slugify(aplicacion);
        let filas = conn.execute(
            "UPDATE application SET repo_path = ?1 WHERE slug = ?2",
            rusqlite::params![ruta, slug],
        )?;
        Ok(filas > 0)
    }

    /// Por slug y no por id: Entry no lleva application_id, solo el slug y el
    /// nombre, y el exportador solo tiene la entrada delante.
    pub fn repo_path_de_slug(&self, slug: &str) -> AppResult<Option<String>> {
        let conn = self.pool.get()?;
        let ruta: Option<Option<String>> = conn
            .query_row(
                "SELECT repo_path FROM application WHERE slug = ?1",
                rusqlite::params![slug],
                |r| r.get(0),
            )
            .optional()?;
        Ok(ruta.flatten())
    }

    pub fn listar_repos(&self) -> AppResult<Vec<(String, Option<String>)>> {
        let conn = self.pool.get()?;
        let mut st = conn.prepare("SELECT name, repo_path FROM application ORDER BY name")?;
        let filas = st
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<String>>(1)?)))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(filas)
    }
```

- [ ] **Step 4: Ejecutar y comprobar que pasan**

Run: `cargo test -p diario-server repo_path`
Expected: PASS (3 tests)

- [ ] **Step 5: Commit**

```bash
git add crates/server/src/storage.rs
git commit -m "Asociar aplicaciones a la carpeta de su repositorio"
```

---

### Task 3: Storage — cola de pendientes y marcado

**Files:**
- Modify: `crates/server/src/storage.rs`
- Test: `crates/server/src/storage.rs`

**Interfaces:**
- Consumes: las columnas de la Task 1.
- Produces:
  - `pub fn entradas_pendientes(&self, limite: usize) -> AppResult<Vec<Entry>>` — las que tienen `exported_at IS NULL`, más antigua primero.
  - `pub fn marcar_exportada(&self, id: i64, cuando: DateTime<Utc>) -> AppResult<()>`
  - `pub fn contar_pendientes_por_aplicacion(&self) -> AppResult<Vec<(String, i64)>>`

- [ ] **Step 1: Escribir los tests que fallan**

```rust
#[test]
fn una_entrada_nueva_nace_pendiente() {
    let store = Store::in_memory().unwrap();
    let id = store.create_entry(&nueva_entrada("mi-app", "Titulo"), Utc::now()).unwrap();

    let pendientes = store.entradas_pendientes(10).unwrap();
    assert_eq!(pendientes.len(), 1);
    assert_eq!(pendientes[0].id, id);
}

#[test]
fn marcar_exportada_la_saca_de_la_cola() {
    let store = Store::in_memory().unwrap();
    let id = store.create_entry(&nueva_entrada("mi-app", "Titulo"), Utc::now()).unwrap();

    store.marcar_exportada(id, Utc::now()).unwrap();

    assert!(store.entradas_pendientes(10).unwrap().is_empty());
}

#[test]
fn las_pendientes_salen_de_la_mas_antigua_a_la_mas_nueva() {
    let store = Store::in_memory().unwrap();
    let primera = store.create_entry(&nueva_entrada("mi-app", "Primera"), Utc::now()).unwrap();
    let segunda = store.create_entry(&nueva_entrada("mi-app", "Segunda"), Utc::now()).unwrap();

    let pendientes = store.entradas_pendientes(10).unwrap();
    assert_eq!(pendientes[0].id, primera);
    assert_eq!(pendientes[1].id, segunda);
}

#[test]
fn contar_pendientes_agrupa_por_aplicacion() {
    let store = Store::in_memory().unwrap();
    store.create_entry(&nueva_entrada("app-a", "Una"), Utc::now()).unwrap();
    store.create_entry(&nueva_entrada("app-a", "Dos"), Utc::now()).unwrap();
    store.create_entry(&nueva_entrada("app-b", "Tres"), Utc::now()).unwrap();

    let cuenta = store.contar_pendientes_por_aplicacion().unwrap();
    assert_eq!(cuenta, vec![("app-a".to_string(), 2), ("app-b".to_string(), 1)]);
}
```

- [ ] **Step 2: Ejecutar y comprobar que fallan**

Run: `cargo test -p diario-server pendiente`
Expected: FAIL — `no method named 'entradas_pendientes'`

- [ ] **Step 3: Implementar**

```rust
    /// Entradas sin exportar, de la mas antigua a la mas nueva.
    pub fn entradas_pendientes(&self, limite: usize) -> AppResult<Vec<Entry>> {
        let conn = self.pool.get()?;
        let mut st = conn.prepare(
            "SELECT id FROM entry WHERE exported_at IS NULL ORDER BY id ASC LIMIT ?1",
        )?;
        let ids: Vec<i64> = st
            .query_map(rusqlite::params![limite as i64], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
        drop(st);
        drop(conn);

        let mut salida = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(e) = self.get_entry(id)? {
                salida.push(e);
            }
        }
        Ok(salida)
    }

    pub fn marcar_exportada(&self, id: i64, cuando: DateTime<Utc>) -> AppResult<()> {
        let conn = self.pool.get()?;
        conn.execute(
            "UPDATE entry SET exported_at = ?1 WHERE id = ?2",
            rusqlite::params![cuando.to_rfc3339(), id],
        )?;
        Ok(())
    }

    pub fn contar_pendientes_por_aplicacion(&self) -> AppResult<Vec<(String, i64)>> {
        let conn = self.pool.get()?;
        let mut st = conn.prepare(
            "SELECT a.name, COUNT(*) FROM entry e \
             JOIN application a ON a.id = e.application_id \
             WHERE e.exported_at IS NULL GROUP BY a.name ORDER BY a.name",
        )?;
        let filas = st
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(filas)
    }
```

- [ ] **Step 4: Ejecutar y comprobar que pasan**

Run: `cargo test -p diario-server pendiente`
Expected: PASS (4 tests)

- [ ] **Step 5: Commit**

```bash
git add crates/server/src/storage.rs
git commit -m "Cola de entradas pendientes de exportar y su marcado"
```

---

### Task 4: Exporter — nombre de fichero y markdown

**Files:**
- Create: `crates/server/src/exporter.rs`
- Modify: `crates/server/src/main.rs` (añadir `mod exporter;` junto a los demás `mod`)
- Test: `crates/server/src/exporter.rs` (bloque `#[cfg(test)]`)

**Interfaces:**
- Consumes: `diario_shared::Entry`.
- Produces:
  - `pub fn nombre_fichero(entry: &Entry) -> String`
  - `pub fn markdown_de(entry: &Entry) -> String`

- [ ] **Step 1: Escribir los tests que fallan**

`crates/server/src/exporter.rs`:

```rust
//! Escritura de las entradas del diario como ficheros markdown en los
//! repositorios. La regla que lo hace seguro: solo se crean ficheros nuevos,
//! nunca se modifica uno existente.

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    // Entry NO deriva Default, asi que se construye entera. Comprobado en
    // crates/shared/src/lib.rs: solo deriva Debug, Clone, Serialize,
    // Deserialize y PartialEq.
    fn entrada_de_prueba() -> Entry {
        Entry {
            id: 19,
            application_slug: "redes-ice-netcore".into(),
            application_name: "redes-ice-netcore".into(),
            agent_name: "claude-code".into(),
            model: Some("claude-opus-5".into()),
            title: "Sacar bin/ y obj/ del control de versiones".into(),
            prompt: "el prompt literal".into(),
            task_summary: Some("el resumen".into()),
            response_markdown: "# Respuesta\n\ncuerpo".into(),
            response_html: "<h1>Respuesta</h1>".into(),
            status: None,
            tags: vec![],
            tokens_input: None,
            tokens_output: None,
            duration_ms: None,
            metadata: None,
            created_at: chrono::Utc.with_ymd_and_hms(2026, 9, 17, 12, 58, 51).unwrap(),
            attachments: vec![],
        }
    }

    #[test]
    fn el_nombre_lleva_fecha_hora_aplicacion_e_id() {
        let e = entrada_de_prueba();
        assert_eq!(nombre_fichero(&e), "20260917-125851-redes-ice-netcore-19.md");
    }

    #[test]
    fn dos_entradas_del_mismo_segundo_no_colisionan() {
        let a = entrada_de_prueba();
        let mut b = entrada_de_prueba();
        b.id = 20;
        assert_ne!(nombre_fichero(&a), nombre_fichero(&b));
    }

    #[test]
    fn el_markdown_lleva_el_titulo_como_h1_y_el_prompt_literal() {
        let md = markdown_de(&entrada_de_prueba());
        assert!(md.starts_with("# Sacar bin/ y obj/ del control de versiones\n"));
        assert!(md.contains("el prompt literal"));
        assert!(md.contains("cuerpo"));
    }
}
```

- [ ] **Step 2: Ejecutar y comprobar que falla**

Run: `cargo test -p diario-server exporter`
Expected: FAIL — `cannot find function 'nombre_fichero'`

- [ ] **Step 3: Implementar**

Al principio de `crates/server/src/exporter.rs`, antes del bloque de tests:

```rust
use diario_shared::Entry;

/// `20260917-125851-redes-ice-netcore-19.md`
///
/// Fecha y hora primero para que el orden alfabetico sea el cronologico; la
/// aplicacion para que el fichero se explique solo fuera de su carpeta; y el id
/// porque cuatro entradas seguidas caen en el mismo segundo y sin el se pisan.
pub fn nombre_fichero(entry: &Entry) -> String {
    format!(
        "{}-{}-{}.md",
        entry.created_at.format("%Y%m%d-%H%M%S"),
        entry.application_slug,
        entry.id
    )
}

pub fn markdown_de(entry: &Entry) -> String {
    let mut s = String::new();
    s.push_str(&format!("# {}\n\n", entry.title));
    s.push_str(&format!("- Aplicacion: {}\n", entry.application_name));
    s.push_str(&format!(
        "- Agente: {} ({})\n",
        entry.agent_name,
        entry.model.as_deref().unwrap_or("-")
    ));
    s.push_str(&format!("- Fecha: {}\n", entry.created_at.to_rfc3339()));
    s.push_str(&format!("- Entrada: {}\n\n", entry.id));
    s.push_str(&format!("## Prompt\n\n{}\n\n", entry.prompt));
    if let Some(resumen) = &entry.task_summary {
        s.push_str(&format!("## Resumen\n\n{}\n\n", resumen));
    }
    s.push_str(&format!("## Respuesta\n\n{}\n", entry.response_markdown));
    s
}
```

Y en `crates/server/src/main.rs`, junto a los demás módulos:

```rust
mod exporter;
```

- [ ] **Step 4: Ejecutar y comprobar que pasan**

Run: `cargo test -p diario-server exporter`
Expected: PASS (3 tests)

- [ ] **Step 5: Commit**

```bash
git add crates/server/src/exporter.rs crates/server/src/main.rs
git commit -m "Nombre de fichero y markdown de una entrada exportada"
```

---

### Task 5: Exporter — escribir las pendientes

**Files:**
- Modify: `crates/server/src/exporter.rs`
- Modify: `crates/server/Cargo.toml` (dev-dependency `tempfile`)
- Test: `crates/server/src/exporter.rs`

**Interfaces:**
- Consumes: `nombre_fichero`, `markdown_de` (Task 4); `entradas_pendientes`, `marcar_exportada`, `repo_path_de` (Tasks 2 y 3).
- Produces: `pub fn exportar_pendientes(store: &Store, tareas_dir: Option<&str>) -> anyhow::Result<usize>` — devuelve cuántas entradas se marcaron.

- [ ] **Step 1: Añadir `tempfile` a dev-dependencies**

En `crates/server/Cargo.toml`:

```toml
[dev-dependencies]
tower = { workspace = true, features = ["util"] }
http-body-util = "0.1"
tempfile = "3"
```

- [ ] **Step 2: Escribir los tests que fallan**

```rust
    #[test]
    fn escribe_en_el_repo_y_en_el_directorio_central() {
        let repo = tempfile::tempdir().unwrap();
        let central = tempfile::tempdir().unwrap();
        let store = Store::in_memory().unwrap();
        store.create_entry(&nueva_entrada("mi-app", "Titulo"), chrono::Utc::now()).unwrap();
        store.set_repo_path("mi-app", Some(repo.path().to_str().unwrap())).unwrap();

        let n = exportar_pendientes(&store, central.path().to_str()).unwrap();

        assert_eq!(n, 1);
        assert_eq!(std::fs::read_dir(repo.path().join("diario-ia")).unwrap().count(), 1);
        assert_eq!(std::fs::read_dir(central.path()).unwrap().count(), 1);
        assert!(store.entradas_pendientes(10).unwrap().is_empty());
    }

    #[test]
    fn sin_repo_path_escribe_solo_en_el_central() {
        let central = tempfile::tempdir().unwrap();
        let store = Store::in_memory().unwrap();
        store.create_entry(&nueva_entrada("sin-repo", "Titulo"), chrono::Utc::now()).unwrap();

        let n = exportar_pendientes(&store, central.path().to_str()).unwrap();

        assert_eq!(n, 1);
        assert_eq!(std::fs::read_dir(central.path()).unwrap().count(), 1);
    }

    #[test]
    fn un_repo_path_inexistente_deja_la_entrada_pendiente() {
        let central = tempfile::tempdir().unwrap();
        let store = Store::in_memory().unwrap();
        store.create_entry(&nueva_entrada("mi-app", "Titulo"), chrono::Utc::now()).unwrap();
        store.set_repo_path("mi-app", Some("Z:/no/existe/de/ninguna/manera")).unwrap();

        let n = exportar_pendientes(&store, central.path().to_str()).unwrap();

        assert_eq!(n, 0);
        assert_eq!(store.entradas_pendientes(10).unwrap().len(), 1);
    }

    #[test]
    fn reintentar_no_duplica_el_fichero_ya_escrito() {
        let repo = tempfile::tempdir().unwrap();
        let central = tempfile::tempdir().unwrap();
        let store = Store::in_memory().unwrap();
        store.create_entry(&nueva_entrada("mi-app", "Titulo"), chrono::Utc::now()).unwrap();
        store.set_repo_path("mi-app", Some(repo.path().to_str().unwrap())).unwrap();

        exportar_pendientes(&store, central.path().to_str()).unwrap();
        // Se fuerza un segundo intento devolviendo la entrada a la cola.
        let id = store.entradas_pendientes(10).unwrap().first().map(|e| e.id).unwrap_or(1);
        store.pool.get().unwrap()
            .execute("UPDATE entry SET exported_at = NULL WHERE id = ?1", rusqlite::params![id])
            .unwrap();

        exportar_pendientes(&store, central.path().to_str()).unwrap();

        assert_eq!(std::fs::read_dir(central.path()).unwrap().count(), 1);
    }

    #[test]
    fn procesa_todas_las_pendientes_no_solo_la_ultima() {
        let central = tempfile::tempdir().unwrap();
        let store = Store::in_memory().unwrap();
        for i in 0..3 {
            store.create_entry(&nueva_entrada("mi-app", &format!("Titulo {i}")), chrono::Utc::now()).unwrap();
        }

        let n = exportar_pendientes(&store, central.path().to_str()).unwrap();

        assert_eq!(n, 3);
    }
```

Añadir al principio del bloque de tests de `exporter.rs` este ayudante (se repite aquí a propósito: las tareas pueden implementarse en cualquier orden y por personas distintas, y cada módulo tiene su propio `mod tests`):

```rust
    use diario_shared::NewEntry;

    // NewEntry NO deriva Default: se construye entero.
    fn nueva_entrada(app: &str, titulo: &str) -> NewEntry {
        NewEntry {
            application: app.to_string(),
            agent: "test".to_string(),
            model: None,
            title: titulo.to_string(),
            prompt: "prompt".to_string(),
            task_summary: None,
            response_markdown: "respuesta".to_string(),
            tags: vec![],
            attachments: vec![],
            tokens_input: None,
            tokens_output: None,
            duration_ms: None,
            metadata: None,
        }
    }
```

Como `Entry`, `NewEntry` tampoco deriva `Default`: comprobado en `crates/shared/src/lib.rs`, solo deriva `Debug`, `Clone`, `Serialize`, `Deserialize` y `PartialEq`.

- [ ] **Step 3: Ejecutar y comprobar que fallan**

Run: `cargo test -p diario-server exportar_pendientes`
Expected: FAIL — `cannot find function 'exportar_pendientes'`

- [ ] **Step 4: Implementar**

```rust
use crate::storage::Store;
use std::path::{Path, PathBuf};

/// Escribe todas las entradas pendientes y devuelve cuantas se marcaron.
///
/// Procesa todas y no solo la ultima a proposito: si el servidor estuvo parado
/// o un repositorio no existia, la siguiente llamada arrastra lo atrasado.
pub fn exportar_pendientes(store: &Store, tareas_dir: Option<&str>) -> anyhow::Result<usize> {
    let mut marcadas = 0usize;

    for entrada in store.entradas_pendientes(200)? {
        let mut destinos: Vec<PathBuf> = Vec::new();

        if let Some(repo) = store.repo_path_de_slug(&entrada.application_slug)? {
            destinos.push(Path::new(&repo).join("diario-ia"));
        }
        if let Some(central) = tareas_dir.filter(|t| !t.is_empty()) {
            destinos.push(PathBuf::from(central));
        }

        let nombre = nombre_fichero(&entrada);
        let cuerpo = markdown_de(&entrada);

        let mut todos_ok = true;
        for dir in &destinos {
            if let Err(e) = escribir_si_no_existe(dir, &nombre, &cuerpo) {
                tracing::warn!("no se pudo exportar la entrada {} a {}: {e}", entrada.id, dir.display());
                todos_ok = false;
            }
        }

        if todos_ok {
            store.marcar_exportada(entrada.id, chrono::Utc::now())?;
            marcadas += 1;
        }
    }

    Ok(marcadas)
}

/// Crea el fichero solo si no existe. Saltarselo es lo que hace correcto el
/// reintento: una entrada tiene dos destinos que pueden fallar por separado, y
/// sin esta comprobacion el que fue bien se escribiria por duplicado.
fn escribir_si_no_existe(dir: &Path, nombre: &str, cuerpo: &str) -> anyhow::Result<()> {
    std::fs::create_dir_all(dir)?;
    let destino = dir.join(nombre);
    if destino.exists() {
        return Ok(());
    }
    std::fs::write(&destino, cuerpo)?;
    Ok(())
}
```

- [ ] **Step 5: Ejecutar y comprobar que pasan**

Run: `cargo test -p diario-server exporter`
Expected: PASS (8 tests entre esta tarea y la anterior)

- [ ] **Step 6: Commit**

```bash
git add crates/server/src/exporter.rs crates/server/Cargo.toml
git commit -m "Escribir las entradas pendientes en repositorio y directorio central"
```

---

### Task 6: Config — `DIARIO_TAREAS_DIR`

**Files:**
- Modify: `crates/server/src/config.rs`
- Modify: `crates/server/src/main.rs` (`ServeArgs` y la construcción de `ServerConfig` en `run_server`, línea 141)
- Test: `crates/server/src/config.rs`

**Interfaces:**
- Consumes: nada.
- Produces: `ServerConfig.tareas_dir: Option<String>`.

- [ ] **Step 1: Escribir el test que falla**

En `crates/server/src/config.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn config_con(tareas: Option<&str>) -> ServerConfig {
        ServerConfig {
            bind: "127.0.0.1:8787".parse().unwrap(),
            db_path: "diario.db".into(),
            public_url: "http://localhost:8787".into(),
            viewer_token: None,
            tareas_dir: tareas.map(|s| s.to_string()),
        }
    }

    #[test]
    fn una_cadena_vacia_equivale_a_no_escribir_destino_central() {
        assert_eq!(config_con(Some("")).tareas_dir_efectivo(), None);
        assert_eq!(config_con(None).tareas_dir_efectivo(), None);
        assert_eq!(config_con(Some("C:/x")).tareas_dir_efectivo(), Some("C:/x"));
    }
}
```

- [ ] **Step 2: Ejecutar y comprobar que falla**

Run: `cargo test -p diario-server tareas_dir`
Expected: FAIL — `struct 'ServerConfig' has no field named 'tareas_dir'`

- [ ] **Step 3: Implementar**

En `crates/server/src/config.rs`, dentro de `ServerConfig`:

```rust
    /// Directorio central de tareas (DIARIO_TAREAS_DIR). Vacio = no se escribe
    /// destino central. Es configurable porque la ruta por defecto es de la
    /// maquina de desarrollo: un servidor desplegado en otro host no la tiene.
    pub tareas_dir: Option<String>,
```

Y en `impl ServerConfig`:

```rust
    pub fn tareas_dir_efectivo(&self) -> Option<&str> {
        self.tareas_dir.as_deref().filter(|t| !t.is_empty())
    }
```

En `crates/server/src/main.rs`, dentro de `ServeArgs`:

```rust
    /// Directorio central donde se acumulan las tareas de todas las aplicaciones.
    #[arg(long, env = "DIARIO_TAREAS_DIR")]
    tareas_dir: Option<String>,
```

Y en `run_server`, al construir `ServerConfig`:

```rust
        tareas_dir: args.tareas_dir,
```

- [ ] **Step 4: Ejecutar y comprobar que pasa**

Run: `cargo test -p diario-server tareas_dir`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/server/src/config.rs crates/server/src/main.rs
git commit -m "Configurar el directorio central de tareas"
```

---

### Task 7: Disparo asíncrono tras crear una entrada

**Files:**
- Modify: `crates/server/src/state.rs`
- Modify: `crates/server/src/api.rs` (donde se llama a `create_entry`)
- Modify: `crates/server/src/main.rs` (`run_server`)
- Test: `crates/server/src/api.rs`

**Interfaces:**
- Consumes: `exportar_pendientes` (Task 5), `tareas_dir_efectivo` (Task 6).
- Produces: `AppState.avisar_exportador: Arc<tokio::sync::Notify>`.

- [ ] **Step 1: Escribir el test que falla**

En el bloque de tests de `crates/server/src/api.rs`:

```rust
    #[tokio::test]
    async fn crear_una_entrada_avisa_al_exportador() {
        let state = estado_de_prueba();
        let aviso = state.avisar_exportador.clone();

        let esperando = tokio::spawn(async move { aviso.notified().await; });
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        state.store.create_entry(&nueva_entrada("mi-app", "Titulo"), chrono::Utc::now()).unwrap();
        state.avisar_exportador.notify_waiters();

        tokio::time::timeout(std::time::Duration::from_secs(1), esperando)
            .await
            .expect("el exportador no recibio el aviso")
            .unwrap();
    }
```

`api.rs` ya construye un `AppState` en sus cuatro tests (`#[tokio::test]` de las líneas 142, 152, 213 y 251). Extraer ese montaje a un ayudante en el mismo bloque y añadirle el campo nuevo:

```rust
    fn estado_de_prueba() -> AppState {
        AppState {
            store: Store::in_memory().unwrap(),
            config: Arc::new(ServerConfig {
                bind: "127.0.0.1:8787".parse().unwrap(),
                db_path: ":memory:".into(),
                public_url: "http://localhost:8787".into(),
                viewer_token: None,
                tareas_dir: None,
            }),
            avisar_exportador: Arc::new(tokio::sync::Notify::new()),
        }
    }
```

- [ ] **Step 2: Ejecutar y comprobar que falla**

Run: `cargo test -p diario-server avisa_al_exportador`
Expected: FAIL — `no field 'avisar_exportador' on type 'AppState'`

- [ ] **Step 3: Añadir el canal al estado**

En `crates/server/src/state.rs`:

```rust
    /// Se dispara tras cada entrada creada para despertar al exportador.
    pub avisar_exportador: Arc<tokio::sync::Notify>,
```

Actualizar todos los sitios que construyen `AppState` con `avisar_exportador: Arc::new(tokio::sync::Notify::new())`.

- [ ] **Step 4: Avisar tras crear la entrada**

En `crates/server/src/api.rs`, justo después de la llamada a `create_entry` del handler de creación:

```rust
    // El aviso va despues de que la entrada este commiteada, y no se espera a
    // que el exportador termine: un fallo de disco no puede tumbar el registro.
    state.avisar_exportador.notify_waiters();
```

- [ ] **Step 5: Arrancar el bucle en `run_server`**

Dentro de `run_async(async move { ... })`, antes de `axum::serve`:

```rust
        let store_exp = state.store.clone();
        let tareas = config.tareas_dir.clone();
        let aviso = state.avisar_exportador.clone();
        tokio::spawn(async move {
            loop {
                let store = store_exp.clone();
                let dir = tareas.clone();
                let r = tokio::task::spawn_blocking(move || {
                    crate::exporter::exportar_pendientes(&store, dir.as_deref())
                })
                .await;
                match r {
                    Ok(Ok(n)) if n > 0 => tracing::info!("exportadas {n} entradas"),
                    Ok(Err(e)) => tracing::warn!("fallo al exportar: {e}"),
                    Err(e) => tracing::warn!("el exportador se cayo: {e}"),
                    _ => {}
                }
                aviso.notified().await;
            }
        });
```

`spawn_blocking` porque `exportar_pendientes` escribe a disco y usa SQLite de forma síncrona: bloquear un hilo del runtime asíncrono pararía también el servidor HTTP. La primera vuelta se ejecuta antes del primer `notified()`, así que al arrancar ya arrastra lo atrasado.

`Store` debe ser `Clone` (lo es, si envuelve un pool r2d2); si no lo fuera, envolverlo en `Arc` en `AppState`.

- [ ] **Step 6: Ejecutar y comprobar que pasa**

Run: `cargo test -p diario-server`
Expected: PASS (toda la suite)

- [ ] **Step 7: Commit**

```bash
git add crates/server/src/state.rs crates/server/src/api.rs crates/server/src/main.rs
git commit -m "Despertar al exportador tras cada entrada creada"
```

---

### Task 8: CLI `diario repo`

**Files:**
- Modify: `crates/server/src/main.rs`
- Test: manual, con los comandos de abajo

**Interfaces:**
- Consumes: `set_repo_path`, `listar_repos`, `contar_pendientes_por_aplicacion` (Tasks 2 y 3).
- Produces: subcomando `repo` con `set`, `list` y `status`.

- [ ] **Step 1: Declarar el subcomando**

En el `enum Command`:

```rust
    /// Asocia aplicaciones a la carpeta de su repositorio.
    Repo {
        #[command(subcommand)]
        action: RepoAction,
    },
```

```rust
#[derive(Subcommand)]
enum RepoAction {
    /// Asocia una aplicacion a la carpeta de su repositorio.
    Set {
        aplicacion: String,
        ruta: String,
        #[arg(long, env = "DIARIO_DB", default_value = "diario.db")]
        db: String,
    },
    /// Que aplicaciones escriben y donde.
    List {
        #[arg(long, env = "DIARIO_DB", default_value = "diario.db")]
        db: String,
    },
    /// Entradas pendientes de exportar, por aplicacion.
    Status {
        #[arg(long, env = "DIARIO_DB", default_value = "diario.db")]
        db: String,
    },
}
```

- [ ] **Step 2: Implementar el despacho**

En `main()`, junto a los demás brazos:

```rust
        Command::Repo { action } => run_repo(action),
```

```rust
fn run_repo(action: RepoAction) -> anyhow::Result<()> {
    match action {
        RepoAction::Set { aplicacion, ruta, db } => {
            let store = Store::open(&db)?;
            let ruta_opt = if ruta.is_empty() { None } else { Some(ruta.as_str()) };
            if store.set_repo_path(&aplicacion, ruta_opt)? {
                println!("{aplicacion} -> {ruta}");
            } else {
                println!("La aplicacion '{aplicacion}' no existe todavia en el diario.");
            }
        }
        RepoAction::List { db } => {
            let store = Store::open(&db)?;
            for (nombre, ruta) in store.listar_repos()? {
                println!("{nombre:<46} {}", ruta.unwrap_or_else(|| "(solo central)".into()));
            }
        }
        RepoAction::Status { db } => {
            let store = Store::open(&db)?;
            let pendientes = store.contar_pendientes_por_aplicacion()?;
            if pendientes.is_empty() {
                println!("No hay entradas pendientes de exportar.");
            }
            for (nombre, n) in pendientes {
                println!("{nombre:<46} {n} pendiente(s)");
            }
        }
    }
    Ok(())
}
```

- [ ] **Step 3: Comprobar a mano**

```bash
cargo build --release -p diario-server
./target/release/diario repo list
./target/release/diario repo set redes-ice-netcore "C:/Users/fgtortosa/codigo/aplicaciones/redes-ice-netcore"
./target/release/diario repo status
```

Expected: `repo list` enumera las aplicaciones; `set` confirma la ruta; `status` dice que no hay pendientes (la migración marcó las 21 antiguas).

- [ ] **Step 4: Commit**

```bash
git add crates/server/src/main.rs
git commit -m "Subcomando repo para asociar aplicaciones a repositorios"
```

---

### Task 9: Documentación y norma

**Files:**
- Modify: `INSTALACION-MCP.md`
- Modify: `README.md`
- Modify: `DIARIO.md` (entrada de la tarea)

- [ ] **Step 1: Documentar en `INSTALACION-MCP.md`**

Añadir una sección **"Que el diario se escriba solo en el repositorio"** después de *"Que el agente registre solo"*, cubriendo: qué hace, `diario repo set`, el nombre de los ficheros, `DIARIO_TAREAS_DIR`, qué pasa si falla (`repo status`), y que los `DIARIO.md` antiguos se quedan como histórico.

- [ ] **Step 2: Actualizar la instrucción de registro**

En esa misma sección, sustituir la instrucción de dos pasos por una de un paso: el agente llama a `log_task` y **no** escribe `DIARIO.md`. Señalar que el `CLAUDE.md` raíz del workspace y el de cada repositorio tienen que cambiar en el mismo sentido, y que eso va en su propio PR porque son otros repositorios.

- [ ] **Step 3: Nota en el `README.md`**

Un párrafo en la sección de integración con agentes, enlazando a la sección nueva.

- [ ] **Step 4: Entrada en `DIARIO.md`**

Fecha y hora, qué se hizo, y las decisiones: por qué multifichero (cero conflictos por construcción), por qué el marcado va después de escribir, por qué la migración marca las 21 antiguas, y por qué `TAREAS-PENDIENTES.md` queda fuera.

- [ ] **Step 5: Commit y PR**

```bash
git add INSTALACION-MCP.md README.md DIARIO.md
git commit -m "Documentar la exportacion automatica al repositorio"
git push -u origin ExportacionAutomaticaAlRepositorio
```

Abrir el PR contra `main` explicando el motivo, el diseño y la verificación.

---

## Verificación final

Recorrer el criterio de terminación de la spec:

- [ ] Una llamada a `log_task` deja el fichero en el repositorio sin nada más.
- [ ] Cuatro entradas seguidas producen cuatro ficheros, sin colisión.
- [ ] Parar el servidor, registrar por REST, arrancarlo y registrar una más escribe también las atrasadas.
- [ ] Una aplicación sin `repo_path` escribe solo en el central.
- [ ] Un `repo_path` inexistente deja la entrada pendiente, no rompe `log_task`, y sale en `repo status`.
- [ ] Dos ramas que registren tareas y hagan merge no producen conflicto.
