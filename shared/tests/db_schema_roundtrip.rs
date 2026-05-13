//! Roundtrip-Test fuer das erweiterte `DbSchema`.
//!
//! Stellt sicher, dass ein voll besetztes Schema (inkl. neuer Felder aus dem
//! `JezitLibrary.EntityProvider.Model`-Import) ueber `serde_json` verlustfrei
//! serialisiert und wieder deserialisiert werden kann.

use shared::{
    AuditRole, ColumnGenerated, DbColumn, DbColumnType, DbIndex, DbKey, DbRelation, DbSchema,
    DbTable, DeleteBehavior, Position, RelationColumnPair, RelationKind,
};

fn sample_schema() -> DbSchema {
    let kunde_id = DbColumn {
        id: "c-1".into(),
        name: "id".into(),
        data_type: DbColumnType::Uuid,
        nullable: false,
        primary_key: true,
        unique: true,
        generated: ColumnGenerated::OnAdd,
        concurrency_token: false,
        default_value: Some("gen_random_uuid()".into()),
        audit_role: AuditRole::None,
    };
    let kunde_version = DbColumn {
        id: "c-2".into(),
        name: "row_version".into(),
        data_type: DbColumnType::BigInt,
        nullable: false,
        primary_key: false,
        unique: false,
        generated: ColumnGenerated::OnAddOrUpdate,
        concurrency_token: true,
        default_value: None,
        audit_role: AuditRole::UpdatedAt,
    };
    let bestellung_kunde_id = DbColumn {
        id: "c-10".into(),
        name: "kunde_id".into(),
        data_type: DbColumnType::ForeignKey,
        nullable: false,
        primary_key: false,
        unique: false,
        generated: ColumnGenerated::Never,
        concurrency_token: false,
        default_value: None,
        audit_role: AuditRole::None,
    };
    let bestellung_id = DbColumn {
        id: "c-11".into(),
        name: "id".into(),
        data_type: DbColumnType::Uuid,
        nullable: false,
        primary_key: true,
        unique: true,
        generated: ColumnGenerated::OnAdd,
        concurrency_token: false,
        default_value: None,
        audit_role: AuditRole::None,
    };

    let kunde = DbTable {
        id: "t-1".into(),
        name: "Kunde".into(),
        position: Position { x: 80.0, y: 80.0 },
        columns: vec![kunde_id, kunde_version],
    };
    let bestellung = DbTable {
        id: "t-2".into(),
        name: "Bestellung".into(),
        position: Position { x: 420.0, y: 60.0 },
        columns: vec![bestellung_id, bestellung_kunde_id],
    };

    let pk_kunde = DbKey {
        id: "k-1".into(),
        name: "PK_Kunde".into(),
        table_id: "t-1".into(),
        is_primary: true,
        column_ids: vec!["c-1".into()],
    };
    let pk_bestellung = DbKey {
        id: "k-2".into(),
        name: "PK_Bestellung".into(),
        table_id: "t-2".into(),
        is_primary: true,
        column_ids: vec!["c-11".into()],
    };

    let idx_fk = DbIndex {
        id: "i-1".into(),
        name: "IX_Bestellung_kunde_id".into(),
        table_id: "t-2".into(),
        unique: false,
        column_ids: vec!["c-10".into()],
    };

    let fk = DbRelation {
        id: "r-1".into(),
        name: "FK_Bestellung_Kunde".into(),
        kind: RelationKind::ManyToOne,
        on_delete: DeleteBehavior::Restrict,
        required: true,
        source_table_id: "t-2".into(),
        target_table_id: "t-1".into(),
        column_pairs: vec![RelationColumnPair {
            source_column_id: "c-10".into(),
            target_column_id: "c-1".into(),
        }],
    };

    DbSchema {
        id: "s-1".into(),
        name: "Demo".into(),
        tables: vec![kunde, bestellung],
        relations: vec![fk],
        keys: vec![pk_kunde, pk_bestellung],
        indices: vec![idx_fk],
    }
}

#[test]
fn full_schema_roundtrips_through_json() {
    let original = sample_schema();
    let json = serde_json::to_string(&original).expect("serialize");
    let parsed: DbSchema = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(original, parsed);
}

#[test]
fn legacy_schema_without_new_fields_still_parses() {
    // Frueheres Wire-Format ohne `keys`, `indices`, `name`, `on_delete`,
    // `required`, `generated`, `concurrency_token`, `default_value`.
    // Muss dank `#[serde(default)]` weiterhin deserialisierbar bleiben.
    let legacy = r#"{
        "id": "s-old",
        "name": "Alt",
        "tables": [{
            "id": "t-1",
            "name": "T",
            "position": { "x": 0.0, "y": 0.0 },
            "columns": [{
                "id": "c-1",
                "name": "id",
                "dataType": { "kind": "uuid" },
                "nullable": false,
                "primaryKey": true,
                "unique": true
            }]
        }],
        "relations": []
    }"#;
    let parsed: DbSchema = serde_json::from_str(legacy).expect("legacy deserialize");
    assert_eq!(parsed.tables[0].columns[0].generated, ColumnGenerated::Never);
    assert!(!parsed.tables[0].columns[0].concurrency_token);
    assert!(parsed.tables[0].columns[0].default_value.is_none());
    assert!(parsed.keys.is_empty());
    assert!(parsed.indices.is_empty());
}

#[test]
fn camel_case_field_names_on_the_wire() {
    let schema = sample_schema();
    let json = serde_json::to_value(&schema).expect("to_value");
    let rel = &json["relations"][0];
    assert!(rel.get("columnPairs").is_some(), "columnPairs muss camelCase sein");
    assert!(rel.get("onDelete").is_some(), "onDelete muss camelCase sein");
    assert!(rel.get("sourceTableId").is_some());
    assert!(rel.get("targetTableId").is_some());
    let col = &json["tables"][0]["columns"][1];
    assert!(col.get("concurrencyToken").is_some());
    assert!(col.get("defaultValue").is_some());
}
