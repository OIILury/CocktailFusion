use chrono::{Duration, NaiveDate};
use sqlx::{SqliteConnection, SqlitePool};
use tracing::instrument;
use std::fmt::Debug;
use serde::{Serialize, Deserialize};
use sqlx::types::Json;

#[derive(Debug, thiserror::Error)]
pub enum DbTwitterError {
  #[error("SQLx error: {0}")]
  Sqlx(#[from] sqlx::Error),
}

#[derive(Debug, Clone)]
pub struct TopKDatabase(SqlitePool);

impl TopKDatabase {
  pub fn new(pool: SqlitePool) -> Self {
    Self(pool)
  }
}

impl AsRef<SqlitePool> for TopKDatabase {
  fn as_ref(&self) -> &SqlitePool {
    &self.0
  }
}

#[tracing::instrument]
pub async fn monthly_hashtags<S>(
  pool: S,
  project_id: String,
  user_id: &String,
  month: i32,
  year: i32,
) -> sqlx::Result<Vec<Hashtag>>
where
  S: AsRef<SqlitePool> + Debug,
{
  // Vérifier si la table topk existe
  let table_exists: bool = sqlx::query_scalar!(
    r#"SELECT EXISTS (SELECT 1 FROM sqlite_master WHERE type='table' AND name='topk') as "exists!: bool""#
  )
  .fetch_one(pool.as_ref())
  .await?;

  if !table_exists {
    return Ok(Vec::new());
  }

  let rows = sqlx::query!(
    r#"
    SELECT hashtag as "hashtag!: String", retweet as "count!: i32"
    FROM topk
    WHERE project_id = ? AND user_id = ? AND month = ? AND year = ?
    ORDER BY count DESC
    "#,
    project_id,
    user_id,
    month,
    year
  )
  .fetch_all(pool.as_ref())
  .await?;

  Ok(rows
    .into_iter()
    .map(|row| Hashtag {
      hashtag: row.hashtag,
      count: row.count as i64,
    })
    .collect())
}

#[tracing::instrument]
pub async fn search_topk_hashtags<S>(
  pool: S,
  project_id: String,
  user_id: &String,
  query: &String,
) -> sqlx::Result<Vec<Hashtag>>
where
  S: AsRef<SqlitePool> + Debug,
{
  // Vérifier si la table hashtag existe
  let table_exists: bool = sqlx::query_scalar!(
    r#"SELECT EXISTS (SELECT 1 FROM sqlite_master WHERE type='table' AND name='hashtag') as "exists!: bool""#
  )
  .fetch_one(pool.as_ref())
  .await?;

  if !table_exists {
    return Ok(Vec::new());
  }

  let rows = sqlx::query_as!(
    _Hashtag,
    r#"
    SELECT key as "hashtag!: String", doc_count as "count!: i64"
    FROM hashtag
    WHERE project_id = ? AND user_id = ? AND key LIKE ?
    ORDER BY doc_count DESC
    "#,
    project_id,
    user_id,
    format!("%{}%", query)
  )
  .fetch_all(pool.as_ref())
  .await?;

  Ok(rows.into_iter().map(|row| row.into()).collect())
}

#[tracing::instrument]
pub async fn search_topk_hashtags_cooccurence<S>(
  pool: S,
  project_id: String,
  user_id: &String,
  query: &String,
) -> sqlx::Result<Vec<HashtagCooccurence>>
where
  S: AsRef<SqlitePool> + Debug,
{
  // Vérifier si la table hashtag_cooccurence existe
  let table_exists: bool = sqlx::query_scalar!(
    r#"SELECT EXISTS (SELECT 1 FROM sqlite_master WHERE type='table' AND name='hashtag_cooccurence') as "exists!: bool""#
  )
  .fetch_one(pool.as_ref())
  .await?;

  if !table_exists {
    return Ok(Vec::new());
  }

  let rows = sqlx::query_as!(
    HashtagCooccurence,
    r#"
    SELECT 
      key as "hashtag!: String",
      doc_count as "count!: i64",
      cooccurence as "cooccurence!: Json<Vec<String>>"
    FROM hashtag_cooccurence
    WHERE project_id = ? AND user_id = ? AND key LIKE ?
    ORDER BY doc_count DESC
    "#,
    project_id,
    user_id,
    format!("%{}%", query)
  )
  .fetch_all(pool.as_ref())
  .await?;

  Ok(rows)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Hashtag {
  pub hashtag: String,
  pub count: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HashtagCooccurence {
  pub hashtag: String,
  pub count: i64,
  pub cooccurence: Json<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct _Hashtag {
  pub hashtag: String,
  pub count: i64,
}

impl From<_Hashtag> for Hashtag {
  fn from(h: _Hashtag) -> Self {
    Self {
      hashtag: h.hashtag,
      count: h.count,
    }
  }
}

#[derive(Debug, Clone, Default)]
pub struct HashtagQuery(String);

impl From<String> for HashtagQuery {
  fn from(s: String) -> Self {
    Self(s.trim_start_matches('#').to_string())
  }
}

impl ToString for HashtagQuery {
  fn to_string(&self) -> String {
    self.0.clone()
  }
}

#[instrument]
pub async fn init_db(pool: &SqlitePool) -> sqlx::Result<()> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS topk (
            project_id TEXT,
            user_id TEXT,
            month INTEGER,
            year INTEGER,
            hashtag TEXT,
            retweet INTEGER
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS hashtag (
            project_id TEXT,
            user_id TEXT,
            key TEXT,
            doc_count INTEGER
        );
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS hashtag_cooccurence (
            project_id TEXT,
            user_id TEXT,
            key TEXT,
            doc_count INTEGER,
            cooccurence JSON
        );
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}
