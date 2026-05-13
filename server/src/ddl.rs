//! DDL-Generator fuer das `DbSchema` aus dem Designer.
//!
//! Erzeugt SQLite-kompatible `CREATE TABLE`-Statements aus
//! [`shared::DbSchema`] und wendet sie ueber den SeaORM-Pool an.

use shared::{DbColumnType, DbSchema, DbTable};

pub fn render_create_table(table: &DbTable) -> String {
    let mut parts: Vec<String> = Vec::new();
    for col in &table.columns {
        let mut line = format!(
            "  \"{}\" {}",
            col.name,
            sqlite_type(&col.data_type)
        );
        if col.primary_key {
            line.push_str(" PRIMARY KEY");
        }
        if !col.nullable {
            line.push_str(" NOT NULL");
        }
        if col.unique && !col.primary_key {
            line.push_str(" UNIQUE");
        }
        if let Some(def) = &col.default_value {
            line.push_str(&format!(" DEFAULT {}", def));
        }
        parts.push(line);
    }
    format!(
        "CREATE TABLE IF NOT EXISTS \"{}\" (\n{}\n);",
        table.name,
        parts.join(",\n")
    )
}

fn sqlite_type(t: &DbColumnType) -> &'static str {
    match t {
        DbColumnType::Text | DbColumnType::Uuid | DbColumnType::Date | DbColumnType::DateTime => {
            "TEXT"
        }
        DbColumnType::Integer | DbColumnType::BigInt | DbColumnType::ForeignKey => "INTEGER",
        DbColumnType::Decimal { .. } => "REAL",
        DbColumnType::Boolean => "INTEGER",
        DbColumnType::Json => "TEXT",
    }
}

/// Wendet das Schema auf die DB an. Liefert die Anzahl erfolgreich
/// ausgefuehrter Statements.
///
/// Hinweis: Designer-Tabellen koennen mit Kerntabellen (`entities`,
/// `users`, …) kollidieren. Wir lassen das hier durchlaufen — bei
/// SQLite ist `CREATE TABLE IF NOT EXISTS` idempotent, und der
/// Designer-Pfad ist ein Admin-Tool.
pub async fn try_apply_schema(schema: &DbSchema) -> usize {
    let mut applied = 0usize;
    for table in &schema.tables {
        let sql = render_create_table(table);
        tracing::debug!(target: "server::ddl", "{}", sql);
        match crate::db::execute_raw(&sql).await {
            Ok(_) => applied += 1,
            Err(e) => {
                tracing::warn!(
                    target: "server::ddl",
                    "DDL-Statement fehlgeschlagen: {e}\n{sql}"
                );
            }
        }
    }
    applied
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::{AuditRole, ColumnGenerated, DbColumn, DbColumnType, DbTable, Position};

    fn col(name: &str, ty: DbColumnType, pk: bool, nullable: bool) -> DbColumn {
        DbColumn {
            id: name.into(),
            name: name.into(),
            data_type: ty,
            nullable,
            primary_key: pk,
            unique: false,
            generated: ColumnGenerated::Never,
            concurrency_token: false,
            default_value: None,
            audit_role: AuditRole::None,
        }
    }

    #[test]
    fn renders_basic_create_table() {
        let t = DbTable {
            id: "t".into(),
            name: "product".into(),
            position: Position::default(),
            columns: vec![
                col("id", DbColumnType::Text, true, false),
                col("name", DbColumnType::Text, false, false),
                col("price", DbColumnType::Decimal { precision: 10, scale: 2 }, false, true),
            ],
        };
        let sql = render_create_table(&t);
        assert!(sql.contains("CREATE TABLE IF NOT EXISTS \"product\""));
        assert!(sql.contains("\"id\" TEXT PRIMARY KEY NOT NULL"));
        assert!(sql.contains("\"price\" REAL"));
    }
}
