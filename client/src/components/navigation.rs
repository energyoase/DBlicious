//! Rekursive Navigationskomponente.
//!
//! Liest die Hierarchie ueber GraphQL und rendert sie unbeschraenkt tief.
//! Klickbare Knoten (mit `route`) werden als `<A>`-Links ausgegeben,
//! reine Gruppierungsknoten als optisch getrennte Ueberschriften.

use leptos::prelude::*;
use leptos_router::components::A;
use shared::{MenuAction, NavigationNode, PermissionOp, TabInfo};

use crate::auth::AuthContext;
use crate::commands::use_command_registry;
use crate::graphql::queries::fetch_navigation;
use crate::i18n::t;
use crate::styling::use_design;
use crate::tabs::use_tabs_state;

#[component]
pub fn Navigation() -> impl IntoView {
    let nav_resource = LocalResource::new(|| async { fetch_navigation().await });

    let auth = AuthContext::use_context();
    view! {
        <Suspense fallback=move || view! { <div style="padding:1rem;">{move || t("app.loading")}</div> }>
            {move || nav_resource.get().map(|res| match res.take() {
                Ok(nodes) => {
                    let filtered: Vec<_> = nodes
                        .into_iter()
                        .filter(|n| is_visible(n, &auth))
                        .collect();
                    view! { <NavList nodes=filtered depth=0 /> }.into_any()
                },
                Err(e) => {
                    let msg = e.to_string();
                    view! {
                        <div style="padding:1rem; color:#fca5a5;">
                            {move || crate::t!("app.error", "message" => msg.clone())}
                        </div>
                    }.into_any()
                }
            })}
        </Suspense>
    }
}

#[component]
fn NavList(nodes: Vec<NavigationNode>, depth: usize) -> AnyView {
    view! {
        <ul style="list-style:none; padding:0; margin:0;">
            {nodes.into_iter().map(|n| view! { <NavItem node=n depth=depth /> }).collect_view()}
        </ul>
    }.into_any()
}

/// Pruefe, ob ein Nav-Knoten unter der aktuellen Auth-Sicht sichtbar sein
/// darf. Entity-Routen (`/entities/<type>`) werden gegen `PermissionOp::Read`
/// geprueft, alles andere ist offen.
fn is_visible(node: &NavigationNode, auth: &AuthContext) -> bool {
    let action = node.resolved_action();
    if let MenuAction::Link { route } = action {
        if let Some(rest) = route.strip_prefix("/entities/") {
            // Entity-Typ extrahieren (vor `?`, `/`).
            let entity_type = rest.split(['/', '?']).next().unwrap_or(rest);
            return auth.is_allowed(entity_type, PermissionOp::Read);
        }
        if let Some(rest) = route.strip_prefix("/builder/") {
            // Designer-Sub-Link (Q0004 Option C): Update-Recht analog zur
            // Auth-Gate-Pruefung in `BuilderPage`.
            let entity_type = rest.split(['/', '?']).next().unwrap_or(rest);
            return auth.is_allowed(entity_type, PermissionOp::Update);
        }
    }
    true
}

#[component]
fn NavItem(node: NavigationNode, depth: usize) -> AnyView {
    let design = use_design();
    let auth = AuthContext::use_context();
    let label_key = node.label_key.clone();
    let label_key_view = label_key.clone();
    let has_children = !node.children.is_empty();
    let children: Vec<_> = node
        .children
        .iter()
        .filter(|c| is_visible(c, &auth))
        .cloned()
        .collect();
    let action = node.resolved_action();
    let commands = use_command_registry();

    view! {
        <li>
            {match action {
                MenuAction::Link { route } => {
                    let item_style = design.nav_item(depth, false).inline.clone();
                    view! {
                        <A href=route attr:style=item_style>
                            {move || t(&label_key_view)}
                        </A>
                    }.into_any()
                },
                MenuAction::Tab { tab_id, focus_existing } => {
                    // Klick oeffnet/aktiviert den Tab im TabsState. Wenn der
                    // Tab schon existiert, wird er fokussiert (sofern
                    // `focus_existing`).
                    let item_style = design.nav_item(depth, false).inline.clone();
                    let tabs = use_tabs_state();
                    let tab_id_for_click = tab_id.clone();
                    let label_for_tab = label_key_view.clone();
                    let on_click = move |_| {
                        let info = TabInfo {
                            id: tab_id_for_click.clone(),
                            label_key: label_for_tab.clone(),
                            route: None,
                            closable: true,
                            icon: None,
                            payload: serde_json::Value::Null,
                        };
                        tabs.open(info, focus_existing);
                    };
                    view! {
                        <span
                            style=format!("{item_style} cursor: pointer;")
                            on:click=on_click
                        >
                            {move || t(&label_key_view)}
                        </span>
                    }.into_any()
                },
                MenuAction::Code { command, args } => {
                    let item_style = design.nav_item(depth, false).inline.clone();
                    let cmd = command.clone();
                    let args = args.clone();
                    view! {
                        <span
                            style=format!("{item_style} cursor: pointer;")
                            on:click=move |_| { commands.dispatch(&cmd, &args); }
                        >
                            {move || t(&label_key_view)}
                        </span>
                    }.into_any()
                },
                MenuAction::None => {
                    let group_style = design.nav_group(depth).inline.clone();
                    view! {
                        <span style=group_style>
                            {move || t(&label_key_view)}
                        </span>
                    }.into_any()
                }
            }}
            {has_children.then(|| view! {
                <NavList nodes=children depth=depth+1 />
            })}
        </li>
    }.into_any()
}
