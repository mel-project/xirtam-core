use std::{path::PathBuf, time::Duration};

use anyhow::Context;
use sqlx::{SqlitePool, sqlite::SqliteConnectOptions};
use xirtam_crypt::hash::Hash;
use xirtam_structs::directory::{DirectoryChunk, DirectoryHeader, DirectoryUpdate, PowSeed};

pub async fn init_sqlite(db_dir: &PathBuf) -> anyhow::Result<SqlitePool> {
    let path = db_dir.join("directory.db");
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .busy_timeout(Duration::from_secs(5));
    let pool = SqlitePool::connect_with(options).await?;
    sqlx::migrate!("./migrations").run(&pool).await?;
    Ok(pool)
}

pub async fn insert_pow_seed(
    pool: &SqlitePool,
    seed: &PowSeed,
    effort: u64,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT OR REPLACE INTO pow_seeds (seed, use_before, effort) VALUES (?, ?, ?)",
    )
    .bind(seed.seed.to_bytes().to_vec())
    .bind(seed.use_before.0 as i64)
    .bind(effort as i64)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn fetch_pow_seed(pool: &SqlitePool, seed: &Hash) -> anyhow::Result<Option<(u64, u64)>> {
    let row = sqlx::query_as::<_, (i64, i64)>(
        "SELECT use_before, effort FROM pow_seeds WHERE seed = ?",
    )
    .bind(seed.to_bytes().to_vec())
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|(use_before, effort)| (use_before as u64, effort as u64)))
}

pub async fn purge_pow_seeds(pool: &SqlitePool, now: u64) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM pow_seeds WHERE use_before < ?")
        .bind(now as i64)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn load_last_header(pool: &SqlitePool) -> anyhow::Result<(u64, Hash)> {
    let row = sqlx::query_as::<_, (i64, Vec<u8>)>(
        "SELECT height, header_hash FROM headers ORDER BY height DESC LIMIT 1",
    )
    .fetch_optional(pool)
    .await?;
    match row {
        Some((height, hash)) => {
            let bytes: [u8; 32] = hash
                .as_slice()
                .try_into()
                .context("invalid header hash bytes")?;
            Ok((height as u64, Hash::from_bytes(bytes)))
        }
        None => Ok((0, Hash::from_bytes([0u8; 32]))),
    }
}

pub async fn insert_chunk(
    pool: &SqlitePool,
    height: u64,
    header: &DirectoryHeader,
    header_hash: &Hash,
    chunk: &DirectoryChunk,
) -> anyhow::Result<()> {
    let header_bytes = bcs::to_bytes(header)?;
    let updates = &chunk.updates;
    let mut tx = pool.begin().await?;
    sqlx::query("INSERT INTO headers (height, header, header_hash) VALUES (?, ?, ?)")
        .bind(height as i64)
        .bind(header_bytes)
        .bind(header_hash.to_bytes().to_vec())
        .execute(&mut *tx)
        .await?;

    for (key, list) in updates {
        for (idx, update) in list.iter().enumerate() {
            let update_bytes = bcs::to_bytes(update)?;
            sqlx::query(
                "INSERT INTO updates (height, key_str, idx, update_blob) VALUES (?, ?, ?, ?)",
            )
            .bind(height as i64)
            .bind(key)
            .bind(idx as i64)
            .bind(update_bytes)
            .execute(&mut *tx)
            .await?;
        }
    }

    tx.commit().await?;
    Ok(())
}

pub async fn load_header(pool: &SqlitePool, height: u64) -> anyhow::Result<Option<DirectoryHeader>> {
    let row = sqlx::query_scalar::<_, Vec<u8>>("SELECT header FROM headers WHERE height = ?")
        .bind(height as i64)
        .fetch_optional(pool)
        .await?;
    match row {
        Some(data) => {
            let header = bcs::from_bytes(&data)?;
            Ok(Some(header))
        }
        None => Ok(None),
    }
}

pub async fn load_headers(
    pool: &SqlitePool,
    first: u64,
    last: u64,
) -> anyhow::Result<Vec<DirectoryHeader>> {
    let rows = sqlx::query_scalar::<_, Vec<u8>>(
        "SELECT header FROM headers WHERE height >= ? AND height <= ? ORDER BY height ASC",
    )
    .bind(first as i64)
    .bind(last as i64)
    .fetch_all(pool)
    .await?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        out.push(bcs::from_bytes(&row)?);
    }
    Ok(out)
}

pub async fn load_chunk(pool: &SqlitePool, height: u64) -> anyhow::Result<Option<DirectoryChunk>> {
    let header = match load_header(pool, height).await? {
        Some(header) => header,
        None => return Ok(None),
    };
    let rows = sqlx::query_as::<_, (String, i64, Vec<u8>)>(
        "SELECT key_str, idx, update_blob FROM updates WHERE height = ? ORDER BY key_str ASC, idx ASC",
    )
    .bind(height as i64)
    .fetch_all(pool)
    .await?;
    let mut updates: std::collections::BTreeMap<String, Vec<DirectoryUpdate>> =
        std::collections::BTreeMap::new();
    for (key, _idx, update_bytes) in rows {
        let update: DirectoryUpdate = bcs::from_bytes(&update_bytes)?;
        updates.entry(key).or_default().push(update);
    }
    Ok(Some(DirectoryChunk { header, updates }))
}

pub async fn load_updates_for_key(
    pool: &SqlitePool,
    key: &str,
    upto_height: u64,
) -> anyhow::Result<Vec<DirectoryUpdate>> {
    let rows = sqlx::query_scalar::<_, Vec<u8>>(
        "SELECT update_blob FROM updates WHERE key_str = ? AND height <= ? ORDER BY height ASC, idx ASC",
    )
    .bind(key)
    .bind(upto_height as i64)
    .fetch_all(pool)
    .await?;
    let mut updates = Vec::with_capacity(rows.len());
    for row in rows {
        updates.push(bcs::from_bytes(&row)?);
    }
    Ok(updates)
}
