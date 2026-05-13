//! `<EntityTableShell>` — Wurzelkomponente der dekomponierten Tabelle.
//!
//! Erstellt den geteilten [`TableShellContext`] mit `TableState`,
//! `SelectionState`, der `DataResource`, der `DataSource` und allen
//! Auth-Capabilities, dann rendert die Komponente nur ihre `children`.
//!
//! Hinweis zur Implementierung: Leptos' `provide_context` verlangt
//! `Send + Sync + 'static`. Die hier genutzten `Rc<...>`-Felder
//! (`entity_type`, `columns`, `source`, `filters`, `row_actions`) sind das
//! nicht — sie werden deshalb in einer `StoredValue<_, LocalStorage>`
//! gebuendelt, die selbst `Copy + Send + Sync` ist und einen `!Send`-Wert
//! kapselt. Auf WASM laeuft die App single-threaded, der Trick ist
//! deshalb risikofrei.

use std::cell::RefCell;
use std::rc::Rc;

use leptos::prelude::*;
use shared::{ColumnMeta, EntitySettings, PermissionOp};

use super::data_source::{DataRequest, DataResponse, DataSource};
use super::filters::FilterRegistry;
use super::row_actions::RowActionsRender;
use super::selection::SelectionState;
use super::state::TableState;
use crate::auth::AuthContext;
use crate::graphql::GqlError;

pub type DataResource = LocalResource<Result<DataResponse, GqlError>>;

/// Slot fuer die `<RowActions>`-Render-Funktion. `<RowActions>` schreibt
/// hierhin synchron beim Render und bumpt anschliessend
/// [`TableShellContext::row_actions_trigger`], damit `<TableView>` reagiert.
pub type RowActionsSlot = Rc<RefCell<Option<RowActionsRender>>>;

#[derive(Clone, Copy, Debug)]
pub struct TableCaps {
    pub can_create: bool,
    pub can_update: bool,
    pub can_delete: bool,
}

impl TableCaps {
    fn from_auth(auth: &AuthContext, entity_type: &str) -> Self {
        if entity_type.is_empty() {
            return Self {
                can_create: false,
                can_update: false,
                can_delete: false,
            };
        }
        Self {
            can_create: auth.is_allowed(entity_type, PermissionOp::Create),
            can_update: auth.is_allowed(entity_type, PermissionOp::Update),
            can_delete: auth.is_allowed(entity_type, PermissionOp::Delete),
        }
    }
}

/// `!Send`-lastige Felder, die in einer `StoredValue<_, LocalStorage>`
/// gebuendelt werden.
#[derive(Clone)]
struct TableShellLocals {
    entity_type: Rc<str>,
    columns: Rc<Vec<ColumnMeta>>,
    source: Rc<dyn DataSource>,
    filters: Rc<FilterRegistry>,
    row_actions: RowActionsSlot,
}

/// Geteilter Kontext aller Tabellen-Bausteine. Bausteine konsumieren via
/// [`use_shell`].
#[derive(Clone, Copy)]
pub struct TableShellContext {
    pub state: TableState,
    pub selection: SelectionState,
    pub data: DataResource,
    pub caps: TableCaps,
    /// Wird bei jedem `<RowActions>`-Mount inkrementiert; `<TableView>`
    /// subskribiert darauf, um die Actions-Spalte erst nach dem Schreiben
    /// der Render-Funktion zu zeichnen.
    pub row_actions_trigger: RwSignal<u32>,
    locals: StoredValue<TableShellLocals, LocalStorage>,
}

impl TableShellContext {
    pub fn entity_type(&self) -> Rc<str> {
        self.locals.with_value(|l| l.entity_type.clone())
    }
    pub fn columns(&self) -> Rc<Vec<ColumnMeta>> {
        self.locals.with_value(|l| l.columns.clone())
    }
    pub fn source(&self) -> Rc<dyn DataSource> {
        self.locals.with_value(|l| l.source.clone())
    }
    pub fn filters(&self) -> Rc<FilterRegistry> {
        self.locals.with_value(|l| l.filters.clone())
    }
    pub fn with_row_actions<R>(&self, f: impl FnOnce(&RowActionsSlot) -> R) -> R {
        self.locals.with_value(|l| f(&l.row_actions))
    }
}

/// Wurzelkomponente der dekomponierten Tabelle. Erzeugt den Context und
/// rendert nur die `children`.
#[component]
pub fn EntityTableShell(
    columns: Vec<ColumnMeta>,
    source: Rc<dyn DataSource>,
    #[prop(default = String::new())] entity_type: String,
    #[prop(default = None)] settings: Option<EntitySettings>,
    /// Optional vorbefuellte Filter-Registry. Wird in 0.5.8 mit Standard-
    /// Filtern befuellt; in der Uebergangszeit reicht eine leere Registry.
    #[prop(default = Rc::new(FilterRegistry::new()))]
    filters: Rc<FilterRegistry>,
    children: Children,
) -> impl IntoView {
    let state = TableState::new();
    if let Some(s) = &settings {
        state.apply_settings(s);
    }
    let selection = SelectionState::new();

    let auth = AuthContext::use_context();
    let caps = TableCaps::from_auth(&auth, &entity_type);

    // Lade-Resource: reagiert auf Aenderungen von page/page_size/sort/filter.
    let page = state.page;
    let page_size = state.page_size;
    let sort = state.sort;
    let filter = state.filter;
    let source_for_resource = source.clone();
    let data: DataResource = LocalResource::new(move || {
        let req = DataRequest {
            page: page.get(),
            page_size: page_size.get(),
            sort: sort.get(),
            filter: filter.get(),
        };
        let src = source_for_resource.clone();
        async move { src.fetch(req).await }
    });

    let locals = TableShellLocals {
        entity_type: Rc::from(entity_type.as_str()),
        columns: Rc::new(columns),
        source,
        filters,
        row_actions: Rc::new(RefCell::new(None)),
    };

    let ctx = TableShellContext {
        state,
        selection,
        data,
        caps,
        row_actions_trigger: RwSignal::new(0),
        locals: StoredValue::new_local(locals),
    };
    provide_context(ctx);

    children()
}

/// Komfort-Helper fuer Bausteine: liefert den `TableShellContext` aus dem
/// reaktiven Kontext.
pub fn use_shell() -> TableShellContext {
    use_context::<TableShellContext>()
        .expect("kein TableShellContext im Kontext — <EntityTableShell> fehlt im Baum?")
}
