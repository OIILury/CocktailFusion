use chrono::{Duration, NaiveDate};
use sqlx::{SqliteConnection, SqlitePool};

#[derive(Debug, thiserror::Error)]
pub enum DbTwitterError {
  #[error("Erreur SQL")]
  SQL { source: sqlx::Error },
}

impl From<sqlx::Error> for DbTwitterError {
  fn from(source: sqlx::Error) -> Self {
    DbTwitterError::SQL { source }
  }
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

pub async fn monthly_hashtags(
  conn: &mut SqliteConnection,
  end_date: NaiveDate,
  duration: Duration,
  count: i32,
) -> Result<Vec<(String, i32)>, DbTwitterError> {
  let start_date = end_date - duration;

  let start_date = start_date.format("%F").to_string();
  let end_date = end_date.format("%F").to_string();

  sqlx::query(
    r#"
    SELECT hashtag
    FROM topk
    WHERE  date(published_time, 'unixepoch') BETWEEN ?1 AND ?2
    ORDER BY retweet DESC
    LIMIT ?3
    "#,
  )
  .bind(start_date)
  .bind(end_date)
  .bind(count)
  .fetch_all(conn)
  .await?;

  Ok(vec![("lorem".to_string(), 42)])
}

#[derive(Debug)]
pub struct _Hashtag {
  pub hashtag: String,
  pub count: i64,
}

#[derive(Debug, Default)]
pub struct Hashtag {
  pub hashtag: String,
  pub count: i64,
  pub available: bool,
}

#[derive(Debug, Default)]
pub struct HashtagCooccurence {
  pub hashtag1: String,
  pub hashtag2: String,
  pub count: i64,
}

impl From<_Hashtag> for Hashtag {
  fn from(_Hashtag { hashtag, count }: _Hashtag) -> Self {
    Self {
      hashtag,
      count,
      ..Default::default()
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

pub async fn search_topk_hashtags<S: AsRef<SqlitePool>>(
  conn: S,
  query: HashtagQuery,
) -> Result<Vec<Hashtag>, DbTwitterError> {
  let query = format!("%{}%", query.0);
  let rows = sqlx::query_as!(
    _Hashtag,
    r#"
SELECT key as "hashtag", doc_count as "count"
FROM hashtag
WHERE key like $1
ORDER BY doc_count DESC
LIMIT 10 "#,
    query
  )
  .fetch_all(conn.as_ref())
  .await?;

  Ok(rows.into_iter().map(Into::into).collect())
}

pub async fn search_topk_hashtags_cooccurence<S: AsRef<SqlitePool>>(
  conn: S,
) -> Result<Vec<HashtagCooccurence>, DbTwitterError> {
  let rows = sqlx::query_as!(
    HashtagCooccurence,
    r#"
SELECT hashtag1, hashtag2, count
FROM hashtag_cooccurence
ORDER BY count DESC
LIMIT 10 "#
  )
  .fetch_all(conn.as_ref())
  .await?;

  Ok(rows.into_iter().map(Into::into).collect())
}
