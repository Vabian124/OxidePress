// Modified by Vabian124 on 2026-11-07 to apply WordPress 7.1.4 repairs on existing databases.
// Original work: RustPress by Ryuji Yasukochi.
// Licensed under the GNU General Public License, version 2.
//! Repairs that `CREATE TABLE IF NOT EXISTS` will not apply.
//!
//! The statements are the `existing_database_repairs` rows in `gaps/schema.json`.
//! Fresh installs already match, so each repair runs only when the live table
//! is still on the older shape.

use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseConnection, DbErr, Statement};
use tracing::info;

/// WordPress 7.1.4 `db_version`. Older rows are raised to this after the repairs.
pub const TARGET_DB_VERSION: &str = "61833";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepairKind {
    AddIndex,
    ModifyColumn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SchemaRepair {
    pub id: &'static str,
    pub kind: RepairKind,
    pub table: &'static str,
    pub name: &'static str,
    pub sql: &'static str,
}

const REPAIRS: &[SchemaRepair] = &[
    SchemaRepair {
        id: "wp_posts.type_status_author",
        kind: RepairKind::AddIndex,
        table: "wp_posts",
        name: "type_status_author",
        sql: "ALTER TABLE wp_posts ADD KEY type_status_author (post_type,post_status,post_author)",
    },
    SchemaRepair {
        id: "wp_comments.comment_author",
        kind: RepairKind::ModifyColumn,
        table: "wp_comments",
        name: "comment_author",
        sql: "ALTER TABLE wp_comments MODIFY comment_author tinytext not null",
    },
];

pub fn existing_schema_repairs() -> &'static [SchemaRepair] {
    REPAIRS
}

pub fn index_is_missing(count: i64) -> bool {
    count == 0
}

pub fn desired_column_type(modify_sql: &str) -> &str {
    let Some((_, rest)) = modify_sql.split_once(" MODIFY ") else {
        return "";
    };
    rest.split_whitespace().nth(1).unwrap_or("")
}

pub fn column_type_differs(current: &str, modify_sql: &str) -> bool {
    !current.eq_ignore_ascii_case(desired_column_type(modify_sql))
}

pub fn db_version_bump_sql(version: &str) -> Result<String, DbErr> {
    if version.is_empty() || !version.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(DbErr::Custom(format!(
            "refusing non-numeric db_version: {version}"
        )));
    }
    Ok(format!(
        "UPDATE wp_options SET option_value = '{version}' WHERE option_name = 'db_version' AND CAST(option_value AS UNSIGNED) < {version}"
    ))
}

fn ident(value: &str) -> Result<&str, DbErr> {
    let ok = !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_');
    if ok {
        Ok(value)
    } else {
        Err(DbErr::Custom(format!(
            "refusing non-identifier in schema repair: {value}"
        )))
    }
}

/// Bring an existing single-site database up to the pinned schema.
///
/// Indexes that already exist are left alone. `comment_author` is modified
/// only when its type is not yet `tinytext`. Comment authors longer than
/// 255 bytes are truncated by that modification, which is the WordPress column.
pub async fn upgrade_existing_schema(db: &DatabaseConnection) -> Result<(), DbErr> {
    for repair in existing_schema_repairs() {
        let table = ident(repair.table)?;
        let name = ident(repair.name)?;
        let apply = match repair.kind {
            RepairKind::AddIndex => {
                let sql = format!(
                    "SELECT COUNT(*) AS n FROM information_schema.statistics WHERE table_schema = DATABASE() AND table_name = '{table}' AND index_name = '{name}'"
                );
                let count = scalar_i64(db, &sql, "n").await?;
                index_is_missing(count)
            }
            RepairKind::ModifyColumn => {
                let sql = format!(
                    "SELECT COLUMN_TYPE AS column_type FROM information_schema.columns WHERE table_schema = DATABASE() AND table_name = '{table}' AND column_name = '{name}'"
                );
                let current = scalar_string(db, &sql, "column_type").await?;
                column_type_differs(&current, repair.sql)
            }
        };
        if apply {
            info!(
                repair = repair.id,
                "applying existing-database schema repair"
            );
            db.execute(Statement::from_string(DatabaseBackend::MySql, repair.sql))
                .await?;
        }
    }

    let bump = db_version_bump_sql(TARGET_DB_VERSION)?;
    db.execute(Statement::from_string(DatabaseBackend::MySql, bump))
        .await?;
    Ok(())
}

async fn scalar_i64(db: &DatabaseConnection, sql: &str, column: &str) -> Result<i64, DbErr> {
    let row = db
        .query_one(Statement::from_string(DatabaseBackend::MySql, sql))
        .await?
        .ok_or_else(|| DbErr::Custom(format!("schema lookup returned no row: {column}")))?;
    row.try_get("", column)
}

async fn scalar_string(db: &DatabaseConnection, sql: &str, column: &str) -> Result<String, DbErr> {
    let row = db
        .query_one(Statement::from_string(DatabaseBackend::MySql, sql))
        .await?
        .ok_or_else(|| DbErr::Custom(format!("schema lookup returned no row: {column}")))?;
    row.try_get("", column)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    struct GapReport {
        wordpress_db_version: u64,
        existing_database_repairs: Vec<GapRepair>,
    }

    #[derive(Debug, Deserialize)]
    struct GapRepair {
        id: String,
        kind: String,
        table: String,
        name: String,
        sql: String,
    }

    fn report() -> GapReport {
        let raw = include_str!("../../../gaps/schema.json");
        serde_json::from_str(raw).expect("gaps/schema.json")
    }

    #[test]
    fn repairs_match_schema_gap_report() {
        let report = report();
        let repairs = existing_schema_repairs();
        assert_eq!(repairs.len(), report.existing_database_repairs.len());
        for (repair, row) in repairs.iter().zip(report.existing_database_repairs.iter()) {
            assert_eq!(repair.id, row.id);
            assert_eq!(repair.table, row.table);
            assert_eq!(repair.name, row.name);
            assert_eq!(repair.sql, row.sql);
            let kind = match repair.kind {
                RepairKind::AddIndex => "add_index",
                RepairKind::ModifyColumn => "modify_column",
            };
            assert_eq!(kind, row.kind);
        }
    }

    #[test]
    fn db_version_bump_matches_the_pinned_report() {
        let report = report();
        assert_eq!(TARGET_DB_VERSION, report.wordpress_db_version.to_string());
        let sql = db_version_bump_sql(TARGET_DB_VERSION).unwrap();
        assert!(sql.contains("61833"));
        assert!(sql.contains("CAST(option_value AS UNSIGNED) < 61833"));
        assert!(db_version_bump_sql("61833; DROP TABLE wp_users").is_err());
    }

    #[test]
    fn index_repair_runs_only_when_missing() {
        assert!(index_is_missing(0));
        assert!(!index_is_missing(1));
    }

    #[test]
    fn comment_author_repair_runs_only_when_the_type_differs() {
        let sql = existing_schema_repairs()
            .iter()
            .find(|repair| repair.id == "wp_comments.comment_author")
            .unwrap()
            .sql;
        assert_eq!(desired_column_type(sql), "tinytext");
        assert!(column_type_differs("text", sql));
        assert!(!column_type_differs("tinytext", sql));
        assert!(!column_type_differs("TINYTEXT", sql));
    }
}
