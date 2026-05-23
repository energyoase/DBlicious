//! Phase 1.7.5: State-Machine-Engine.
//!
//! Akzeptanz aus Roadmap: Invoice geht nur per `post`-Transition von
//! `draft` → `posted`; Permission `Invoice.post` erforderlich; Audit-
//! Eintrag pro Transition.

use sea_orm::{ActiveValue, EntityTrait};
use serial_test::serial;
use shared::builder::guard::GuardExpr;
use shared::state_machine::{StateMachine, Transition};

async fn install_invoice_settings_with_sm() {
    use server::entity::metadata_settings;
    let sm = StateMachine {
        states:      vec!["draft".into(), "posted".into(), "cancelled".into()],
        initial:     Some("draft".into()),
        state_field: "state".into(),
        transitions: vec![
            Transition {
                from:       "draft".into(),
                to:         "posted".into(),
                event:      "post".into(),
                guard:      Some(GuardExpr::new("fields.amount > 0")),
                permission: Some("invoice.post".into()),
            },
            Transition {
                from:       "posted".into(),
                to:         "cancelled".into(),
                event:      "cancel".into(),
                guard:      None,
                permission: None,
            },
        ],
    };
    let settings = shared::EntitySettings {
        entity_type:   "invoice".into(),
        state_machine: Some(sm),
        ..Default::default()
    };
    let json = serde_json::to_string(&settings).unwrap();
    let conn = server::db::conn();
    let _ = metadata_settings::Entity::insert(metadata_settings::ActiveModel {
        entity_type:   ActiveValue::Set("invoice".into()),
        settings_json: ActiveValue::Set(json),
    })
    .exec(&conn)
    .await;
}

async fn insert_invoice(id: &str, state: &str, amount: i64) {
    use server::entity::entities;
    let conn = server::db::conn();
    let fields = serde_json::json!({
        "state":  state,
        "amount": amount,
    });
    let _ = entities::Entity::insert(entities::ActiveModel {
        entity_type: ActiveValue::Set("invoice".into()),
        id:          ActiveValue::Set(id.into()),
        fields_json: ActiveValue::Set(fields.to_string()),
        hash:        ActiveValue::Set("0".into()),
    })
    .exec(&conn)
    .await;
}

#[tokio::test]
#[serial]
async fn no_state_machine_returns_specific_error() {
    server::fresh_test_setup().await;
    let err = server::state_machine::apply_transition("nope", "x", "post", None)
        .await
        .unwrap_err();
    assert!(matches!(err, server::state_machine::TransitionError::NoStateMachine(_)));
}

#[tokio::test]
#[serial]
async fn missing_entity_returns_not_found() {
    server::fresh_test_setup().await;
    install_invoice_settings_with_sm().await;
    let err = server::state_machine::apply_transition("invoice", "nope-id", "post", None)
        .await
        .unwrap_err();
    assert!(matches!(err, server::state_machine::TransitionError::NotFound { .. }));
}

#[tokio::test]
#[serial]
async fn unmatching_event_returns_no_transition() {
    server::fresh_test_setup().await;
    install_invoice_settings_with_sm().await;
    insert_invoice("inv-1", "draft", 100).await;
    // "cancel" ist nur aus "posted" definiert.
    let err = server::state_machine::apply_transition("invoice", "inv-1", "cancel", None)
        .await
        .unwrap_err();
    assert!(
        matches!(err, server::state_machine::TransitionError::NoMatchingTransition { ref from, .. } if from == "draft")
    );
}

#[tokio::test]
#[serial]
async fn guard_failure_blocks_transition() {
    server::fresh_test_setup().await;
    install_invoice_settings_with_sm().await;
    insert_invoice("inv-zero", "draft", 0).await; // amount = 0 → guard "amount > 0" failed
    let err = server::state_machine::apply_transition("invoice", "inv-zero", "post", None)
        .await
        .unwrap_err();
    assert!(matches!(err, server::state_machine::TransitionError::GuardFailed { .. }));
}

#[tokio::test]
#[serial]
async fn happy_path_transitions_and_writes_audit() {
    use sea_orm::QueryFilter;
    server::fresh_test_setup().await;
    install_invoice_settings_with_sm().await;
    insert_invoice("inv-2", "draft", 100).await;
    // Ohne actor_user_id ⇒ keine Permission-Pruefung (System-Pfad).
    let outcome = server::state_machine::apply_transition("invoice", "inv-2", "post", None)
        .await
        .unwrap();
    assert_eq!(outcome.from,  "draft");
    assert_eq!(outcome.to,    "posted");
    assert_eq!(outcome.event, "post");

    // Audit-Eintrag muss existieren.
    use sea_orm::ColumnTrait;
    use server::entity::audit_log;
    let conn = server::db::conn();
    let audit_rows = audit_log::Entity::find()
        .filter(audit_log::Column::Kind.eq("state_transition"))
        .all(&conn)
        .await
        .unwrap();
    assert_eq!(audit_rows.len(), 1);
    let row = &audit_rows[0];
    assert_eq!(row.op.as_deref(), Some("post"));
    assert!(row.resource_id.as_deref().unwrap_or("").contains("inv-2"));
    let payload: serde_json::Value = serde_json::from_str(
        row.payload_json.as_deref().unwrap_or("null"),
    )
    .unwrap();
    assert_eq!(payload["from"], serde_json::json!("draft"));
    assert_eq!(payload["to"],   serde_json::json!("posted"));
}
