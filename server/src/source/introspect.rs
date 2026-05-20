//! SQLite-Schema-Introspektion via sqlite_master + PRAGMA.

use std::collections::BTreeMap;

use sea_orm::{DatabaseConnection, DbBackend, FromQueryResult, Statement};

use super::SourceError;

#[derive(Debug, Default, Clone)]
pub struct Schema {
    pub tables: BTreeMap<String, TableMeta>,
}

#[derive(Debug, Clone)]
pub struct TableMeta {
    pub name: String,
    pub columns: Vec<ColumnMeta>,
    pub primary_key: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ColumnMeta {
    pub name: String,
    pub sql_type: String,
    pub not_null: bool,
}

#[derive(FromQueryResult, Debug)]
struct TableRow {
    name: String,
}

#[derive(FromQueryResult, Debug)]
struct ColumnRow {
    name: String,
    #[sea_orm(from_alias = "type")]
    type_: String,
    notnull: i32,
    pk: i32,
}

pub async fn read_schema(db: &DatabaseConnection) -> Result<Schema, SourceError> {
    let table_rows: Vec<TableRow> = TableRow::find_by_statement(Statement::from_string(
        DbBackend::Sqlite,
        "SELECT name FROM sqlite_master \
         WHERE type='table' AND name NOT LIKE 'sqlite_%' \
         ORDER BY name",
    ))
    .all(db)
    .await?;

    let mut tables = BTreeMap::new();
    for t in table_rows {
        let col_rows: Vec<ColumnRow> = ColumnRow::find_by_statement(Statement::from_string(
            DbBackend::Sqlite,
            format!("PRAGMA table_info(\"{}\")", t.name.replace('"', "\"\"")),
        ))
        .all(db)
        .await?;

        let mut columns = Vec::new();
        let mut pk_pairs: Vec<(i32, String)> = Vec::new();
        for c in col_rows {
            if c.pk > 0 {
                pk_pairs.push((c.pk, c.name.clone()));
            }
            columns.push(ColumnMeta {
                name: c.name,
                sql_type: c.type_,
                not_null: c.notnull != 0,
            });
        }
        pk_pairs.sort_by_key(|(idx, _)| *idx);
        let primary_key: Vec<String> = pk_pairs.into_iter().map(|(_, n)| n).collect();

        tables.insert(
            t.name.clone(),
            TableMeta { name: t.name, columns, primary_key },
        );
    }
    Ok(Schema { tables })
}
