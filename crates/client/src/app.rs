//! Componentes de la SPA: cabecera, sidebar, timeline y detalle de entrada.

use leptos::prelude::*;
use wasm_bindgen_futures::spawn_local;

use diario_shared::{Application, Entry, EntrySummary};

use crate::api::{self, EntryFilters};
use crate::ffi::{current_path, on_popstate, push_path, render_diagrams};

#[derive(Clone, Copy, PartialEq)]
pub enum View {
    List,
    Detail(i64),
}

fn parse_route(path: &str) -> View {
    if let Some(rest) = path.strip_prefix("/entry/") {
        if let Ok(id) = rest.trim_end_matches('/').parse::<i64>() {
            return View::Detail(id);
        }
    }
    View::List
}

#[component]
pub fn App() -> impl IntoView {
    let apps = RwSignal::new(Vec::<Application>::new());
    let selected_app = RwSignal::new(Option::<String>::None);
    let from = RwSignal::new(String::new());
    let to = RwSignal::new(String::new());
    let search = RwSignal::new(String::new());
    let entries = RwSignal::new(Vec::<EntrySummary>::new());
    let loading = RwSignal::new(false);
    let view = RwSignal::new(parse_route(&current_path()));

    on_popstate(move || view.set(parse_route(&current_path())));

    spawn_local(async move {
        if let Ok(a) = api::fetch_applications().await {
            apps.set(a);
        }
    });

    // Recarga las entradas cuando cambia cualquier filtro.
    Effect::new(move |_| {
        let f = EntryFilters {
            application: selected_app.get(),
            from: from.get(),
            to: to.get(),
            search: search.get(),
        };
        loading.set(true);
        spawn_local(async move {
            match api::fetch_entries(f).await {
                Ok(page) => entries.set(page.entries),
                Err(_) => entries.set(Vec::new()),
            }
            loading.set(false);
        });
    });

    view! {
        <div class="app">
            <Header search=search />
            <Sidebar apps=apps selected_app=selected_app from=from to=to />
            <main class="main">
                {move || match view.get() {
                    View::List => view! { <Timeline entries=entries loading=loading view=view /> }.into_any(),
                    View::Detail(id) => view! { <EntryDetail id=id view=view /> }.into_any(),
                }}
            </main>
        </div>
    }
}

#[component]
fn Header(search: RwSignal<String>) -> impl IntoView {
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
        </header>
    }
}

#[component]
fn Sidebar(
    apps: RwSignal<Vec<Application>>,
    selected_app: RwSignal<Option<String>>,
    from: RwSignal<String>,
    to: RwSignal<String>,
) -> impl IntoView {
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
                <button
                    class="btn-clear"
                    on:click=move |_| {
                        selected_app.set(None);
                        from.set(String::new());
                        to.set(String::new());
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
                        {list.into_iter().map(|e| entry_card(e, view)).collect_view()}
                    </div>
                }
            })
            .collect_view()
            .into_any()
    }
}

fn entry_card(e: EntrySummary, view: RwSignal<View>) -> impl IntoView {
    let id = e.id;
    let time = e.created_at.format("%H:%M").to_string();
    let model_suffix = e.model.clone().map(|m| format!(" · {m}")).unwrap_or_default();
    let meta = format!("{}{} · {}", e.agent_name, model_suffix, time);
    let tags = e.tags.clone();
    view! {
        <div
            class="card"
            on:click=move |_| {
                push_path(&format!("/entry/{id}"));
                view.set(View::Detail(id));
            }
        >
            <div class="row">
                <span class="badge">{e.application_name.clone()}</span>
                <span class="title">{e.title.clone()}</span>
            </div>
            <div class="row" style="margin-top:6px">
                <span class="meta">{meta}</span>
                {tags.into_iter().map(|t| view! { <span class="tag">{t}</span> }).collect_view()}
            </div>
            <p class="snippet">{e.snippet.clone()}</p>
        </div>
    }
}

#[component]
fn EntryDetail(id: i64, view: RwSignal<View>) -> impl IntoView {
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
                on:click=move |_| {
                    push_path("/");
                    view.set(View::List);
                }
            >
                "← Volver"
            </button>
            {move || match entry.get() {
                None => view! { <p class="loading">"Cargando…"</p> }.into_any(),
                Some(e) => detail_body(e).into_any(),
            }}
        </div>
    }
}

fn detail_body(e: Entry) -> impl IntoView {
    let date = e.created_at.format("%d/%m/%Y %H:%M").to_string();
    let model = e.model.clone().unwrap_or_else(|| "—".to_string());
    let meta = format!("{} · {}", e.agent_name, model);
    let tags = e.tags.clone();
    let summary = e.task_summary.clone().filter(|s| !s.is_empty());
    let attachments = e.attachments.clone();

    view! {
        <h1>{e.title.clone()}</h1>
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
