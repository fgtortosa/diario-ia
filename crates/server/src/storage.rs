//! Capa de acceso a datos sobre SQLite (rusqlite + pool r2d2).
//!
//! Toda la BD vive en un unico fichero (o en memoria para tests). Las
//! operaciones son sincronas; los handlers Axum las envuelven en
//! `tokio::task::spawn_blocking`.

use chrono::{DateTime, Utc};
use diario_shared::{
    Application, Attachment, DayCount, Entry, EntryPage, EntryQuery, EntrySummary, NewEntry,
};
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{params, params_from_iter, types::Value, OptionalExtension};
use sha2::{Digest, Sha256};

use crate::error::{AppError, AppResult};
use crate::render;

const MIGRATION_0001: &str = include_str!("../../../migrations/0001_init.sql");

/// Separador de unidad (US) usado para agrupar tags con group_concat.
const TAG_SEP: char = '\u{1f}';

pub type Pool = r2d2::Pool<SqliteConnectionManager>;

#[derive(Clone)]
pub struct Store {
    pool: Pool,
}

/// Informacion de una API key verificada.
#[derive(Debug, Clone)]
pub struct KeyInfo {
    pub id: i64,
    pub name: String,
    /// Reservado para futura comprobacion de permisos por scope.
    #[allow(dead_code)]
    pub scope: String,
}

/// Fila de listado de API keys (sin el secreto).
#[derive(Debug, Clone)]
pub struct ApiKeyRow {
    pub id: i64,
    pub name: String,
    pub scope: String,
    pub active: bool,
    pub created_at: String,
    pub last_used_at: Option<String>,
}

impl Store {
    /// Abre (o crea) la base de datos en la ruta indicada.
    pub fn open(path: &str) -> anyhow::Result<Self> {
        let manager = SqliteConnectionManager::file(path).with_init(init_conn);
        let pool = r2d2::Pool::builder().build(manager)?;
        let store = Store { pool };
        store.migrate()?;
        Ok(store)
    }

    /// Base de datos en memoria (para tests). Una unica conexion compartida.
    #[cfg(test)]
    pub fn in_memory() -> anyhow::Result<Self> {
        let manager = SqliteConnectionManager::memory().with_init(init_conn);
        let pool = r2d2::Pool::builder()
            .max_size(1)
            .build(manager)?;
        let store = Store { pool };
        store.migrate()?;
        Ok(store)
    }

    fn migrate(&self) -> anyhow::Result<()> {
        let conn = self.pool.get()?;
        conn.execute_batch(MIGRATION_0001)?;
        Ok(())
    }

    // ----------------------------------------------------------------- apps

    pub fn list_applications(&self) -> AppResult<Vec<Application>> {
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare(
            "SELECT a.id, a.slug, a.name, a.description,
                    (SELECT count(*) FROM entry e WHERE e.application_id = a.id) AS cnt,
                    (SELECT max(e.created_at) FROM entry e WHERE e.application_id = a.id) AS last
             FROM application a
             ORDER BY last DESC NULLS LAST, a.name",
        )?;
        let rows = stmt.query_map([], |r| {
            let last: Option<String> = r.get(5)?;
            Ok(Application {
                id: r.get(0)?,
                slug: r.get(1)?,
                name: r.get(2)?,
                description: r.get(3)?,
                entry_count: r.get(4)?,
                last_activity: last.and_then(|s| parse_dt(&s)),
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    // -------------------------------------------------------------- entries

    /// Crea una entrada (y la aplicacion si no existe). Devuelve el id.
    pub fn create_entry(&self, new: &NewEntry, now: DateTime<Utc>) -> AppResult<i64> {
        if new.title.trim().is_empty() {
            return Err(AppError::BadRequest("title es obligatorio".into()));
        }
        if new.application.trim().is_empty() {
            return Err(AppError::BadRequest("application es obligatorio".into()));
        }

        let mut conn = self.pool.get()?;
        let tx = conn.transaction()?;
        let now_s = now.to_rfc3339();

        let slug = slugify(&new.application);
        let app_id: i64 = match tx
            .query_row("SELECT id FROM application WHERE slug = ?1", [&slug], |r| {
                r.get(0)
            })
            .optional()?
        {
            Some(id) => id,
            None => {
                tx.execute(
                    "INSERT INTO application (slug, name, description, created_at)
                     VALUES (?1, ?2, NULL, ?3)",
                    params![slug, new.application.trim(), now_s],
                )?;
                tx.last_insert_rowid()
            }
        };

        let response_html = render::render_markdown(&new.response_markdown);
        let metadata_s = new.metadata.as_ref().map(|v| v.to_string());

        tx.execute(
            "INSERT INTO entry (application_id, agent_name, model, title, prompt, task_summary,
                 response_markdown, response_html, status, tokens_input, tokens_output,
                 duration_ms, metadata_json, created_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,NULL,?9,?10,?11,?12,?13)",
            params![
                app_id,
                new.agent,
                new.model,
                new.title.trim(),
                new.prompt,
                new.task_summary,
                new.response_markdown,
                response_html,
                new.tokens_input,
                new.tokens_output,
                new.duration_ms,
                metadata_s,
                now_s,
            ],
        )?;
        let entry_id = tx.last_insert_rowid();

        {
            let mut stmt = tx.prepare(
                "INSERT OR IGNORE INTO entry_tag (entry_id, tag) VALUES (?1, ?2)",
            )?;
            for tag in &new.tags {
                let t = tag.trim();
                if !t.is_empty() {
                    stmt.execute(params![entry_id, t])?;
                }
            }
        }

        {
            let mut stmt = tx.prepare(
                "INSERT INTO attachment (entry_id, filename, kind, content_markdown, content_html)
                 VALUES (?1,?2,?3,?4,?5)",
            )?;
            for att in &new.attachments {
                let html = render::render_markdown(&att.content_markdown);
                stmt.execute(params![
                    entry_id,
                    att.filename,
                    att.kind,
                    att.content_markdown,
                    html
                ])?;
            }
        }

        tx.commit()?;
        Ok(entry_id)
    }

    /// Consulta paginada de entradas con filtros.
    pub fn query_entries(&self, q: &EntryQuery) -> AppResult<EntryPage> {
        let conn = self.pool.get()?;
        let limit = q.limit.unwrap_or(50).clamp(1, 200);

        let mut sql = String::from(
            "SELECT e.id, a.slug, a.name, e.agent_name, e.model, e.title,
                    e.response_markdown, e.created_at,
                    (SELECT group_concat(t.tag, char(31)) FROM entry_tag t WHERE t.entry_id = e.id)
             FROM entry e JOIN application a ON a.id = e.application_id",
        );
        let mut wheres: Vec<String> = Vec::new();
        let mut args: Vec<Value> = Vec::new();

        if let Some(app) = non_empty(&q.application) {
            wheres.push("a.slug = ?".into());
            args.push(Value::Text(slugify(app)));
        }
        if let Some(from) = non_empty(&q.from) {
            wheres.push("substr(e.created_at,1,10) >= ?".into());
            args.push(Value::Text(from.to_string()));
        }
        if let Some(to) = non_empty(&q.to) {
            wheres.push("substr(e.created_at,1,10) <= ?".into());
            args.push(Value::Text(to.to_string()));
        }
        if let Some(tag) = non_empty(&q.tag) {
            wheres.push("EXISTS (SELECT 1 FROM entry_tag t WHERE t.entry_id = e.id AND t.tag = ?)".into());
            args.push(Value::Text(tag.to_string()));
        }
        if let Some(text) = non_empty(&q.q) {
            wheres.push(
                "(e.title LIKE ? OR e.prompt LIKE ? OR e.task_summary LIKE ? OR e.response_markdown LIKE ?)".into(),
            );
            let pat = format!("%{}%", text.replace('%', "\\%").replace('_', "\\_"));
            for _ in 0..4 {
                args.push(Value::Text(pat.clone()));
            }
        }
        if let Some(cursor) = q.cursor {
            wheres.push("e.id < ?".into());
            args.push(Value::Integer(cursor));
        }

        if !wheres.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&wheres.join(" AND "));
        }
        sql.push_str(" ORDER BY e.id DESC LIMIT ?");
        args.push(Value::Integer(limit + 1));

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(args.iter()), |r| {
            let created: String = r.get(7)?;
            let tags: Option<String> = r.get(8)?;
            let md: String = r.get(6)?;
            Ok(EntrySummary {
                id: r.get(0)?,
                application_slug: r.get(1)?,
                application_name: r.get(2)?,
                agent_name: r.get(3)?,
                model: r.get(4)?,
                title: r.get(5)?,
                snippet: render::snippet(&md, 180),
                tags: split_tags(tags),
                created_at: parse_dt(&created).unwrap_or_else(Utc::now),
            })
        })?;

        let mut entries = Vec::new();
        for r in rows {
            entries.push(r?);
        }

        let next_cursor = if entries.len() as i64 > limit {
            entries.truncate(limit as usize);
            entries.last().map(|e| e.id)
        } else {
            None
        };

        Ok(EntryPage {
            entries,
            next_cursor,
        })
    }

    /// Devuelve una entrada completa por id.
    pub fn get_entry(&self, id: i64) -> AppResult<Option<Entry>> {
        let conn = self.pool.get()?;
        let entry = conn
            .query_row(
                "SELECT e.id, a.slug, a.name, e.agent_name, e.model, e.title, e.prompt,
                        e.task_summary, e.response_markdown, e.response_html, e.status,
                        e.tokens_input, e.tokens_output, e.duration_ms, e.metadata_json, e.created_at
                 FROM entry e JOIN application a ON a.id = e.application_id
                 WHERE e.id = ?1",
                [id],
                |r| {
                    let created: String = r.get(15)?;
                    let metadata: Option<String> = r.get(14)?;
                    Ok(Entry {
                        id: r.get(0)?,
                        application_slug: r.get(1)?,
                        application_name: r.get(2)?,
                        agent_name: r.get(3)?,
                        model: r.get(4)?,
                        title: r.get(5)?,
                        prompt: r.get(6)?,
                        task_summary: r.get(7)?,
                        response_markdown: r.get(8)?,
                        response_html: r.get(9)?,
                        status: r.get(10)?,
                        tokens_input: r.get(11)?,
                        tokens_output: r.get(12)?,
                        duration_ms: r.get(13)?,
                        metadata: metadata.and_then(|s| serde_json::from_str(&s).ok()),
                        created_at: parse_dt(&created).unwrap_or_else(Utc::now),
                        tags: Vec::new(),
                        attachments: Vec::new(),
                    })
                },
            )
            .optional()?;

        let Some(mut entry) = entry else {
            return Ok(None);
        };

        let mut tstmt = conn.prepare("SELECT tag FROM entry_tag WHERE entry_id = ?1 ORDER BY tag")?;
        entry.tags = tstmt
            .query_map([id], |r| r.get::<_, String>(0))?
            .collect::<Result<_, _>>()?;

        let mut astmt = conn.prepare(
            "SELECT id, filename, kind, content_markdown, content_html
             FROM attachment WHERE entry_id = ?1 ORDER BY id",
        )?;
        entry.attachments = astmt
            .query_map([id], |r| {
                Ok(Attachment {
                    id: r.get(0)?,
                    filename: r.get(1)?,
                    kind: r.get(2)?,
                    content_markdown: r.get(3)?,
                    content_html: r.get(4)?,
                })
            })?
            .collect::<Result<_, _>>()?;

        Ok(Some(entry))
    }

    /// Recuento de entradas por dia (para el heatmap).
    pub fn day_counts(&self, application: Option<&str>, from: Option<&str>, to: Option<&str>) -> AppResult<Vec<DayCount>> {
        let conn = self.pool.get()?;
        let mut sql = String::from(
            "SELECT substr(e.created_at,1,10) AS day, count(*)
             FROM entry e JOIN application a ON a.id = e.application_id",
        );
        let mut wheres: Vec<String> = Vec::new();
        let mut args: Vec<Value> = Vec::new();
        if let Some(app) = application.filter(|s| !s.is_empty()) {
            wheres.push("a.slug = ?".into());
            args.push(Value::Text(slugify(app)));
        }
        if let Some(f) = from.filter(|s| !s.is_empty()) {
            wheres.push("substr(e.created_at,1,10) >= ?".into());
            args.push(Value::Text(f.to_string()));
        }
        if let Some(t) = to.filter(|s| !s.is_empty()) {
            wheres.push("substr(e.created_at,1,10) <= ?".into());
            args.push(Value::Text(t.to_string()));
        }
        if !wheres.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&wheres.join(" AND "));
        }
        sql.push_str(" GROUP BY day ORDER BY day");

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(args.iter()), |r| {
            Ok(DayCount {
                day: r.get(0)?,
                count: r.get(1)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    // ------------------------------------------------------------- api keys

    /// Crea una API key. Devuelve (id, token_en_claro). El token solo se ve aqui.
    pub fn create_api_key(&self, name: &str, scope: &str) -> AppResult<(i64, String)> {
        let token = generate_token();
        let hash = hash_key(&token);
        let conn = self.pool.get()?;
        conn.execute(
            "INSERT INTO api_key (name, key_hash, scope, active, created_at)
             VALUES (?1, ?2, ?3, 1, ?4)",
            params![name, hash, scope, Utc::now().to_rfc3339()],
        )?;
        Ok((conn.last_insert_rowid(), token))
    }

    pub fn list_api_keys(&self) -> AppResult<Vec<ApiKeyRow>> {
        let conn = self.pool.get()?;
        let mut stmt = conn.prepare(
            "SELECT id, name, scope, active, created_at, last_used_at
             FROM api_key ORDER BY id",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(ApiKeyRow {
                id: r.get(0)?,
                name: r.get(1)?,
                scope: r.get(2)?,
                active: r.get::<_, i64>(3)? != 0,
                created_at: r.get(4)?,
                last_used_at: r.get(5)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn revoke_api_key(&self, id: i64) -> AppResult<bool> {
        let conn = self.pool.get()?;
        let n = conn.execute("UPDATE api_key SET active = 0 WHERE id = ?1", [id])?;
        Ok(n > 0)
    }

    /// Verifica un token en claro. Devuelve la info si es valido y esta activo.
    pub fn verify_api_key(&self, token: &str) -> AppResult<Option<KeyInfo>> {
        let hash = hash_key(token);
        let conn = self.pool.get()?;
        let info = conn
            .query_row(
                "SELECT id, name, scope FROM api_key WHERE key_hash = ?1 AND active = 1",
                [&hash],
                |r| {
                    Ok(KeyInfo {
                        id: r.get(0)?,
                        name: r.get(1)?,
                        scope: r.get(2)?,
                    })
                },
            )
            .optional()?;
        if let Some(ref k) = info {
            let _ = conn.execute(
                "UPDATE api_key SET last_used_at = ?1 WHERE id = ?2",
                params![Utc::now().to_rfc3339(), k.id],
            );
        }
        Ok(info)
    }

    /// Numero de API keys activas (para decidir si exigir auth).
    pub fn active_key_count(&self) -> AppResult<i64> {
        let conn = self.pool.get()?;
        let n = conn.query_row("SELECT count(*) FROM api_key WHERE active = 1", [], |r| r.get(0))?;
        Ok(n)
    }
}

// --------------------------------------------------------------- utilidades

fn init_conn(conn: &mut rusqlite::Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA foreign_keys = ON;
         PRAGMA busy_timeout = 5000;
         PRAGMA synchronous = NORMAL;",
    )
}

fn non_empty(opt: &Option<String>) -> Option<&str> {
    opt.as_deref().map(str::trim).filter(|s| !s.is_empty())
}

fn parse_dt(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|d| d.with_timezone(&Utc))
}

fn split_tags(joined: Option<String>) -> Vec<String> {
    match joined {
        Some(s) if !s.is_empty() => s.split(TAG_SEP).map(|t| t.to_string()).collect(),
        _ => Vec::new(),
    }
}

/// Convierte un texto libre en un slug ("Portal Alumnos" -> "portal-alumnos").
pub fn slugify(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut prev_dash = false;
    for ch in input.trim().chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash && !out.is_empty() {
            out.push('-');
            prev_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        "sin-nombre".to_string()
    } else {
        out
    }
}

fn generate_token() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 24];
    rand::thread_rng().fill_bytes(&mut bytes);
    format!("dk_{}", to_hex(&bytes))
}

fn hash_key(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    to_hex(&hasher.finalize())
}

fn to_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write;
        let _ = write!(s, "{:02x}", b);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use diario_shared::NewAttachment;

    fn sample_entry(app: &str, title: &str) -> NewEntry {
        NewEntry {
            application: app.to_string(),
            agent: "claude-code".to_string(),
            model: Some("fable-5".to_string()),
            title: title.to_string(),
            prompt: "haz algo".to_string(),
            task_summary: Some("resumen".to_string()),
            response_markdown: "# Resultado\n\n```mermaid\ngraph TD; A-->B;\n```".to_string(),
            tags: vec!["refactor".to_string(), "api".to_string()],
            attachments: vec![NewAttachment {
                filename: "diseno.md".to_string(),
                kind: Some("design".to_string()),
                content_markdown: "## Diseno".to_string(),
            }],
            tokens_input: Some(100),
            tokens_output: Some(200),
            duration_ms: Some(1500),
            metadata: Some(serde_json::json!({"branch": "main"})),
        }
    }

    #[test]
    fn slugify_works() {
        assert_eq!(slugify("Portal Alumnos"), "portal-alumnos");
        assert_eq!(slugify("  UACloud 2026!! "), "uacloud-2026");
        assert_eq!(slugify("///"), "sin-nombre");
    }

    #[test]
    fn create_and_get_entry() {
        let store = Store::in_memory().unwrap();
        let id = store
            .create_entry(&sample_entry("Portal Alumnos", "Primera tarea"), Utc::now())
            .unwrap();
        let entry = store.get_entry(id).unwrap().unwrap();
        assert_eq!(entry.title, "Primera tarea");
        assert_eq!(entry.application_slug, "portal-alumnos");
        assert_eq!(entry.tags, vec!["api", "refactor"]);
        assert_eq!(entry.attachments.len(), 1);
        assert!(entry.response_html.contains("language-mermaid"));
        assert_eq!(entry.metadata.unwrap()["branch"], "main");
    }

    #[test]
    fn applications_are_deduplicated_by_slug() {
        let store = Store::in_memory().unwrap();
        store.create_entry(&sample_entry("Portal Alumnos", "t1"), Utc::now()).unwrap();
        store.create_entry(&sample_entry("portal alumnos", "t2"), Utc::now()).unwrap();
        let apps = store.list_applications().unwrap();
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].entry_count, 2);
    }

    #[test]
    fn query_filters_by_application_and_tag() {
        let store = Store::in_memory().unwrap();
        store.create_entry(&sample_entry("App A", "a1"), Utc::now()).unwrap();
        store.create_entry(&sample_entry("App B", "b1"), Utc::now()).unwrap();

        let q = EntryQuery {
            application: Some("app-a".into()),
            ..Default::default()
        };
        let page = store.query_entries(&q).unwrap();
        assert_eq!(page.entries.len(), 1);
        assert_eq!(page.entries[0].title, "a1");

        let q2 = EntryQuery {
            tag: Some("refactor".into()),
            ..Default::default()
        };
        assert_eq!(store.query_entries(&q2).unwrap().entries.len(), 2);

        let q3 = EntryQuery {
            tag: Some("inexistente".into()),
            ..Default::default()
        };
        assert_eq!(store.query_entries(&q3).unwrap().entries.len(), 0);
    }

    #[test]
    fn query_pagination_cursor() {
        let store = Store::in_memory().unwrap();
        for i in 0..5 {
            store.create_entry(&sample_entry("App", &format!("t{i}")), Utc::now()).unwrap();
        }
        let q = EntryQuery {
            limit: Some(2),
            ..Default::default()
        };
        let page = store.query_entries(&q).unwrap();
        assert_eq!(page.entries.len(), 2);
        let cursor = page.next_cursor.unwrap();
        let q2 = EntryQuery {
            limit: Some(2),
            cursor: Some(cursor),
            ..Default::default()
        };
        let page2 = store.query_entries(&q2).unwrap();
        assert_eq!(page2.entries.len(), 2);
        assert!(page2.entries[0].id < cursor);
    }

    #[test]
    fn full_text_search() {
        let store = Store::in_memory().unwrap();
        let mut e = sample_entry("App", "Migracion Oracle");
        e.response_markdown = "Actualizamos el paquete PL/SQL".into();
        store.create_entry(&e, Utc::now()).unwrap();

        let q = EntryQuery { q: Some("PL/SQL".into()), ..Default::default() };
        assert_eq!(store.query_entries(&q).unwrap().entries.len(), 1);
        let q2 = EntryQuery { q: Some("kubernetes".into()), ..Default::default() };
        assert_eq!(store.query_entries(&q2).unwrap().entries.len(), 0);
    }

    #[test]
    fn api_key_lifecycle() {
        let store = Store::in_memory().unwrap();
        assert_eq!(store.active_key_count().unwrap(), 0);
        let (id, token) = store.create_api_key("agente-1", "write").unwrap();
        assert!(token.starts_with("dk_"));
        assert_eq!(store.active_key_count().unwrap(), 1);

        let info = store.verify_api_key(&token).unwrap().unwrap();
        assert_eq!(info.scope, "write");
        assert!(store.verify_api_key("dk_falso").unwrap().is_none());

        assert!(store.revoke_api_key(id).unwrap());
        assert!(store.verify_api_key(&token).unwrap().is_none());
        assert_eq!(store.active_key_count().unwrap(), 0);
    }

    #[test]
    fn day_counts_group_by_day() {
        let store = Store::in_memory().unwrap();
        store.create_entry(&sample_entry("App", "t1"), Utc::now()).unwrap();
        store.create_entry(&sample_entry("App", "t2"), Utc::now()).unwrap();
        let counts = store.day_counts(None, None, None).unwrap();
        assert_eq!(counts.len(), 1);
        assert_eq!(counts[0].count, 2);
    }
}
