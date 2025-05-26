use clap::Parser;
use serde::{Deserialize, Serialize};
use sqlx::{postgres::PgPoolOptions, FromRow};

#[derive(Debug, Parser)]
struct Args {
  #[clap(long, env = "PG_DATABASE_URL")]
  pg_database_url: String,
  #[clap(long, short)]
  schema: String,
}

#[derive(Debug, Deserialize, Serialize, FromRow)]
struct Cooccurence {
  hashtag1: String,
  hashtag2: String,
  count: i64,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
  let args = Args::parse();
  let pool = PgPoolOptions::new()
    .connect(args.pg_database_url.as_ref())
    .await?;

  let topk = sqlx::query_as::<_, Cooccurence>(&format!(
    r#"
    SELECT ht1.hashtag AS hashtag1, ht2.hashtag AS hashtag2, COUNT(ht1.tweet_id) as count
    FROM {schema}.tweet_hashtag ht1
        INNER JOIN {schema}.tweet_hashtag ht2 ON ht1.tweet_id = ht2.tweet_id AND ht1.hashtag < ht2.hashtag 
    GROUP BY ht1.hashtag, ht2.hashtag 
    ORDER BY COUNT(ht1.tweet_id) DESC 
    LIMIT 10  
            "#,
    schema = args.schema
  ))
  .fetch_all(&pool).await?;

  println!("{}", serde_json::to_string_pretty(&topk)?);

  Ok(())
}
