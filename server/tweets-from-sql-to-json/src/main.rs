use std::io::{stdin, stdout, BufRead, BufReader, BufWriter, Write};

use chrono::NaiveDate;
use futures::TryStreamExt;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};

#[derive(Debug, Deserialize, Serialize, FromRow)]
struct Tweet {
  id: String,
  created_at: String,
  published_time: i64,
  user_id: String,
  user_name: String,
  user_screen_name: String,
  text: String,
  source: Option<String>,
  language: String,
  coordinates_longitude: Option<String>,
  coordinates_latitude: Option<String>,
  possibly_sensitive: Option<bool>,
  hashtags: Option<Vec<String>>,
  urls: Option<Vec<String>>,
  retweet_count: i64,
  reply_count: i64,
  quote_count: i64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  let pg_url = std::env::var("PG_DATABASE_URL").unwrap_or_else(|_| {
    "postgres://cocktailuser:cocktailuser@localhost:5432/cocktail_pg".to_string()
  });

  let pool = PgPool::connect(&pg_url).await?;

  let rdr = BufReader::new(stdin());
  let mut date: Option<NaiveDate> = None;

  for input in rdr.lines() {
    let input_date = NaiveDate::parse_from_str(input?.as_str(), "%Y-%m-%d");

    if input_date.is_ok() {
      date = Some(input_date?);
    }

    break;
  }

  if date.is_none() {
    println!("Paramètre date invalide: format attendu YYYY-MM-DD");

    return Ok(());
  }
  let query = format!(
    r#"
        WITH rt as (
            SELECT retweeted_tweet_id AS id, count(*) AS count
            FROM cockt.retweet
            GROUP BY 1
        ),
		reply AS (
            SELECT in_reply_to_tweet_id AS id, count(*) AS count
            FROM cockt.reply
            GROUP BY 1
		),
		quote_ AS (
            SELECT quoted_tweet_id AS id, count(*) AS count
            FROM cockt.quote
            GROUP BY 1
		)
        SELECT
            id,
            created_at,
            published_time,
            user_id,
            user_name,
            user_screen_name,
            text,
            source,
            language,
            coordinates_longitude,
            coordinates_latitude,
            possibly_sensitive,
            h.hashtags,
            u.urls,
            COALESCE(rt.count, 0) AS retweet_count,
            COALESCE(reply.count, 0) AS reply_count,
			COALESCE(quote_.count, 0) AS quote_count
        FROM
          cockt.tweet
            LEFT JOIN (
                SELECT
                    tweet_id as id,
                    array_agg(hashtag) as hashtags
                FROM
                cockt.tweet_hashtag
                GROUP BY
                    tweet_id
            ) h USING (id)
            LEFT JOIN (
              SELECT
                  tweet_id as id,
                  array_agg(url) as urls
              FROM
              cockt.tweet_url
              GROUP BY
                  tweet_id
          ) u USING (id)
            LEFT JOIN rt USING(id)
			LEFT JOIN reply USING(id)
			LEFT JOIN quote_ USING(id)
        WHERE
            user_name is not null AND user_screen_name is not null
          AND
            date(created_at) = '{date}'
    "#,
    date = date.unwrap().to_string()
  );

  let mut rows = sqlx::query_as::<_, Tweet>(&query).fetch(&pool);

  let mut writer = BufWriter::new(stdout());
  while let Some(row) = rows.try_next().await? {
    serde_json::to_writer(&mut writer, &row)?;
    writer.write_all(b"\n")?;
  }
  Ok(())
}
