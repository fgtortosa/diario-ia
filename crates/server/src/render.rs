//! Renderizado de markdown a HTML seguro.
//!
//! El markdown proviene de agentes de IA, asi que el HTML resultante se
//! sanitiza con `ammonia` (elimina <script>, manejadores de eventos, etc.).
//! Los bloques ```mermaid``` se dejan como `<code class="language-mermaid">`;
//! el cliente los detecta y los renderiza con mermaid.js leyendo su textContent
//! (evitando problemas de escapado de entidades HTML).

use comrak::Options;

/// Convierte markdown en HTML sanitizado listo para inyectar en el DOM.
pub fn render_markdown(md: &str) -> String {
    let mut options = Options::default();
    options.extension.strikethrough = true;
    options.extension.table = true;
    options.extension.autolink = true;
    options.extension.tasklist = true;
    options.extension.footnotes = true;
    options.extension.superscript = true;
    // Dejamos pasar HTML crudo; la seguridad la garantiza ammonia despues.
    options.render.unsafe_ = true;

    let raw_html = comrak::markdown_to_html(md, &options);
    sanitize(&raw_html)
}

/// Sanitiza HTML permitiendo `class` (para `language-*` de highlight/mermaid).
fn sanitize(html: &str) -> String {
    ammonia::Builder::default()
        // Permitimos class en todas las etiquetas admitidas para conservar
        // `language-rust`, `language-mermaid`, etc.
        .add_generic_attributes(["class"])
        .clean(html)
        .to_string()
}

/// Genera un fragmento de texto plano (sin markdown) para los resumenes.
pub fn snippet(md: &str, max_chars: usize) -> String {
    let text: String = md
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        // Quitamos marcadores markdown mas comunes al inicio de linea.
        .map(|l| l.trim_start_matches(['#', '>', '-', '*', '`', ' ']))
        .collect::<Vec<_>>()
        .join(" ");
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.chars().count() <= max_chars {
        text
    } else {
        let truncated: String = text.chars().take(max_chars).collect();
        format!("{}…", truncated.trim_end())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_basic_markdown() {
        let html = render_markdown("# Hola\n\nMundo **negrita**");
        assert!(html.contains("<h1>"));
        assert!(html.contains("<strong>negrita</strong>"));
    }

    #[test]
    fn strips_script_tags() {
        let html = render_markdown("Texto <script>alert(1)</script> fin");
        assert!(!html.contains("<script"));
        assert!(!html.to_lowercase().contains("alert(1)") || !html.contains("<script"));
    }

    #[test]
    fn keeps_mermaid_language_class() {
        let md = "```mermaid\ngraph TD; A-->B;\n```";
        let html = render_markdown(md);
        assert!(html.contains("language-mermaid"));
    }

    #[test]
    fn snippet_truncates() {
        let s = snippet("# Titulo\n\nEste es un texto largo de ejemplo", 10);
        assert!(s.chars().count() <= 11); // 10 + el caracter de elipsis
    }
}
