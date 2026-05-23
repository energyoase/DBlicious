//! `ui`-Host-Modul (Spec §7.2) — Client-Pendant.
//!
//! Erzeugt JSON-Subtrees mit `type`-Diskriminator (identisch zum Server, weil
//! die Wire-Form die gleiche bleiben muss — der Server-SSR und das Client-
//! Eval muessen austauschbar denselben `UiNode`-Tree liefern). Whitelist-
//! Check gegen `manifest.ui_primitives`: ein Primitive-Aufruf ohne passenden
//! `UiPrimitive`-Eintrag im Manifest schlaegt mit `UiPrimitiveDenied` fehl
//! (Spec §7.2.2).
//!
//! Sandbox-Capability-Gating (`EmitUiNode`-Token) wird daruebergelegt und
//! lebt im `Sandbox::gate`-Wrapper — diese Schicht prueft nur die
//! Primitive-Whitelist.

use serde_json::{json, Value};

use shared::script::error::ScriptError;
use shared::script::manifest::{ScriptManifest, UiPrimitive};

pub struct UiHost<'m> {
    manifest: &'m ScriptManifest,
}

impl<'m> UiHost<'m> {
    pub fn new(manifest: &'m ScriptManifest) -> Self {
        Self { manifest }
    }

    fn check(&self, prim: UiPrimitive) -> Result<(), ScriptError> {
        if !self.manifest.ui_primitives.contains(&prim) {
            return Err(ScriptError::UiPrimitiveDenied {
                primitive: primitive_wire_name(prim).into(),
            });
        }
        Ok(())
    }

    pub fn text(&mut self, text: &str, props: &Value) -> Result<Value, ScriptError> {
        self.check(UiPrimitive::Text)?;
        Ok(json!({"type": "text", "text": text, "props": props}))
    }

    pub fn vstack(&mut self, children: Vec<Value>) -> Result<Value, ScriptError> {
        self.check(UiPrimitive::Vstack)?;
        Ok(json!({"type": "vstack", "children": children}))
    }

    pub fn hstack(&mut self, children: Vec<Value>) -> Result<Value, ScriptError> {
        self.check(UiPrimitive::Hstack)?;
        Ok(json!({"type": "hstack", "children": children}))
    }

    pub fn table(&mut self, props: &Value) -> Result<Value, ScriptError> {
        self.check(UiPrimitive::Table)?;
        Ok(json!({"type": "table", "props": props}))
    }

    pub fn chart(&mut self, props: &Value) -> Result<Value, ScriptError> {
        self.check(UiPrimitive::Chart)?;
        Ok(json!({"type": "chart", "props": props}))
    }
}

/// Stabile Wire-Strings — *nicht* `format!("{prim:?}").to_lowercase()`,
/// weil das `Hstack`/`ForEach` zu `hstack`/`foreach` machen wuerde, was
/// nicht dem camelCase-Vertrag aus `manifest.rs` entspricht (`forEach`).
fn primitive_wire_name(prim: UiPrimitive) -> &'static str {
    match prim {
        UiPrimitive::Vstack => "vstack",
        UiPrimitive::Hstack => "hstack",
        UiPrimitive::Text => "text",
        UiPrimitive::Table => "table",
        UiPrimitive::Chart => "chart",
        UiPrimitive::If => "if",
        UiPrimitive::ForEach => "forEach",
        UiPrimitive::Action => "action",
    }
}
