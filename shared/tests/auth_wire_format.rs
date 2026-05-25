//! Wire-Format-Tests fuer das Permission-Modell aus Phase 0.7.1.
//!
//! Diese Tests fixieren die exakte JSON-Form der Auth-Typen, damit der
//! Server und der Client (und kuenftige Loader-Formate `security/
//! permissions.{toml,json}`) sie verbindlich nutzen koennen.
//!
//! Wie bei `field_type_wire_format.rs` gilt:
//! - Tagged Enums (`Subject`, `Resource`) serialisieren `kind` in camelCase
//!   (`"user"`, `"entityProperty"`).
//! - Die **inneren** Felder der Struct-Varianten bleiben snake_case
//!   (`entity_type`, nicht `entityType`), weil `rename_all = "camelCase"`
//!   auf Enum-Ebene nicht in Struct-Varianten hineingreift. Diese Tests
//!   fixieren genau diesen Ist-Zustand.
//! - `Op` und `Effect` sind keine tagged Enums; sie serialisieren als
//!   camelCase-String.

use serde_json::{json, Value};
use shared::auth::{Effect, Op, Permission, Resource, Subject};

// -----------------------------------------------------------------------------
// Subject
// -----------------------------------------------------------------------------

#[test]
fn subject_user_serializes_as_kind_user() {
    let s = Subject::User { id: "u-1".into() };
    assert_eq!(
        serde_json::to_value(&s).unwrap(),
        json!({"kind": "user", "id": "u-1"})
    );
}

#[test]
fn subject_group_serializes_as_kind_group() {
    let s = Subject::Group {
        id: "g-admins".into(),
    };
    assert_eq!(
        serde_json::to_value(&s).unwrap(),
        json!({"kind": "group", "id": "g-admins"})
    );
}

#[test]
fn subject_role_serializes_as_kind_role() {
    let s = Subject::Role {
        id: "r-editor".into(),
    };
    assert_eq!(
        serde_json::to_value(&s).unwrap(),
        json!({"kind": "role", "id": "r-editor"})
    );
}

#[test]
fn subject_roundtrip_through_json() {
    let originals = vec![
        Subject::User { id: "u-1".into() },
        Subject::Group { id: "g-1".into() },
        Subject::Role { id: "r-1".into() },
    ];
    for s in originals {
        let serialized = serde_json::to_string(&s).unwrap();
        let back: Subject = serde_json::from_str(&serialized).unwrap();
        assert_eq!(s, back);
    }
}

#[test]
fn subject_unknown_kind_fails_to_deserialize() {
    let r: Result<Subject, _> = serde_json::from_value(json!({"kind": "frobnicated", "id": "x"}));
    assert!(r.is_err(), "unbekannter Subject-kind muss Fehler werfen");
}

#[test]
fn subject_id_accessor_returns_inner_id() {
    assert_eq!(Subject::User { id: "u".into() }.id(), "u");
    assert_eq!(Subject::Group { id: "g".into() }.id(), "g");
    assert_eq!(Subject::Role { id: "r".into() }.id(), "r");
}

// -----------------------------------------------------------------------------
// Resource
// -----------------------------------------------------------------------------

#[test]
fn resource_entity_type_serializes_with_name() {
    let r = Resource::EntityType {
        name: "product".into(),
    };
    assert_eq!(
        serde_json::to_value(&r).unwrap(),
        json!({"kind": "entityType", "name": "product"})
    );
}

#[test]
fn resource_entity_property_uses_snake_case_inner_fields() {
    // rename_all = "camelCase" auf der Enum-Ebene benennt die Varianten
    // (kind=entityProperty), aber die inneren Felder bleiben snake_case.
    let r = Resource::EntityProperty {
        entity_type: "product".into(),
        property: "price".into(),
    };
    assert_eq!(
        serde_json::to_value(&r).unwrap(),
        json!({
            "kind": "entityProperty",
            "entity_type": "product",
            "property": "price"
        })
    );
}

#[test]
fn resource_entity_instance_uses_snake_case_inner_fields() {
    let r = Resource::EntityInstance {
        entity_type: "product".into(),
        id: "p-42".into(),
    };
    assert_eq!(
        serde_json::to_value(&r).unwrap(),
        json!({
            "kind": "entityInstance",
            "entity_type": "product",
            "id": "p-42"
        })
    );
}

#[test]
fn resource_action_serializes_with_name() {
    let r = Resource::Action {
        name: "exportCsv".into(),
    };
    assert_eq!(
        serde_json::to_value(&r).unwrap(),
        json!({"kind": "action", "name": "exportCsv"})
    );
}

#[test]
fn resource_implementation_id_uses_registry_field() {
    // Inneres Feld heisst `registry`, nicht `kind` — das wuerde mit dem
    // serde-Variantentag `kind` der Enum-Ebene kollidieren und serde-Derive
    // bricht in dem Fall den Build.
    let r = Resource::ImplementationId {
        registry: "filter".into(),
        id: "number-range".into(),
    };
    assert_eq!(
        serde_json::to_value(&r).unwrap(),
        json!({
            "kind":     "implementationId",
            "registry": "filter",
            "id":       "number-range"
        })
    );
}

#[test]
fn resource_migration_serializes_with_id() {
    let r = Resource::Migration {
        id: "mig-2026-05-15-rename-price".into(),
    };
    assert_eq!(
        serde_json::to_value(&r).unwrap(),
        json!({"kind": "migration", "id": "mig-2026-05-15-rename-price"})
    );
}

#[test]
fn resource_roundtrip_through_json() {
    let originals = vec![
        Resource::EntityType {
            name: "product".into(),
        },
        Resource::EntityProperty {
            entity_type: "product".into(),
            property: "price".into(),
        },
        Resource::EntityInstance {
            entity_type: "product".into(),
            id: "p-42".into(),
        },
        Resource::Action {
            name: "exportCsv".into(),
        },
        Resource::Migration { id: "mig-1".into() },
    ];
    for r in originals {
        let s = serde_json::to_string(&r).unwrap();
        let back: Resource = serde_json::from_str(&s).unwrap();
        assert_eq!(r, back, "Resource-Roundtrip fehlgeschlagen: {s}");
    }
}

#[test]
fn resource_constructors_are_ergonomic() {
    assert_eq!(
        Resource::entity_type("product"),
        Resource::EntityType {
            name: "product".into()
        }
    );
    assert_eq!(
        Resource::entity_property("product", "price"),
        Resource::EntityProperty {
            entity_type: "product".into(),
            property: "price".into()
        }
    );
}

// -----------------------------------------------------------------------------
// Op + Effect
// -----------------------------------------------------------------------------

#[test]
fn op_serializes_as_camelcase_string() {
    let cases = [
        (Op::Create, "create"),
        (Op::Read, "read"),
        (Op::Update, "update"),
        (Op::Delete, "delete"),
        (Op::Execute, "execute"),
        (Op::Choose, "choose"),
        (Op::Approve, "approve"),
        (Op::Cutover, "cutover"),
        (Op::Contract, "contract"),
        (Op::Rollback, "rollback"),
    ];
    for (op, expected) in cases {
        assert_eq!(
            serde_json::to_value(op).unwrap(),
            Value::String(expected.into()),
            "Op::{:?} sollte als \"{}\" serialisieren",
            op,
            expected
        );
    }
}

#[test]
fn op_roundtrip() {
    for op in [
        Op::Create,
        Op::Read,
        Op::Update,
        Op::Delete,
        Op::Execute,
        Op::Choose,
        Op::Approve,
        Op::Cutover,
        Op::Contract,
        Op::Rollback,
    ] {
        let s = serde_json::to_string(&op).unwrap();
        let back: Op = serde_json::from_str(&s).unwrap();
        assert_eq!(op, back);
    }
}

#[test]
fn effect_serializes_as_lowercase_string() {
    assert_eq!(
        serde_json::to_value(Effect::Allow).unwrap(),
        Value::String("allow".into())
    );
    assert_eq!(
        serde_json::to_value(Effect::Deny).unwrap(),
        Value::String("deny".into())
    );
}

#[test]
fn effect_default_is_allow() {
    assert_eq!(Effect::default(), Effect::Allow);
}

// -----------------------------------------------------------------------------
// Permission (ganzes Tupel)
// -----------------------------------------------------------------------------

#[test]
fn permission_minimal_roundtrip() {
    let p = Permission {
        subject: Subject::Role {
            id: "r-editor".into(),
        },
        resource: Resource::EntityType {
            name: "product".into(),
        },
        op: Op::Update,
        effect: Effect::Allow,
        priority: 0,
        tenant_id: None,
    };
    let v = serde_json::to_value(&p).unwrap();
    assert_eq!(
        v,
        json!({
            "subject":  { "kind": "role", "id": "r-editor" },
            "resource": { "kind": "entityType", "name": "product" },
            "op":       "update",
            "effect":   "allow",
            "priority": 0
        })
    );
    // Roundtrip
    let back: Permission = serde_json::from_value(v).unwrap();
    assert_eq!(p, back);
}

#[test]
fn permission_with_property_resource_and_priority() {
    let p = Permission {
        subject: Subject::User { id: "u-7".into() },
        resource: Resource::EntityProperty {
            entity_type: "product".into(),
            property: "price".into(),
        },
        op: Op::Read,
        effect: Effect::Deny,
        priority: 100,
        tenant_id: None,
    };
    let v = serde_json::to_value(&p).unwrap();
    assert_eq!(
        v,
        json!({
            "subject":  { "kind": "user", "id": "u-7" },
            "resource": {
                "kind": "entityProperty",
                "entity_type": "product",
                "property": "price"
            },
            "op":       "read",
            "effect":   "deny",
            "priority": 100
        })
    );
}

#[test]
fn permission_with_tenant_id_serializes_as_camelcase() {
    // Permission ist eine reine Struct (kein Enum). Hier greift
    // `rename_all = "camelCase"` auf die Feldnamen: `tenant_id` -> `tenantId`.
    // Das ist konsistent zur Konvention im restlichen `shared`-Modul.
    let p = Permission {
        subject: Subject::Group { id: "g-1".into() },
        resource: Resource::EntityType {
            name: "product".into(),
        },
        op: Op::Create,
        effect: Effect::Allow,
        priority: 0,
        tenant_id: Some("acme".into()),
    };
    let v = serde_json::to_value(&p).unwrap();
    assert_eq!(v["tenantId"], json!("acme"));
    assert!(
        v.get("tenant_id").is_none(),
        "snake_case-Variante darf nicht im Wire-Format auftauchen, war: {v}"
    );
}

#[test]
fn permission_without_tenant_id_omits_the_field() {
    let p = Permission {
        subject: Subject::Group { id: "g-1".into() },
        resource: Resource::EntityType {
            name: "product".into(),
        },
        op: Op::Create,
        effect: Effect::Allow,
        priority: 0,
        tenant_id: None,
    };
    let v = serde_json::to_value(&p).unwrap();
    assert!(
        v.get("tenantId").is_none(),
        "tenantId=None soll im Wire-Format weggelassen werden, war: {v}"
    );
    assert!(v.get("tenant_id").is_none());
}

#[test]
fn permission_priority_defaults_to_zero_on_deserialize() {
    let v = json!({
        "subject":  { "kind": "role", "id": "r-x" },
        "resource": { "kind": "entityType", "name": "product" },
        "op":       "read",
        "effect":   "allow"
    });
    let p: Permission = serde_json::from_value(v).unwrap();
    assert_eq!(p.priority, 0);
    assert_eq!(p.tenant_id, None);
}

#[test]
fn permission_migration_lifecycle_ops_roundtrip() {
    // Sanity: Approve/Cutover/Contract/Rollback gegen Migration-Resource.
    for op in [Op::Approve, Op::Cutover, Op::Contract, Op::Rollback] {
        let p = Permission {
            subject: Subject::Role {
                id: "r-release-manager".into(),
            },
            resource: Resource::Migration {
                id: "mig-42".into(),
            },
            op,
            effect: Effect::Allow,
            priority: 0,
            tenant_id: None,
        };
        let s = serde_json::to_string(&p).unwrap();
        let back: Permission = serde_json::from_str(&s).unwrap();
        assert_eq!(p, back);
    }
}

#[test]
fn permission_choose_on_implementation_id_works() {
    // Phase-1.5-Vorbote: jemand darf eine konkrete Filter-Implementation waehlen.
    let p = Permission {
        subject: Subject::Group {
            id: "g-power-users".into(),
        },
        resource: Resource::ImplementationId {
            registry: "filter".into(),
            id: "number-range".into(),
        },
        op: Op::Choose,
        effect: Effect::Allow,
        priority: 0,
        tenant_id: None,
    };
    let s = serde_json::to_string(&p).unwrap();
    let back: Permission = serde_json::from_str(&s).unwrap();
    assert_eq!(p, back);
}
