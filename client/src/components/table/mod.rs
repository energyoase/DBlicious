//! Generische Tabelle fuer Entitaeten — dekomponiert in Shell + Bausteine.
//!
//! Aufruf-Muster:
//! ```ignore
//! view! {
//!     <EntityTableShell entity_type=t columns=cols source=src settings=opt>
//!         <TopMenu>
//!             <GlobalFilter/>
//!             <EntityMenu> /* bulk actions */ </EntityMenu>
//!         </TopMenu>
//!         <SelectionColumn mode=SelectionMode::Multi/>
//!         <RowActions>
//!             <EditAction/>
//!             <DeleteAction/>
//!         </RowActions>
//!         <TableView/>
//!         <BottomMenu>
//!             <Pager/>
//!             <PageSize/>
//!         </BottomMenu>
//!     </EntityTableShell>
//! }
//! ```
//!
//! Module-Uebersicht:
//!   - `data_source`     : Trait-basierter Zugriff (`RemoteSource`/`LocalSource`)
//!   - `formatters`      : Zellen-Renderer (delegiert an `field::FieldRegistry`)
//!   - `state`           : `TableState` (Sort/Filter/Pagination)
//!   - `selection`       : `SelectionState` (Single-/Multi-Selektion)
//!   - `shell`           : `<EntityTableShell>` + `TableShellContext`
//!   - `top_menu`/`bottom_menu` : Container fuer obere/untere Toolbar
//!   - `entity_menu`     : Container fuer Bulk-Aktionen
//!   - `global_filter`   : Freitextsuche
//!   - `pager` / `page_size` : Pagination-Controls
//!   - `selection_column`: Marker — schaltet Selektions-Modus
//!   - `row_actions`     : Marker — registriert Per-Row-Aktionen
//!   - `actions`         : `EditAction`/`DeleteAction`/`EntityAction` + `RowContext`
//!   - `table_view`      : Die eigentliche Tabelle (Header + Body)
//!   - `filters`         : Filter-Registry (Skelett heute, 0.5.8 fuellt aus)
//!   - `view`            : `#[deprecated]` Convenience-Wrapper (Alt-API)

pub mod actions;
pub mod bottom_menu;
pub mod column_editor;
pub mod builder_preview;
pub mod data_source;
pub mod entity_menu;
pub mod filters;
pub mod formatters;
pub mod global_filter;
pub mod page_size;
pub mod pager;
pub mod row_actions;
pub mod selection;
pub mod selection_column;
pub mod shell;
pub mod state;
pub mod table_view;
pub mod top_menu;
pub mod view;

pub use actions::{DeleteAction, EditAction, EntityAction, RowContext};
pub use column_editor::apply_pending_overrides;
pub use bottom_menu::BottomMenu;
pub use builder_preview::{
    synthesize_preview_rows, BuilderPreviewSource, DEFAULT_PREVIEW_ROWS,
};
pub use data_source::{DataRequest, DataSource, LocalSource, RemoteSource};
pub use entity_menu::EntityMenu;
pub use filters::{FilterContext, FilterFactory, FilterRegistry};
pub use global_filter::GlobalFilter;
pub use page_size::PageSize;
pub use pager::Pager;
pub use row_actions::RowActions;
pub use selection::{SelectionMode, SelectionState};
pub use selection_column::SelectionColumn;
pub use shell::{EntityTableShell, TableShellContext};
pub use state::TableState;
pub use table_view::TableView;
pub use top_menu::TopMenu;
#[allow(deprecated)]
pub use view::EntityTable;
