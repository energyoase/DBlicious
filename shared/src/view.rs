//! Phase 1.9 / Q0005 — Named Views mit 3-Layer-Overlay.
//!
//! `EntityView` ist der Wire-Typ fuer eine *gespeicherte* View-Schicht
//! (Global/Group/User). Der Resolver auf Server-Seite stapelt diese
//! Schichten zu `EntitySettings`-aequivalenten Effective-Werten.

use serde::{Deserialize, Serialize};

use crate::settings::Visibility;
use crate::{FilterCriteria, Sort};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ViewLayer {
    Global,
    Group,
    User,
}

/// Sparse Property-Override fuer eine einzelne Spalte.
///
/// Merge-Semantik im Resolver: ein `Some(_)` schlaegt den darunter
/// liegenden Layer; `None` = inherit.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ViewPropertyOverride {
    pub key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visibility: Option<Visibility>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label_override_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sortable: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter_id_override: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub formatter_id_override: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EntityView {
    pub id: String,
    pub entity_type: String,
    pub view_name: String,
    pub layer: ViewLayer,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_id: Option<String>,
    pub properties: Vec<ViewPropertyOverride>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_filter: Option<FilterCriteria>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_sort: Option<Sort>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_page_size: Option<u32>,
    pub version: i32,
    pub updated_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_by: Option<String>,
}

/// Audit-Spur fuer den Resolver. Liegt in `shared`, weil sie ueber den
/// GraphQL-Response (server `ResolvedView::provenance`) mit ausgeliefert
/// werden kann — heute zwar server-intern, aber kein Wire-Bruch noetig,
/// sobald ein Client sie konsumieren will.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedLayerRef {
    pub layer: ViewLayer,
    pub view_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_id: Option<String>,
    pub version: i32,
}
