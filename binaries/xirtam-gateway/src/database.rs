use std::{sync::LazyLock, time::Duration};

use sqlx::{SqlitePool, sqlite::SqliteConnectOptions};

use crate::config::CONFIG;

pub static DATABASE: LazyLock<SqlitePool> = LazyLock::new(|| {
    let options = SqliteConnectOptions::new()
        .filename(CONFIG.db_path.clone())
        .create_if_missing(true)
        .busy_timeout(Duration::from_secs(5))
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .foreign_keys(true)
        .synchronous(sqlx::sqlite::SqliteSynchronous::Normal);
    SqlitePool::connect_lazy_with(options)
});

pub async fn init_database() -> anyhow::Result<()> {
    sqlx::migrate!("./migrations").run(&*DATABASE).await?;
    Ok(())
}
