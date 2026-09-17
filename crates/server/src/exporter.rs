//! Escritura de las entradas del diario como ficheros markdown en los
//! repositorios.
//!
//! La regla que lo hace seguro: **solo se crean ficheros nuevos, nunca se
//! modifica uno existente**. De ahi salen las dos propiedades que motivan todo
//! el diseno: cero conflictos de merge por construccion, porque cada rama crea
//! ficheros con nombres que no pueden coincidir; y que nada se corrompe si el
//! escritor falla a mitad, porque como mucho falta un fichero y la siguiente
//! pasada lo escribe.

use diario_shared::Entry;

/// `20260917-125851-redes-ice-netcore-19.md`
///
/// Fecha y hora primero para que el orden alfabetico sea el cronologico; la
/// aplicacion para que el fichero se explique solo fuera de su carpeta; y el id
/// porque varias entradas seguidas caen en el mismo segundo y sin el se pisan.
pub fn nombre_fichero(entry: &Entry) -> String {
    format!(
        "{}-{}-{}.md",
        entry.created_at.format("%Y%m%d-%H%M%S"),
        entry.application_slug,
        entry.id
    )
}

/// El markdown que se escribe en el repositorio. El titulo va como H1 y el
/// prompt literal, sin parafrasear, que es lo que le da valor al registro.
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
    s.push_str(&format!("- Entrada: {}\n", entry.id));
    if !entry.tags.is_empty() {
        s.push_str(&format!("- Etiquetas: {}\n", entry.tags.join(", ")));
    }
    s.push('\n');
    s.push_str(&format!("## Prompt\n\n{}\n\n", entry.prompt));
    if let Some(resumen) = &entry.task_summary {
        s.push_str(&format!("## Resumen\n\n{}\n\n", resumen));
    }
    s.push_str(&format!("## Respuesta\n\n{}\n", entry.response_markdown));
    s
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
            attachments: vec![],
        }
    }

    #[test]
    fn el_nombre_lleva_fecha_hora_aplicacion_e_id() {
        assert_eq!(
            nombre_fichero(&entrada_de_prueba()),
            "20260917-125851-redes-ice-netcore-19.md"
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
}
