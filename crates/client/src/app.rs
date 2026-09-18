//! Componentes de la SPA: cabecera, sidebar, timeline y detalle de entrada.

use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;

use diario_shared::{
    markdown_de, query_a, query_de, Application, Entry, EntryQuery, EntrySummary, TagCount,
};

use crate::api;
use crate::ffi::{
    copiar_al_portapapeles, current_path, current_search, descargar_fichero, hay_portapapeles,
    imprimir, on_popstate, push_path, render_diagrams,
};

#[derive(Clone, Copy, PartialEq)]
pub enum View {
    List,
    Detail(i64),
    /// Todo lo que cumple el filtro, entero y en una sola pagina.
    Hilo,
}

fn parse_route(path: &str) -> View {
    if let Some(rest) = path.strip_prefix("/entry/") {
        if let Ok(id) = rest.trim_end_matches('/').parse::<i64>() {
            return View::Detail(id);
        }
    }
    if path.trim_end_matches('/') == "/hilo" {
        return View::Hilo;
    }
    View::List
}

/// Ruta que le corresponde a cada vista.
///
/// El detalle tambien arrastra los filtros aunque no los use: si su URL no los
/// llevara, volver con el boton de atras desde una entrada los borraria, porque
/// popstate los lee de la query.
fn ruta_de(view: View) -> String {
    match view {
        View::Hilo => "/hilo".to_string(),
        View::Detail(id) => format!("/entry/{id}"),
        View::List => "/".to_string(),
    }
}

/// Escribe la vista y los filtros en la barra de direcciones.
///
/// No apila nada si la URL ya dice lo mismo, y de eso depende que el boton de
/// atras funcione: al volver, popstate repone los filtros, el Effect recalcula
/// esta misma URL y no empuja una entrada nueva encima de la que se acaba de
/// dejar.
fn sincronizar_url(view: View, query: &str) {
    let ruta = ruta_de(view);
    let destino = if query.is_empty() {
        ruta
    } else {
        format!("{ruta}?{query}")
    };
    if destino != format!("{}{}", current_path(), current_search()) {
        push_path(&destino);
    }
}

/// Cambia de vista llevandose los filtros puestos.
fn ir_a(view: RwSignal<View>, destino: View, q: &EntryQuery) {
    view.set(destino);
    sincronizar_url(destino, &query_de(q));
}

#[component]
pub fn App() -> impl IntoView {
    // Los filtros salen de la URL, no de cero: asi un enlace pegado reproduce
    // la busqueda entera y no solo la vista.
    let inicial = query_a(&current_search());

    let apps = RwSignal::new(Vec::<Application>::new());
    let selected_app = RwSignal::new(inicial.application.clone());
    let from = RwSignal::new(inicial.from.clone().unwrap_or_default());
    let to = RwSignal::new(inicial.to.clone().unwrap_or_default());
    let search = RwSignal::new(inicial.q.clone().unwrap_or_default());
    let selected_tag = RwSignal::new(inicial.tag.clone());
    let tags = RwSignal::new(Vec::<TagCount>::new());
    let solo_pendientes = RwSignal::new(inicial.exported == Some(false));
    let entries = RwSignal::new(Vec::<EntrySummary>::new());
    let loading = RwSignal::new(false);
    let view = RwSignal::new(parse_route(&current_path()));

    // Un solo sitio donde se juntan los filtros: lo leen el listado, el hilo y
    // la URL, asi que no pueden acabar diciendo cosas distintas.
    let consulta = Memo::new(move |_| EntryQuery {
        application: selected_app.get().filter(|s| !s.is_empty()),
        from: Some(from.get()).filter(|s| !s.is_empty()),
        to: Some(to.get()).filter(|s| !s.is_empty()),
        q: Some(search.get()).filter(|s| !s.is_empty()),
        tag: selected_tag.get().filter(|s| !s.is_empty()),
        exported: solo_pendientes.get().then_some(false),
        ..Default::default()
    });

    // El boton de atras repone vista y filtros a la vez: la URL los lleva los
    // dos, asi que restaurar solo uno dejaria la pagina mintiendo.
    on_popstate(move || {
        view.set(parse_route(&current_path()));
        let q = query_a(&current_search());
        selected_app.set(q.application.clone());
        from.set(q.from.clone().unwrap_or_default());
        to.set(q.to.clone().unwrap_or_default());
        search.set(q.q.clone().unwrap_or_default());
        selected_tag.set(q.tag.clone());
        solo_pendientes.set(q.exported == Some(false));
    });

    spawn_local(async move {
        if let Ok(a) = api::fetch_applications().await {
            apps.set(a);
        }
    });

    spawn_local(async move {
        if let Ok(t) = api::fetch_tags().await {
            tags.set(t);
        }
    });

    // Recarga las entradas cuando cambia cualquier filtro.
    Effect::new(move |anterior: Option<()>| {
        let f = consulta.get();

        // Al cambiar un filtro se vuelve al listado. Los resultados solo se
        // pintan en View::List, asi que desde el detalle se filtraba a ciegas:
        // la barra lateral y las fechas funcionaban, pero no se veia nada y
        // parecia que estaban rotas. El hilo si los pinta, y se queda.
        //
        // La guarda 'anterior.is_some()' no es cosmetica: este Effect tambien
        // corre al montar, y sin ella un enlace directo a /entry/N rebotaria al
        // listado antes de que se llegara a ver la entrada.
        //
        // view se lee sin rastrear a proposito: rastrearlo haria que el Effect
        // se reejecutara al cambiar de vista y, con el set de aqui dentro, seria
        // un bucle.
        if anterior.is_some() && matches!(view.get_untracked(), View::Detail(_)) {
            view.set(View::List);
        }
        sincronizar_url(view.get_untracked(), &query_de(&f));

        loading.set(true);
        spawn_local(async move {
            match api::fetch_entries(&f).await {
                Ok(page) => entries.set(page.entries),
                Err(_) => entries.set(Vec::new()),
            }
            loading.set(false);
        });
    });

    view! {
        <div class="app">
            <Header search=search selected_tag=selected_tag />
            <Sidebar apps=apps selected_app=selected_app from=from to=to selected_tag=selected_tag tags=tags solo_pendientes=solo_pendientes consulta=consulta view=view />
            <main class="main">
                {move || match view.get() {
                    View::List => view! { <Timeline entries=entries loading=loading view=view selected_tag=selected_tag consulta=consulta /> }.into_any(),
                    View::Detail(id) => view! { <EntryDetail id=id view=view consulta=consulta /> }.into_any(),
                    View::Hilo => view! { <HiloView consulta=consulta view=view /> }.into_any(),
                }}
            </main>
        </div>
    }
}

#[component]
fn Header(search: RwSignal<String>, selected_tag: RwSignal<Option<String>>) -> impl IntoView {
    view! {
        <header class="header">
            <h1><span class="logo">"📓"</span> "Diario-IA"</h1>
            <span class="spacer"></span>
            <input
                class="search"
                type="search"
                placeholder="Buscar en prompts, tareas y respuestas…"
                prop:value=move || search.get()
                on:input=move |ev| search.set(event_target_value(&ev))
            />
            {move || selected_tag.get().map(|etiqueta| view! {
                <button
                    class="tag tag-activo"
                    title="Quitar el filtro de etiqueta"
                    on:click=move |_| selected_tag.set(None)
                >{format!("#{etiqueta} ×")}</button>
            })}
        </header>
    }
}

#[component]
fn Sidebar(
    apps: RwSignal<Vec<Application>>,
    selected_app: RwSignal<Option<String>>,
    from: RwSignal<String>,
    to: RwSignal<String>,
    selected_tag: RwSignal<Option<String>>,
    tags: RwSignal<Vec<TagCount>>,
    solo_pendientes: RwSignal<bool>,
    consulta: Memo<EntryQuery>,
    view: RwSignal<View>,
) -> impl IntoView {
    let estado_export = RwSignal::new(String::new());
    view! {
        <aside class="sidebar">
            <h2>"Aplicaciones"</h2>
            <ul class="app-list">
                <li
                    class=move || if selected_app.get().is_none() { "active" } else { "" }
                    on:click=move |_| selected_app.set(None)
                >
                    <span>"Todas"</span>
                </li>
                {move || {
                    apps.get()
                        .into_iter()
                        .map(|a| {
                            let slug = a.slug.clone();
                            let slug_active = a.slug.clone();
                            let is_active = move || selected_app.get().as_deref() == Some(slug_active.as_str());
                            view! {
                                <li
                                    class=move || if is_active() { "active" } else { "" }
                                    on:click=move |_| selected_app.set(Some(slug.clone()))
                                >
                                    <span>{a.name.clone()}</span>
                                    <span class="count">{a.entry_count}</span>
                                </li>
                            }
                        })
                        .collect_view()
                }}
            </ul>
            <h2>"Etiquetas"</h2>
            <ul class="app-list">
                {move || {
                    tags.get()
                        .into_iter()
                        .map(|t| {
                            let nombre = t.tag.clone();
                            let activa = nombre.clone();
                            let clase = move || {
                                if selected_tag.get().as_deref() == Some(activa.as_str()) {
                                    "active"
                                } else {
                                    ""
                                }
                            };
                            view! {
                                <li
                                    class=clase
                                    on:click=move |_| selected_tag.set(Some(nombre.clone()))
                                >
                                    <span>{t.tag.clone()}</span>
                                    <span class="count">{t.count}</span>
                                </li>
                            }
                        })
                        .collect_view()
                }}
            </ul>
            <h2>"Fechas"</h2>
            <div class="filters">
                <label>"Desde"</label>
                <input
                    type="date"
                    prop:value=move || from.get()
                    on:input=move |ev| from.set(event_target_value(&ev))
                />
                <label>"Hasta"</label>
                <input
                    type="date"
                    prop:value=move || to.get()
                    on:input=move |ev| to.set(event_target_value(&ev))
                />
                <label class="check">
                    <input
                        type="checkbox"
                        prop:checked=move || solo_pendientes.get()
                        on:change=move |ev| solo_pendientes.set(event_target_checked(&ev))
                    />
                    " Solo sin exportar"
                </label>
                <button
                    class="btn-clear"
                    title="Escribe ahora los markdown que aun no se han volcado a los repositorios"
                    on:click=move |_| {
                        estado_export.set("Exportando…".to_string());
                        spawn_local(async move {
                            match api::exportar_ahora().await {
                                Ok((escritas, pendientes)) => estado_export.set(
                                    if pendientes > 0 {
                                        format!("{escritas} escrita(s), {pendientes} sin destino")
                                    } else if escritas > 0 {
                                        format!("{escritas} escrita(s)")
                                    } else {
                                        "No quedaba nada".to_string()
                                    },
                                ),
                                Err(e) => estado_export.set(format!("Error: {e}")),
                            }
                        });
                    }
                >
                    "Exportar pendientes"
                </button>
                {move || {
                    let m = estado_export.get();
                    (!m.is_empty()).then(|| view! { <p class="estado-export">{m}</p> })
                }}
                <button
                    class="btn-clear btn-hilo"
                    title="Muestra en una sola pagina todo lo que cumple este filtro, listo para copiarselo a un agente"
                    on:click=move |_| ir_a(view, View::Hilo, &consulta.get_untracked())
                >
                    "Ver en hilo"
                </button>
                <button
                    class="btn-clear"
                    on:click=move |_| {
                        estado_export.set(String::new());
                        selected_app.set(None);
                        from.set(String::new());
                        to.set(String::new());
                        selected_tag.set(None);
                        solo_pendientes.set(false);
                    }
                >
                    "Limpiar filtros"
                </button>
            </div>
        </aside>
    }
}

#[component]
fn Timeline(
    entries: RwSignal<Vec<EntrySummary>>,
    loading: RwSignal<bool>,
    view: RwSignal<View>,
    selected_tag: RwSignal<Option<String>>,
    consulta: Memo<EntryQuery>,
) -> impl IntoView {
    move || {
        if loading.get() && entries.get().is_empty() {
            return view! { <p class="loading">"Cargando…"</p> }.into_any();
        }
        let items = entries.get();
        if items.is_empty() {
            return view! {
                <div class="empty">"No hay tareas registradas para este filtro."</div>
            }
            .into_any();
        }
        group_by_day(items)
            .into_iter()
            .map(|(day, list)| {
                view! {
                    <div class="day-group">
                        <p class="day-heading">{day}</p>
                        {list.into_iter().map(|e| entry_card(e, view, selected_tag, consulta)).collect_view()}
                    </div>
                }
            })
            .collect_view()
            .into_any()
    }
}

fn entry_card(
    e: EntrySummary,
    view: RwSignal<View>,
    selected_tag: RwSignal<Option<String>>,
    consulta: Memo<EntryQuery>,
) -> impl IntoView {
    let id = e.id;
    let time = e.created_at.format("%H:%M").to_string();
    let model_suffix = e
        .model
        .clone()
        .map(|m| format!(" · {m}"))
        .unwrap_or_default();
    let meta = format!("{}{} · {}", e.agent_name, model_suffix, time);
    let tags = e.tags.clone();
    view! {
        <div
            class="card"
            on:click=move |_| ir_a(view, View::Detail(id), &consulta.get_untracked())
        >
            <div class="row">
                <span class="badge">{e.application_name.clone()}</span>
                <span class="title">{e.title.clone()}</span>
            </div>
            <div class="row" style="margin-top:6px">
                <span class="meta">{meta}</span>
                {tags.into_iter().map(|etiqueta| {
                    let valor = etiqueta.clone();
                    view! {
                        <button
                            class="tag"
                            title="Filtrar por esta etiqueta"
                            on:click=move |ev| {
                                // Sin stop_propagation el clic llegaria tambien a
                                // la tarjeta y navegaria al detalle en vez de filtrar.
                                ev.stop_propagation();
                                selected_tag.set(Some(valor.clone()));
                            }
                        >{etiqueta}</button>
                    }
                }).collect_view()}
            </div>
            <p class="snippet">{e.snippet.clone()}</p>
        </div>
    }
}

#[component]
fn EntryDetail(id: i64, view: RwSignal<View>, consulta: Memo<EntryQuery>) -> impl IntoView {
    let entry = RwSignal::new(Option::<Entry>::None);

    spawn_local(async move {
        match api::fetch_entry(id).await {
            Ok(e) => entry.set(Some(e)),
            Err(_) => entry.set(None),
        }
    });

    // Tras inyectar el HTML de la respuesta, renderiza diagramas y resalta codigo.
    Effect::new(move |_| {
        if entry.get().is_some() {
            render_diagrams();
        }
    });

    view! {
        <div class="detail">
            <button
                class="back"
                on:click=move |_| ir_a(view, View::List, &consulta.get_untracked())
            >
                "← Volver"
            </button>
            {move || match entry.get() {
                None => view! { <p class="loading">"Cargando…"</p> }.into_any(),
                Some(e) => detail_body(e, entry).into_any(),
            }}
        </div>
    }
}

fn detail_body(e: Entry, entry: RwSignal<Option<Entry>>) -> impl IntoView {
    let date = e.created_at.format("%d/%m/%Y %H:%M").to_string();
    let model = e.model.clone().unwrap_or_else(|| "—".to_string());
    let meta = format!("{} · {}", e.agent_name, model);
    let tags = e.tags.clone();
    let summary = e.task_summary.clone().filter(|s| !s.is_empty());
    let attachments = e.attachments.clone();

    // El nombre lo da el servidor, no se calcula aqui: lleva la hora local, y
    // calcularlo tambien en el navegador daria otro nombre si las zonas no
    // coinciden, rompiendo la garantia de que descargar a mano y exportar
    // produzcan lo mismo.
    let nombre_md = e.export_filename.clone();
    let md = markdown_de(&e);
    let id = e.id;
    let exportada = e.exported_at;

    view! {
        <h1>{e.title.clone()}</h1>
        <div class="acciones">
            <button class="btn" title="Descargar esta entrada en markdown"
                on:click=move |_| descargar_fichero(&nombre_md, &md)>"Descargar .md"</button>
            <button class="btn" title="Imprimir o guardar como PDF"
                on:click=move |_| imprimir()>"PDF"</button>
            <span class="estado-export">
                {match exportada {
                    Some(cuando) => format!(
                        "Exportada el {}",
                        cuando.with_timezone(&chrono::Local).format("%d/%m/%Y %H:%M")
                    ),
                    None => "Sin exportar".to_string(),
                }}
            </span>
            <button
                class="btn"
                title=if exportada.is_some() {
                    "Marcarla como no exportada: el exportador la reescribira en la siguiente pasada, respetando el fichero si ya existe"
                } else {
                    "Marcarla como exportada sin escribir el fichero: quedara sin exportar de verdad"
                }
                on:click=move |_| {
                    let nuevo = exportada.is_none();
                    spawn_local(async move {
                        if api::set_exported(id, nuevo).await.is_ok() {
                            // Se recarga la entrada para que el indicador no mienta.
                            if let Ok(e) = api::fetch_entry(id).await {
                                entry.set(Some(e));
                            }
                        }
                    });
                }
            >
                {if exportada.is_some() { "Marcar sin exportar" } else { "Marcar exportada" }}
            </button>
        </div>
        <div class="meta-row">
            <span class="badge">{e.application_name.clone()}</span>
            <span class="meta">{meta}</span>
            <span class="meta">{date}</span>
            {tags.into_iter().map(|t| view! { <span class="tag">{t}</span> }).collect_view()}
        </div>
        <details class="prompt">
            <summary>"Ver prompt"</summary>
            <div class="prompt-box">{e.prompt.clone()}</div>
        </details>
        {summary
            .map(|s| {
                view! { <div class="section"><h3>"Tarea"</h3><div class="summary-box">{s}</div></div> }
            })}
        <div class="section">
            <h3>"Respuesta"</h3>
            <div class="markdown" inner_html=e.response_html.clone()></div>
        </div>
        {(!attachments.is_empty())
            .then(|| {
                view! {
                    <div class="section">
                        <h3>"Documentos"</h3>
                        {attachments
                            .into_iter()
                            .map(|a| {
                                view! {
                                    <div class="attachment">
                                        <h4>{a.filename.clone()}</h4>
                                        <div class="markdown" inner_html=a.content_html.clone()></div>
                                    </div>
                                }
                            })
                            .collect_view()}
                    </div>
                }
            })}
    }
}

fn group_by_day(items: Vec<EntrySummary>) -> Vec<(String, Vec<EntrySummary>)> {
    let mut out: Vec<(String, Vec<EntrySummary>)> = Vec::new();
    for e in items {
        let day = e.created_at.format("%d/%m/%Y").to_string();
        match out.last_mut() {
            Some((d, list)) if *d == day => list.push(e),
            _ => out.push((day, vec![e])),
        }
    }
    out
}

/// Como se describe un filtro en una linea: "diario-ia · #build · desde …".
fn descripcion_filtros(q: &EntryQuery) -> String {
    let mut partes = Vec::new();
    if let Some(a) = &q.application {
        partes.push(a.clone());
    }
    if let Some(t) = &q.tag {
        partes.push(format!("#{t}"));
    }
    if let Some(d) = &q.from {
        partes.push(format!("desde {d}"));
    }
    if let Some(h) = &q.to {
        partes.push(format!("hasta {h}"));
    }
    if let Some(b) = &q.q {
        partes.push(format!("\"{b}\""));
    }
    if q.exported == Some(false) {
        partes.push("sin exportar".to_string());
    }
    if partes.is_empty() {
        "todo el diario".to_string()
    } else {
        partes.join(" · ")
    }
}

fn cuantas(n: usize) -> String {
    if n == 1 {
        "1 entrada".to_string()
    } else {
        format!("{n} entradas")
    }
}

/// Markdown del hilo entero: una cabecera que dice de que busqueda salio y las
/// entradas separadas por una regla.
///
/// Cada entrada se arma con `markdown_de`, la misma funcion que escribe los
/// ficheros de los repositorios: lo que copias aqui y lo que hay en disco son
/// el mismo texto por construccion.
fn markdown_del_hilo(q: &EntryQuery, entradas: &[Entry]) -> String {
    let mut s = format!(
        "# Diario-IA — {} ({})\n\n",
        descripcion_filtros(q),
        cuantas(entradas.len())
    );
    s.push_str(
        &entradas
            .iter()
            .map(markdown_de)
            .collect::<Vec<_>>()
            .join("\n---\n\n"),
    );
    s
}

fn nombre_del_hilo(q: &EntryQuery) -> String {
    let ambito = q
        .application
        .clone()
        .or_else(|| q.tag.clone())
        .unwrap_or_else(|| "diario".to_string());
    format!(
        "hilo-{ambito}-{}.md",
        chrono::Local::now().format("%Y%m%d-%H%M%S")
    )
}

/// Todo lo que cumple el filtro, entero, en una sola pagina y en orden
/// cronologico: la forma de pasarle a un agente el contexto de una aplicacion
/// sin ir entrada por entrada.
#[component]
fn HiloView(consulta: Memo<EntryQuery>, view: RwSignal<View>) -> impl IntoView {
    let entradas = RwSignal::new(Vec::<Entry>::new());
    let error = RwSignal::new(String::new());
    let cargando = RwSignal::new(true);
    let aviso = RwSignal::new(String::new());

    Effect::new(move |_| {
        let q = consulta.get();
        cargando.set(true);
        aviso.set(String::new());
        spawn_local(async move {
            match api::fetch_hilo(&q).await {
                Ok(v) => {
                    entradas.set(v);
                    error.set(String::new());
                }
                Err(e) => {
                    entradas.set(Vec::new());
                    error.set(e);
                }
            }
            cargando.set(false);
        });
    });

    // Tras inyectar el HTML de las respuestas, renderiza diagramas y resalta codigo.
    Effect::new(move |_| {
        if !entradas.get().is_empty() {
            render_diagrams();
        }
    });

    let texto = move || markdown_del_hilo(&consulta.get_untracked(), &entradas.get_untracked());

    view! {
        <div class="detail hilo">
            <div class="acciones">
                <button class="back" on:click=move |_| ir_a(view, View::List, &consulta.get_untracked())>
                    "← Volver"
                </button>
                <button
                    class="btn"
                    title="Copia el hilo entero en markdown, listo para pegarselo a un agente"
                    on:click=move |_| {
                        let md = texto();
                        if hay_portapapeles() {
                            copiar_al_portapapeles(&md, move |ok| {
                                aviso.set(
                                    if ok { "Copiado".to_string() }
                                    else { "El navegador no dejo copiar".to_string() },
                                );
                            });
                        } else {
                            // Sin contexto seguro no hay portapapeles: en vez de
                            // no hacer nada, el hilo se baja como fichero.
                            descargar_fichero(&nombre_del_hilo(&consulta.get_untracked()), &md);
                            aviso.set("Sin portapapeles: descargado como fichero".to_string());
                        }
                    }
                >
                    "Copiar markdown"
                </button>
                <button
                    class="btn"
                    title="Descarga el hilo entero como un solo .md"
                    on:click=move |_| descargar_fichero(
                        &nombre_del_hilo(&consulta.get_untracked()),
                        &texto(),
                    )
                >
                    "Descargar .md"
                </button>
                <button class="btn" title="Imprimir o guardar como PDF" on:click=move |_| imprimir()>
                    "PDF"
                </button>
                <span class="estado-export">{move || aviso.get()}</span>
            </div>
            <h1>"Hilo"</h1>
            <p class="meta-hilo">
                {move || format!(
                    "{} · {}",
                    descripcion_filtros(&consulta.get()),
                    cuantas(entradas.get().len()),
                )}
            </p>
            {move || {
                let fallo = error.get();
                if !fallo.is_empty() {
                    return view! { <div class="empty">{fallo}</div> }.into_any();
                }
                if cargando.get() {
                    return view! { <p class="loading">"Cargando…"</p> }.into_any();
                }
                let items = entradas.get();
                if items.is_empty() {
                    return view! {
                        <div class="empty">"No hay tareas registradas para este filtro."</div>
                    }
                    .into_any();
                }
                items.into_iter().map(bloque_de_hilo).collect_view().into_any()
            }}
        </div>
    }
}

/// Una entrada dentro del hilo. A diferencia del detalle, el prompt va
/// desplegado: el hilo se lee y se copia de un tiron, y un <details> cerrado
/// obliga a abrir uno por uno lo que precisamente se venia a leer junto.
fn bloque_de_hilo(e: Entry) -> impl IntoView {
    let fecha = e.created_at.format("%d/%m/%Y %H:%M").to_string();
    let modelo = e.model.clone().unwrap_or_else(|| "—".to_string());
    let meta = format!("{} · {} · {}", e.agent_name, modelo, fecha);
    let resumen = e.task_summary.clone().filter(|s| !s.is_empty());
    view! {
        <article class="hilo-entrada">
            <h2>{e.title.clone()}</h2>
            <div class="meta-row">
                <span class="badge">{e.application_name.clone()}</span>
                <span class="meta">{meta}</span>
                {e.tags.clone().into_iter().map(|t| view! { <span class="tag">{t}</span> }).collect_view()}
            </div>
            <div class="section">
                <h3>"Prompt"</h3>
                <div class="prompt-box">{e.prompt.clone()}</div>
            </div>
            {resumen.map(|s| view! { <div class="section"><h3>"Tarea"</h3><div class="summary-box">{s}</div></div> })}
            <div class="section">
                <h3>"Respuesta"</h3>
                <div class="markdown" inner_html=e.response_html.clone()></div>
            </div>
        </article>
    }
}
