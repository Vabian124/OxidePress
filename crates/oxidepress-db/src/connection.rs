// Modified by Vabian124 on 2026-11-07 to allow WordPress zero dates on MySQL 8.
// Original work: RustPress by Ryuji Yasukochi.
// Licensed under the GNU General Public License, version 2.
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, DbErr,
    Statement,
};
use tracing::info;

/// Connect to the WordPress database using SeaORM.
///
/// MySQL 8 rejects `0000-00-00 00:00:00`, which is the WordPress datetime
/// default. The session mode matches what WordPress sets before creating tables.
pub async fn connect(database_url: &str) -> Result<DatabaseConnection, DbErr> {
    let mut opts = ConnectOptions::new(database_url);
    opts.max_connections(10)
        .min_connections(1)
        .sqlx_logging(true);

    info!("Connecting to database...");
    let db = Database::connect(opts.clone()).await?;
    // WordPress stores unset datetimes as 0000-00-00. MySQL 8's default mode
    // rejects that. The global change applies only to connections opened after
    // it, so the pool used by the server is opened second.
    let global = "SET GLOBAL sql_mode = 'NO_ENGINE_SUBSTITUTION'";
    if let Err(error) = db
        .execute(Statement::from_string(DatabaseBackend::MySql, global))
        .await
    {
        tracing::warn!(%error, "could not set the global sql_mode");
    }
    db.close().await?;

    let db = Database::connect(opts).await?;
    db.execute(Statement::from_string(
        DatabaseBackend::MySql,
        "SET SESSION sql_mode = 'NO_ENGINE_SUBSTITUTION'",
    ))
    .await?;
    info!("Database connection established");

    Ok(db)
}
