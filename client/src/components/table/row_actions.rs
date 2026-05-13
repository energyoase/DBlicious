//! Marker-Komponente: speichert die Pro-Zeile-Aktionen als Render-Funktion
//! im Shell-Context. `<TableView>` ruft die Funktion pro Zeile auf, nachdem
//! der [`super::actions::RowContext`] ins Context geschoben wurde.
//!
//! Selbst rendert die Komponente nichts.

use std::rc::Rc;

use leptos::prelude::*;

/// Eintrag im Shell-Context, der die Aktion-Children kapselt. `Rc<dyn Fn()>`
/// erlaubt mehrfaches Rendern (pro Zeile).
#[derive(Clone)]
pub struct RowActionsRender(pub Rc<dyn Fn() -> AnyView>);

#[component]
pub fn RowActions(children: ChildrenFn) -> impl IntoView {
    let children = Rc::new(children);
    let render: Rc<dyn Fn() -> AnyView> = Rc::new(move || (children)().into_any());
    let shell = super::shell::use_shell();
    shell.with_row_actions(|slot| {
        *slot.borrow_mut() = Some(RowActionsRender(render));
    });
    shell.row_actions_trigger.update(|v| *v += 1);
    view! { <></> }
}
