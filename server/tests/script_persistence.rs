//! Q0009 Phase 3.1 — SeaORM-Persistenz fuer Scripts.
//!
//! Round-trip durch die drei Tabellen `scripts`, `script_versions` und
//! `script_audit_log`. Diese Tests fassen den prozessweiten DB-Pool an und
//! brauchen daher `#[serial_test::serial]` plus `fresh_test_setup()`.

use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use serial_test::serial;

use server::entity::{script, script_audit_log, script_version};

/// Hilfsfunktion: legt einen Default-Script-Datensatz an, damit FKs aus
/// `script_versions` / `script_audit_log` aufgehen.
async fn insert_default_script(db: &DatabaseConnection, id: &str) {
    let now = chrono::Utc::now().to_rfc3339();
    let am = script::ActiveModel {
        id: Set(id.into()),
        kind: Set("provider".into()),
        manifest_json: Set("{}".into()),
        source: Set("".into()),
        version: Set(1),
        state: Set("active".into()),
        last_error: Set(None),
        created_by: Set("u-system".into()),
        created_at: Set(now.clone()),
        updated_at: Set(now),
    };
    let _ = am.insert(db).await.expect("insert default script");
}

#[tokio::test]
#[serial]
async fn script_table_round_trip_insert_fetch_mutate() {
    let _ = server::fresh_test_setup().await;
    let db = server::db::conn();

    // ---- insert ----
    let now = chrono::Utc::now().to_rfc3339();
    let am = script::ActiveModel {
        id: Set("formatter.eur".into()),
        kind: Set("provider".into()),
        manifest_json: Set(r#"{"manifestVersion":1,"tier":"reader","capabilities":[]}"#.into()),
        source: Set("fn fmt(v) { v }".into()),
        version: Set(1),
        state: Set("active".into()),
        last_error: Set(None),
        created_by: Set("u-system".into()),
        created_at: Set(now.clone()),
        updated_at: Set(now.clone()),
    };
    let _ = am.insert(&db).await.expect("insert script");

    // ---- fetch ----
    let fetched = script::Entity::find_by_id("formatter.eur".to_string())
        .one(&db)
        .await
        .expect("query")
        .expect("row exists");
    assert_eq!(fetched.kind, "provider");
    assert_eq!(fetched.version, 1);
    assert_eq!(fetched.state, "active");
    assert!(fetched.last_error.is_none());

    // ---- mutate ----
    let mut am: script::ActiveModel = fetched.into();
    am.version = Set(2);
    am.state = Set("draft".into());
    am.last_error = Set(Some(r#"{"kind":"parseFailed","line":1,"col":2,"msg":"x"}"#.into()));
    let _ = am.update(&db).await.expect("update script");

    // ---- re-fetch ----
    let fetched = script::Entity::find_by_id("formatter.eur".to_string())
        .one(&db)
        .await
        .expect("query")
        .expect("row exists");
    assert_eq!(fetched.version, 2);
    assert_eq!(fetched.state, "draft");
    assert!(fetched.last_error.is_some());
}

#[tokio::test]
#[serial]
async fn script_version_table_supports_composite_pk_history() {
    let _ = server::fresh_test_setup().await;
    let db = server::db::conn();
    insert_default_script(&db, "formatter.eur").await;

    let now = chrono::Utc::now().to_rfc3339();
    // Zwei Versionen desselben Skripts:
    for v in 1..=3 {
        let am = script_version::ActiveModel {
            script_id: Set("formatter.eur".into()),
            version: Set(v),
            manifest_json: Set("{}".into()),
            source: Set(format!("/* v{v} */").into()),
            state_at_save: Set(if v == 2 { "draft".into() } else { "active".into() }),
            last_error: Set(if v == 2 {
                Some(r#"{"kind":"manifestInvalid","reason":{"reason":"x"}}"#.into())
            } else {
                None
            }),
            saved_by: Set("u-system".into()),
            saved_at: Set(now.clone()),
        };
        let _ = am.insert(&db).await.expect("insert version");
    }

    let rows = script_version::Entity::find()
        .filter(script_version::Column::ScriptId.eq("formatter.eur"))
        .all(&db)
        .await
        .expect("query");
    assert_eq!(rows.len(), 3, "drei Versionen erwartet");
    let mut versions: Vec<i32> = rows.iter().map(|r| r.version).collect();
    versions.sort();
    assert_eq!(versions, vec![1, 2, 3]);
}

#[tokio::test]
#[serial]
async fn script_audit_log_append_only_inserts_distinct_ids() {
    let _ = server::fresh_test_setup().await;
    let db = server::db::conn();
    insert_default_script(&db, "formatter.eur").await;

    let now = chrono::Utc::now().to_rfc3339();
    for run in 0..3 {
        let am = script_audit_log::ActiveModel {
            id: sea_orm::ActiveValue::NotSet,
            script_id: Set("formatter.eur".into()),
            script_version: Set(1),
            run_id: Set(format!("run-{run}")),
            user_id: Set(Some("u-system".into())),
            started_at: Set(now.clone()),
            finished_at: Set(now.clone()),
            outcome: Set("ok".into()),
            tokens_used: Set("[]".into()),
            custom_events: Set("[]".into()),
        };
        let _ = am.insert(&db).await.expect("insert audit");
    }

    let rows = script_audit_log::Entity::find()
        .filter(script_audit_log::Column::ScriptId.eq("formatter.eur"))
        .all(&db)
        .await
        .expect("query");
    assert_eq!(rows.len(), 3);
    let ids: std::collections::BTreeSet<i64> = rows.iter().map(|r| r.id).collect();
    assert_eq!(ids.len(), 3, "PK auto_increment liefert distincte IDs");
}
