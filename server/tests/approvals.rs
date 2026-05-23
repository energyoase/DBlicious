//! Phase 1.7.6: Approval-Workflow (Single-Stage MVP).
//!
//! Akzeptanz: Bestellung > X € braucht Vorgesetzten-Bestaetigung;
//! Delegations-Pfad transparent im Audit. Hier MVP-Variante ohne
//! Delegation/Multi-Stage — Audit-Eintrag bei jeder Entscheidung.

use sea_orm::{ActiveValue, ColumnTrait, EntityTrait, QueryFilter};
use serial_test::serial;
use shared::builder::guard::GuardExpr;
use shared::state_machine::{StateMachine, Transition};

use server::approvals::{self, ApprovalError, Decision};

async fn install_invoice_settings_with_sm() {
    use server::entity::metadata_settings;
    let sm = StateMachine {
        states: vec!["draft".into(), "posted".into()],
        initial: Some("draft".into()),
        state_field: "state".into(),
        transitions: vec![Transition {
            from: "draft".into(),
            to: "posted".into(),
            event: "post".into(),
            guard: Some(GuardExpr::new("fields.amount > 0")),
            permission: None, // im Test ohne Permission-Pruefung
        }],
    };
    let settings = shared::EntitySettings {
        entity_type: "invoice".into(),
        state_machine: Some(sm),
        ..Default::default()
    };
    let json = serde_json::to_string(&settings).unwrap();
    let conn = server::db::conn();
    let _ = metadata_settings::Entity::insert(metadata_settings::ActiveModel {
        entity_type: ActiveValue::Set("invoice".into()),
        settings_json: ActiveValue::Set(json),
    })
    .exec(&conn)
    .await;
}

async fn insert_invoice(id: &str, state: &str, amount: i64) {
    use server::entity::entities;
    let conn = server::db::conn();
    let fields = serde_json::json!({"state": state, "amount": amount});
    let _ = entities::Entity::insert(entities::ActiveModel {
        entity_type: ActiveValue::Set("invoice".into()),
        id: ActiveValue::Set(id.into()),
        fields_json: ActiveValue::Set(fields.to_string()),
        hash: ActiveValue::Set("0".into()),
    })
    .exec(&conn)
    .await;
}

#[tokio::test]
#[serial]
async fn request_creates_pending_approval() {
    server::fresh_test_setup().await;
    install_invoice_settings_with_sm().await;
    insert_invoice("inv-1", "draft", 100).await;

    let conn = server::db::conn();
    let id = approvals::request(&conn, "invoice", "inv-1", "post", Some("alice"))
        .await
        .unwrap();
    let m = approvals::get(&conn, &id).await.unwrap().unwrap();
    assert_eq!(m.status, "pending");
    assert_eq!(m.requested_by.as_deref(), Some("alice"));
    assert_eq!(m.entity_type, "invoice");
    assert_eq!(m.target_event, "post");
}

#[tokio::test]
#[serial]
async fn approve_triggers_state_transition_and_writes_audit() {
    server::fresh_test_setup().await;
    install_invoice_settings_with_sm().await;
    insert_invoice("inv-ok", "draft", 500).await;

    let conn = server::db::conn();
    let id = approvals::request(&conn, "invoice", "inv-ok", "post", Some("alice"))
        .await
        .unwrap();
    let updated = approvals::decide(&conn, &id, Decision::Approve, "bob", Some("looks good"))
        .await
        .unwrap();
    assert_eq!(updated.status, "approved");
    assert_eq!(updated.last_decided_by.as_deref(), Some("bob"));
    assert_eq!(updated.comment.as_deref(), Some("looks good"));

    // Audit: state_transition + approval_decision beide vorhanden.
    use server::entity::audit_log;
    let rows = audit_log::Entity::find().all(&conn).await.unwrap();
    let kinds: std::collections::HashSet<&str> = rows.iter().map(|r| r.kind.as_str()).collect();
    assert!(kinds.contains("state_transition"));
    assert!(kinds.contains("approval_decision"));

    // Approval-Decision payload pruefen
    let dec = audit_log::Entity::find()
        .filter(audit_log::Column::Kind.eq("approval_decision"))
        .one(&conn)
        .await
        .unwrap()
        .unwrap();
    let payload: serde_json::Value =
        serde_json::from_str(dec.payload_json.as_deref().unwrap_or("null")).unwrap();
    assert_eq!(payload["decision"], serde_json::json!("approved"));
    assert_eq!(payload["comment"], serde_json::json!("looks good"));
}

#[tokio::test]
#[serial]
async fn reject_does_not_trigger_transition() {
    server::fresh_test_setup().await;
    install_invoice_settings_with_sm().await;
    insert_invoice("inv-no", "draft", 500).await;

    let conn = server::db::conn();
    let id = approvals::request(&conn, "invoice", "inv-no", "post", None)
        .await
        .unwrap();
    let _ = approvals::decide(&conn, &id, Decision::Reject, "bob", Some("nope"))
        .await
        .unwrap();

    // Entity-State darf NICHT auf "posted" gewechselt sein.
    use sea_orm::ActiveValue;
    use server::entity::entities;
    let row = entities::Entity::find_by_id(("invoice".to_string(), "inv-no".to_string()))
        .one(&conn)
        .await
        .unwrap()
        .unwrap();
    let _ = ActiveValue::<String>::Set(String::new()); // keep import
    let fields: serde_json::Value = serde_json::from_str(&row.fields_json).unwrap();
    assert_eq!(
        fields["state"],
        serde_json::json!("draft"),
        "state darf nicht transitionieren"
    );

    // Audit: nur approval_decision, KEIN state_transition.
    use server::entity::audit_log;
    let kinds: std::collections::HashSet<String> = audit_log::Entity::find()
        .all(&conn)
        .await
        .unwrap()
        .into_iter()
        .map(|r| r.kind)
        .collect();
    assert!(kinds.contains("approval_decision"));
    assert!(!kinds.contains("state_transition"));
}

#[tokio::test]
#[serial]
async fn cannot_decide_twice() {
    server::fresh_test_setup().await;
    install_invoice_settings_with_sm().await;
    insert_invoice("inv-d", "draft", 100).await;
    let conn = server::db::conn();
    let id = approvals::request(&conn, "invoice", "inv-d", "post", None)
        .await
        .unwrap();
    approvals::decide(&conn, &id, Decision::Approve, "bob", None)
        .await
        .unwrap();
    let err = approvals::decide(&conn, &id, Decision::Approve, "bob", None)
        .await
        .unwrap_err();
    assert!(matches!(err, ApprovalError::AlreadyDecided(_)));
}

#[tokio::test]
#[serial]
async fn approve_failure_leaves_approval_pending() {
    // amount=0 ⇒ Guard fails ⇒ Transition-Fehler ⇒ approval bleibt pending.
    server::fresh_test_setup().await;
    install_invoice_settings_with_sm().await;
    insert_invoice("inv-zero", "draft", 0).await;
    let conn = server::db::conn();
    let id = approvals::request(&conn, "invoice", "inv-zero", "post", None)
        .await
        .unwrap();
    let err = approvals::decide(&conn, &id, Decision::Approve, "bob", None)
        .await
        .unwrap_err();
    assert!(matches!(err, ApprovalError::TransitionFailed(_)));
    let m = approvals::get(&conn, &id).await.unwrap().unwrap();
    assert_eq!(
        m.status, "pending",
        "Approval bleibt pending wenn Transition fehlschlaegt"
    );
}

#[tokio::test]
#[serial]
async fn list_pending_filters_correctly() {
    server::fresh_test_setup().await;
    install_invoice_settings_with_sm().await;
    insert_invoice("inv-a", "draft", 100).await;
    insert_invoice("inv-b", "draft", 200).await;
    let conn = server::db::conn();
    let _ = approvals::request(&conn, "invoice", "inv-a", "post", None)
        .await
        .unwrap();
    let _ = approvals::request(&conn, "invoice", "inv-b", "post", None)
        .await
        .unwrap();
    let pa = approvals::list_pending_for_entity(&conn, "invoice", "inv-a")
        .await
        .unwrap();
    let pb = approvals::list_pending_for_entity(&conn, "invoice", "inv-b")
        .await
        .unwrap();
    assert_eq!(pa.len(), 1);
    assert_eq!(pb.len(), 1);
    assert_ne!(pa[0].id, pb[0].id);
}
