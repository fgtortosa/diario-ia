//! Diario-IA: servidor central (REST + SPA + MCP) en un unico binario.

mod api;
mod auth;
mod config;
mod error;
mod mcp;
mod render;
mod state;
mod static_files;
mod storage;

use std::net::SocketAddr;
use std::sync::Arc;

use clap::{Parser, Subcommand};

use crate::config::ServerConfig;
use crate::state::AppState;
use crate::storage::Store;

#[derive(Parser)]
#[command(name = "diario", version, about = "Diario de tareas de agentes de IA")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Arranca el servidor central (REST + SPA). [por defecto]
    Serve(ServeArgs),
    /// Servidor MCP por stdio que reenvia al servidor central via REST.
    Mcp(McpArgs),
    /// Gestion de API keys.
    Key {
        #[command(subcommand)]
        action: KeyAction,
    },
    /// Crea/actualiza el esquema de la base de datos y sale.
    Migrate(DbArgs),
    /// Exporta las entradas a ficheros markdown en un directorio.
    Export(ExportArgs),
}

#[derive(Parser)]
struct ServeArgs {
    /// Direccion de escucha.
    #[arg(long, env = "DIARIO_BIND", default_value = "0.0.0.0:8787")]
    bind: SocketAddr,
    /// Ruta del fichero SQLite.
    #[arg(long, env = "DIARIO_DB", default_value = "diario.db")]
    db: String,
    /// URL publica base para los enlaces a entradas.
    #[arg(long, env = "DIARIO_PUBLIC_URL", default_value = "http://localhost:8787")]
    public_url: String,
    /// Token de lectura opcional (protege GET/SPA). Vacio = lectura libre.
    #[arg(long, env = "DIARIO_VIEWER_TOKEN")]
    viewer_token: Option<String>,
}

#[derive(Parser)]
struct McpArgs {
    /// URL del servidor central.
    #[arg(long, env = "DIARIO_URL", default_value = "http://localhost:8787")]
    url: String,
    /// API key para escribir en el diario.
    #[arg(long, env = "DIARIO_KEY")]
    key: Option<String>,
}

#[derive(Parser)]
struct DbArgs {
    #[arg(long, env = "DIARIO_DB", default_value = "diario.db")]
    db: String,
}

#[derive(Parser)]
struct ExportArgs {
    #[arg(long, env = "DIARIO_DB", default_value = "diario.db")]
    db: String,
    /// Directorio destino.
    #[arg(long, default_value = "export")]
    out: String,
}

#[derive(Subcommand)]
enum KeyAction {
    /// Crea una API key nueva (muestra el token una sola vez).
    Create {
        name: String,
        #[arg(long, default_value = "write")]
        scope: String,
        #[arg(long, env = "DIARIO_DB", default_value = "diario.db")]
        db: String,
    },
    /// Lista las API keys.
    List {
        #[arg(long, env = "DIARIO_DB", default_value = "diario.db")]
        db: String,
    },
    /// Revoca (desactiva) una API key por id.
    Revoke {
        id: i64,
        #[arg(long, env = "DIARIO_DB", default_value = "diario.db")]
        db: String,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command.unwrap_or(Command::Serve(ServeArgs::parse_from(["diario"]))) {
        Command::Serve(args) => run_server(args),
        Command::Mcp(args) => run_async(mcp::run(args.url, args.key)),
        Command::Key { action } => run_key(action),
        Command::Migrate(args) => {
            Store::open(&args.db)?;
            println!("Esquema actualizado en {}", args.db);
            Ok(())
        }
        Command::Export(args) => run_export(args),
    }
}

fn init_tracing() {
    use tracing_subscriber::{fmt, EnvFilter};
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,diario_server=info"));
    fmt().with_env_filter(filter).with_target(false).init();
}

fn run_async<F: std::future::Future<Output = anyhow::Result<()>>>(fut: F) -> anyhow::Result<()> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(fut)
}

fn run_server(args: ServeArgs) -> anyhow::Result<()> {
    init_tracing();
    let config = ServerConfig {
        bind: args.bind,
        db_path: args.db.clone(),
        public_url: args.public_url,
        viewer_token: args.viewer_token.filter(|t| !t.is_empty()),
    };
    let store = Store::open(&config.db_path)?;
    let state = AppState {
        store,
        config: Arc::new(config.clone()),
    };
    run_async(async move {
        let app = api::router(state);
        let listener = tokio::net::TcpListener::bind(config.bind).await?;
        tracing::info!("Diario-IA escuchando en http://{}", config.bind);
        axum::serve(listener, app).await?;
        Ok(())
    })
}

fn run_key(action: KeyAction) -> anyhow::Result<()> {
    match action {
        KeyAction::Create { name, scope, db } => {
            let store = Store::open(&db)?;
            let (id, token) = store.create_api_key(&name, &scope)?;
            println!("API key creada (id={id}, scope={scope}).");
            println!("Guardala ahora, no se volvera a mostrar:\n\n  {token}\n");
        }
        KeyAction::List { db } => {
            let store = Store::open(&db)?;
            let keys = store.list_api_keys()?;
            if keys.is_empty() {
                println!("No hay API keys.");
            }
            for k in keys {
                println!(
                    "#{:<3} {:<20} scope={:<6} activa={} creada={} ultimo_uso={}",
                    k.id,
                    k.name,
                    k.scope,
                    if k.active { "si" } else { "no" },
                    k.created_at,
                    k.last_used_at.as_deref().unwrap_or("-")
                );
            }
        }
        KeyAction::Revoke { id, db } => {
            let store = Store::open(&db)?;
            if store.revoke_api_key(id)? {
                println!("API key #{id} revocada.");
            } else {
                println!("No existe la API key #{id}.");
            }
        }
    }
    Ok(())
}

fn run_export(args: ExportArgs) -> anyhow::Result<()> {
    use diario_shared::EntryQuery;
    use std::io::Write;

    let store = Store::open(&args.db)?;
    std::fs::create_dir_all(&args.out)?;

    let mut cursor = None;
    let mut total = 0usize;
    loop {
        let page = store.query_entries(&EntryQuery {
            limit: Some(200),
            cursor,
            ..Default::default()
        })?;
        if page.entries.is_empty() {
            break;
        }
        for summary in &page.entries {
            if let Some(entry) = store.get_entry(summary.id)? {
                let dir = std::path::Path::new(&args.out).join(&entry.application_slug);
                std::fs::create_dir_all(&dir)?;
                let day = entry.created_at.format("%Y%m%d-%H%M%S");
                let path = dir.join(format!("{}-{}.md", day, entry.id));
                let mut f = std::fs::File::create(&path)?;
                writeln!(f, "# {}\n", entry.title)?;
                writeln!(f, "- Aplicacion: {}", entry.application_name)?;
                writeln!(f, "- Agente: {} ({})", entry.agent_name, entry.model.as_deref().unwrap_or("-"))?;
                writeln!(f, "- Fecha: {}\n", entry.created_at.to_rfc3339())?;
                writeln!(f, "## Prompt\n\n{}\n", entry.prompt)?;
                if let Some(ts) = &entry.task_summary {
                    writeln!(f, "## Tarea\n\n{}\n", ts)?;
                }
                writeln!(f, "## Respuesta\n\n{}\n", entry.response_markdown)?;
                total += 1;
            }
        }
        cursor = page.next_cursor;
        if cursor.is_none() {
            break;
        }
    }
    println!("Exportadas {total} entradas a {}", args.out);
    Ok(())
}
