use std::{
  collections::HashMap,
  io::{stdin, BufRead, BufReader},
  ops::Sub,
  str::FromStr,
};

use chrono::{Datelike, Duration, Month, NaiveDate, NaiveDateTime, Timelike};
use glob::glob;
use itertools::Itertools;
use serde::{Deserialize, Deserializer};
use sqlx::{
  migrate::MigrateDatabase,
  sqlite::{
    SqliteConnectOptions, SqliteJournalMode, SqliteLockingMode, SqlitePoolOptions,
    SqliteSynchronous,
  },
  ConnectOptions, QueryBuilder, Sqlite, SqlitePool,
};

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct Tweet {
  pub id: String,
  created_at: String,
  // #[serde(with = "ts_milliseconds")]
  // pub published_time: DateTime<Utc>,
  pub published_time: i64,
  #[serde(deserialize_with = "deserialize_null_default")]
  pub hashtags: Vec<String>,
}

pub async fn drop_databases() {
  let d = NaiveDate::from_ymd(2022, 2, 23);
  println!("{}", end_of_month(d));

  // ouais mon pattern est bon maggle
  for entry in glob("*.db").unwrap() {
    match entry {
      Ok(path) => Sqlite::drop_database(path.to_str().unwrap_or_default())
        .await
        .expect("impossible de supprimer la base de données"),
      Err(e) => println!("{:?}", e),
    }
  }
}

pub async fn create_databases(
  start: NaiveDate,
  duration: usize,
) -> anyhow::Result<HashMap<String, SqlitePool>> {
  let mut s = start;
  let mut connections = HashMap::new();
  for _ in 0..duration {
    let e = end_of_month(s);
    let uri = format!(
      "sqlite://./topk_{}_{}.db",
      s.format("%Y-%m-%d"),
      e.format("%Y-%m-%d")
    );
    let options = SqliteConnectOptions::new()
      .journal_mode(SqliteJournalMode::Off)
      .synchronous(SqliteSynchronous::Off)
      .locking_mode(SqliteLockingMode::Exclusive)
      .pragma("cache_size", (-1024 * 1024).to_string()); // on alloue 1GB de mémoire max
    let pool = SqlitePoolOptions::new().connect_with(options).await?;
    connections.insert(uri, pool);

    s = end_of_month(s) + Duration::days(1);
  }

  // let mut c = connections
  //     .get("sqlite://./topk_2021-01-01_2021-01-31.db")
  //     .unwrap()
  //     .acquire()
  //     .await?;
  //
  // let _ = sqlx::query("SELECT 1").execute(&mut c).await;

  Ok(connections)
}

pub async fn ingest_sqlite() -> anyhow::Result<()> {
  let uri = format!("sqlite://./topk.db",);
  let _ = Sqlite::drop_database(&uri).await;
  Sqlite::create_database(&uri).await?;

  let mut conn = SqliteConnectOptions::from_str(&uri)?
    .journal_mode(SqliteJournalMode::Off)
    .synchronous(SqliteSynchronous::Off)
    .locking_mode(SqliteLockingMode::Exclusive)
    .pragma("cache_size", (-1024 * 1024).to_string()) // on alloue 1GB de mémoire max
    .connect()
    .await?;

  sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS topk(hashtag TEXT NOT NULL DEFAULT "" UNIQUE, retweet INTEGER NOT NULL DEFAULT 1, published_time INTEGER NOT NULL);
        CREATE TABLE IF NOT EXISTS topk_coocurrence(hashtag_one TEXT NOT NULL DEFAULT "", hashtag_two TEXT NOT NULL DEFAULT "" , retweet INTEGER NOT NULL DEFAULT 1, published_time INTEGER NOT NULL);
        CREATE UNIQUE INDEX IF NOT EXISTS coocurrences_idx ON topk_coocurrence(hashtag_one, hashtag_two);
        "#
    ).execute(&mut conn).await?;

  let rdr = BufReader::new(stdin());

  // sqlx::query("BEGIN TRANSACTION").execute(&mut conn).await?;
  let mut query_builder_topk: QueryBuilder<Sqlite> =
    QueryBuilder::new("REPLACE INTO topk (hashtag, published_time) ");
  let mut query_builder_coocurrence: QueryBuilder<Sqlite> =
    QueryBuilder::new("REPLACE INTO topk_coocurrence (hashtag_one, hashtag_two, published_time) ");

  for line in rdr.lines() {
    let t: Tweet = serde_json::from_str(&line?)?;

    if t.hashtags.is_empty() {
      continue;
    }
    println!(
      "{}:{} ",
      NaiveDateTime::from_timestamp(t.published_time / 1000, 0).format("%Y-%m-%d"),
      t.created_at
    );
    let d = NaiveDateTime::from_timestamp(t.published_time, 0);
    let d = d
      .with_hour(0)
      .unwrap()
      .with_minute(0)
      .unwrap()
      .with_second(0)
      .unwrap()
      .timestamp();
    let hashtags = t.hashtags;
    // hashtags.sort();

    query_builder_topk.reset();
    query_builder_topk.push_values(hashtags.clone(), |mut b, h| {
      b.push_bind(h).push_bind(d);
    });
    query_builder_topk.push(" ON CONFLICT (hashtag) DO UPDATE SET retweet = retweet + 1");
    let query = query_builder_topk.build();
    query.execute(&mut conn).await?;

    let coocurrences: Vec<_> = hashtags
      .iter()
      .combinations_with_replacement(2)
      .flat_map(|r| {
        let (a, b) = (r[0], r[1]);
        if a == b {
          None
        } else {
          Some((a.clone(), b.clone()))
        }
      })
      .collect();
    if coocurrences.is_empty() {
      continue;
    }

    query_builder_coocurrence.reset();
    query_builder_coocurrence.push_values(coocurrences, |mut b, (hashtag_one, hashtag_two)| {
      b.push_bind(hashtag_one).push_bind(hashtag_two).push_bind(d);
    });
    query_builder_coocurrence.push(" ON CONFLICT DO UPDATE SET retweet = retweet + 1");
    let query = query_builder_coocurrence.build();
    query.execute(&mut conn).await?;
  }
  // sqlx::query("END TRANSACTION").execute(&mut conn).await?;

  println!("create index");
  sqlx::query(
    r#"
    CREATE INDEX IF NOT EXISTS retweet_topk ON topk (retweet DESC);
    CREATE INDEX IF NOT EXISTS retweet_topk_coocurrence ON topk_coocurrence (retweet DESC);
    "#,
  )
  .execute(&mut conn)
  .await?;

  Ok(())
}

fn deserialize_null_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
  T: Default + Deserialize<'de>,
  D: Deserializer<'de>,
{
  let opt = Option::deserialize(deserializer)?;
  Ok(opt.unwrap_or_default())
}

fn end_of_month(d: NaiveDate) -> NaiveDate {
  let (year, month) = if d.month() == Month::December.number_from_month() {
    (d.year() + 1, Month::January.number_from_month())
  } else {
    (d.year(), d.month() + 1)
  };

  let time5 = NaiveDate::from_ymd(year, month, 1).and_hms(0, 0, 0);

  let t = time5.sub(chrono::Duration::nanoseconds(1));
  NaiveDate::from_ymd(t.year(), t.month(), t.day())
}

pub fn beginning_of_month(d: NaiveDate) -> NaiveDate {
  NaiveDate::from_ymd(d.year(), d.month(), 1)
}
