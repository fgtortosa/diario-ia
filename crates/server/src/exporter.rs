//! Escritura de las entradas del diario como ficheros markdown en los
//! repositorios.
//!
//! La regla que lo hace seguro: **solo se crean ficheros nuevos, nunca se
//! modifica uno existente**. De ahi salen las dos propiedades que motivan todo
//! el diseno: cero conflictos de merge por construccion, porque cada rama crea
//! ficheros con nombres que no pueden coincidir; y que nada se corrompe si el
//! escritor falla a mitad, porque como mucho falta un fichero y la siguiente
//! pasada lo escribe.

use crate::storage::Store;
use chrono::Local;
use diario_shared::{markdown_de, Entry};
use std::io::Write;
use std::path::{Path, PathBuf};

/// `20260917-125851-redes-ice-netcore-19.md`
///
/// Fecha y hora primero para que el orden alfabetico sea el cronologico; la
/// aplicacion para que el fichero se explique solo fuera de su carpeta; y el id
/// porque varias entradas seguidas caen en el mismo segundo y sin el se pisan.
pub fn nombre_fichero(entry: &Entry) -> String {
    format!(
        "{}-{}-{}.md",
        entry.created_at.with_timezone(&Local).format("%Y%m%d-%H%M%S"),
        entry.application_slug,
        entry.id
    )
}


/// Escribe todas las entradas pendientes y devuelve cuantas se marcaron.
///
/// Procesa todas y no solo la recien creada a proposito: si el servidor estuvo
/// parado, si un repositorio no existia todavia o si el disco estaba lleno, la
/// siguiente llamada arrastra lo atrasado sin que nadie intervenga.
pub fn exportar_pendientes(store: &Store, tareas_dir: Option<&str>) -> anyhow::Result<usize> {
    let mut marcadas = 0usize;
    let mut despues_de = None;

    loop {
        let pendientes = store.entradas_pendientes_despues(200, despues_de)?;
        if pendientes.is_empty() {
            break;
        }
        for entrada in pendientes {
            despues_de = Some(entrada.id);
        let mut destinos: Vec<PathBuf> = Vec::new();

        if let Some(repo) = store.repo_path_de_slug(&entrada.application_slug)? {
            destinos.push(Path::new(&repo).join("diario-ia"));
        }
        if let Some(central) = tareas_dir.filter(|t| !t.is_empty()) {
            destinos.push(PathBuf::from(central));
        }

        let nombre = nombre_fichero(&entrada);
        let cuerpo = markdown_de(&entrada);

let mut todos_ok = !destinos.is_empty();
        for dir in &destinos {
            if let Err(e) = escribir_si_no_existe(dir, &nombre, &cuerpo) {
                tracing::warn!(
                    "no se pudo exportar la entrada {} a {}: {e}",
                    entrada.id,
                    dir.display()
                );
                todos_ok = false;
            }
        }

        // Se marca solo si TODOS los destinos fueron bien. Si uno falla, la
        // entrada sigue pendiente y se reintenta entera.
        if todos_ok {
                store.marcar_exportada(entrada.id, chrono::Utc::now())?;
                marcadas += 1;
            }
        }
    }

    Ok(marcadas)
}

/// Crea el fichero solo si no existe.
///
/// Saltarselo no es una optimizacion, es lo que hace correcto el reintento: una
/// entrada tiene dos destinos que pueden fallar por separado, y sin esta
/// comprobacion el que fue bien se escribiria por duplicado al reintentar.
fn escribir_si_no_existe(dir: &Path, nombre: &str, cuerpo: &str) -> anyhow::Result<()> {
    std::fs::create_dir_all(dir)?;
    let destino = dir.join(nombre);
    if destino.exists() {
        return Ok(());
    }
    let temporal = dir.join(format!(".{nombre}.{}.tmp", uuid::Uuid::new_v4()));
    let resultado = (|| -> std::io::Result<()> {
        let mut fichero = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporal)?;
        fichero.write_all(cuerpo.as_bytes())?;
        fichero.sync_all()?;
        drop(fichero);
        // hard_link publica sin reemplazar: si otro escritor ya creó el
        // destino, falla con AlreadyExists en vez de modificarlo.
        std::fs::hard_link(&temporal, &destino)?;
        Ok(())
    })();
    match resultado {
        Ok(()) => {
            let _ = std::fs::remove_file(&temporal);
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            let _ = std::fs::remove_file(&temporal);
            Ok(())
        }
        Err(e) => {
            let _ = std::fs::remove_file(&temporal);
            Err(e.into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    // Entry no deriva Default, asi que se construye entera.
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
            tags: vec!["git".into(), "limpieza".into()],
            tokens_input: None,
            tokens_output: None,
            duration_ms: None,
            metadata: None,
            created_at: chrono::Utc.with_ymd_and_hms(2026, 9, 17, 12, 58, 51).unwrap(),
            exported_at: None,
            attachments: vec![],
        }
    }

    #[test]
    fn el_nombre_lleva_fecha_hora_aplicacion_e_id() {
        assert_eq!(
            nombre_fichero(&entrada_de_prueba()),
            // En hora local: la entrada de prueba es 12:58:51 UTC, y el nombre
            // depende de la zona de la maquina, asi que se compara contra el
            // mismo calculo en vez de contra una cadena fija.
            format!(
                "{}-redes-ice-netcore-19.md",
                entrada_de_prueba()
                    .created_at
                    .with_timezone(&chrono::Local)
                    .format("%Y%m%d-%H%M%S")
            )
        );
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
        assert!(md.contains("git, limpieza"));
    }

    use diario_shared::NewEntry;

    // NewEntry tampoco deriva Default.
    fn nueva(app: &str, titulo: &str) -> NewEntry {
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

    fn cuantos(dir: &std::path::Path) -> usize {
        std::fs::read_dir(dir).map(|d| d.count()).unwrap_or(0)
    }

    #[test]
    fn escribe_en_el_repo_y_en_el_directorio_central() {
        let repo = tempfile::tempdir().unwrap();
        let central = tempfile::tempdir().unwrap();
        let store = Store::in_memory().unwrap();
        store.create_entry(&nueva("mi-app", "Titulo"), chrono::Utc::now()).unwrap();
        store.set_repo_path("mi-app", Some(repo.path().to_str().unwrap())).unwrap();

        let n = exportar_pendientes(&store, central.path().to_str()).unwrap();

        assert_eq!(n, 1);
        assert_eq!(cuantos(&repo.path().join("diario-ia")), 1);
        assert_eq!(cuantos(central.path()), 1);
        assert!(store.entradas_pendientes(10).unwrap().is_empty());
    }

    #[test]
    fn sin_repo_path_escribe_solo_en_el_central() {
        let central = tempfile::tempdir().unwrap();
        let store = Store::in_memory().unwrap();
        store.create_entry(&nueva("sin-repo", "Titulo"), chrono::Utc::now()).unwrap();

        let n = exportar_pendientes(&store, central.path().to_str()).unwrap();

        assert_eq!(n, 1);
        assert_eq!(cuantos(central.path()), 1);
    }

    #[test]
    fn un_repo_path_invalido_deja_la_entrada_pendiente() {
        let central = tempfile::tempdir().unwrap();
        let store = Store::in_memory().unwrap();
        store.create_entry(&nueva("mi-app", "Titulo"), chrono::Utc::now()).unwrap();
        // Un hijo bajo un fichero existente falla en todos los sistemas.
        let archivo = tempfile::NamedTempFile::new().unwrap();
        store.set_repo_path("mi-app", Some(archivo.path().to_str().unwrap())).unwrap();

        let n = exportar_pendientes(&store, central.path().to_str()).unwrap();

        assert_eq!(n, 0);
        assert_eq!(store.entradas_pendientes(10).unwrap().len(), 1);
    }

    #[test]
    fn escribir_si_no_existe_no_pisa_lo_ya_escrito() {
        // Es la semantica que hace correcto el reintento: si un destino fue bien
        // y otro fallo, al reintentar el que fue bien NO se vuelve a escribir.
        let dir = tempfile::tempdir().unwrap();

        escribir_si_no_existe(dir.path(), "entrada.md", "primera").unwrap();
        escribir_si_no_existe(dir.path(), "entrada.md", "SEGUNDA").unwrap();

        assert_eq!(cuantos(dir.path()), 1);
        assert_eq!(
            std::fs::read_to_string(dir.path().join("entrada.md")).unwrap(),
            "primera"
        );
    }

    #[test]
    fn escribir_si_no_existe_crea_el_directorio() {
        let base = tempfile::tempdir().unwrap();
        let anidado = base.path().join("repo").join("diario-ia");

        escribir_si_no_existe(&anidado, "entrada.md", "contenido").unwrap();

        assert!(anidado.join("entrada.md").exists());
    }

    #[test]
    fn procesa_todas_las_pendientes_no_solo_la_ultima() {
        let central = tempfile::tempdir().unwrap();
        let store = Store::in_memory().unwrap();
        for i in 0..3 {
            store.create_entry(&nueva("mi-app", &format!("Titulo {i}")), chrono::Utc::now()).unwrap();
        }

        assert_eq!(exportar_pendientes(&store, central.path().to_str()).unwrap(), 3);
        assert_eq!(cuantos(central.path()), 3);
    }

    #[test]
    fn drena_mas_de_un_lote_de_pendientes() {
        let central = tempfile::tempdir().unwrap();
        let store = Store::in_memory().unwrap();
        for i in 0..201 {
            store.create_entry(&nueva("mi-app", &format!("Titulo {i}")), chrono::Utc::now()).unwrap();
        }

        assert_eq!(exportar_pendientes(&store, central.path().to_str()).unwrap(), 201);
        assert!(store.entradas_pendientes(1).unwrap().is_empty());
    }

    #[test]
    fn sin_directorio_central_no_escribe_destino_central() {
        let repo = tempfile::tempdir().unwrap();
        let store = Store::in_memory().unwrap();
        store.create_entry(&nueva("mi-app", "Titulo"), chrono::Utc::now()).unwrap();
        store.set_repo_path("mi-app", Some(repo.path().to_str().unwrap())).unwrap();

        assert_eq!(exportar_pendientes(&store, None).unwrap(), 1);
        assert_eq!(cuantos(&repo.path().join("diario-ia")), 1);
    }
}
