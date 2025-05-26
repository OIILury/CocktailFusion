use std::io::{stdout, BufWriter, Write};
use futures::TryStreamExt;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool, Row};
use tracing::{error, debug};

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
  #[serde(default)]
  hashtags: Option<Vec<String>>,
  #[serde(default)]
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

  let schema_name = std::env::var("SCHEMA_NAME").expect("SCHEMA_NAME must be set");
  debug!("Utilisation du schéma: {}", schema_name);

  let pool = PgPool::connect(&pg_url).await?;
  debug!("Connexion à la base de données établie");

  // Vérifier si les tables existent
  let tables_exist = sqlx::query(&format!(
    "SELECT EXISTS (
      SELECT FROM information_schema.tables 
      WHERE table_schema = '{}' 
      AND table_name = 'tweet'
    )",
    schema_name
  ))
  .fetch_one(&pool)
  .await?;

  if !tables_exist.get::<bool, _>(0) {
    error!("Les tables n'existent pas dans le schéma {}", schema_name);
    return Ok(());
  }

  // Compter le nombre de tweets
  let count: i64 = sqlx::query(&format!(
    "SELECT COUNT(*) FROM {}.tweet",
    schema_name
  ))
  .fetch_one(&pool)
  .await?
  .get(0);

  debug!("Nombre de tweets trouvés: {}", count);

  let query = format!(
    r#"
        WITH rt as (
            SELECT retweeted_tweet_id AS id, count(*) AS count
            FROM {schema_name}.retweet
            GROUP BY 1
        ),
		reply AS (
            SELECT in_reply_to_tweet_id AS id, count(*) AS count
            FROM {schema_name}.reply
            GROUP BY 1
		),
		quote_ AS (
            SELECT quoted_tweet_id AS id, count(*) AS count
            FROM {schema_name}.quote
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
          {schema_name}.tweet
            LEFT JOIN (
                SELECT
                    tweet_id as id,
                    COALESCE(array_agg(hashtag), ARRAY[]::text[]) as hashtags
                FROM
                {schema_name}.tweet_hashtag
                GROUP BY
                    tweet_id
            ) h USING (id)
            LEFT JOIN (
              SELECT
                  tweet_id as id,
                  COALESCE(array_agg(url), ARRAY[]::text[]) as urls
              FROM
              {schema_name}.tweet_url
              GROUP BY
                  tweet_id
          ) u USING (id)
            LEFT JOIN rt USING(id)
			LEFT JOIN reply USING(id)
			LEFT JOIN quote_ USING(id)
        WHERE
            user_name is not null AND user_screen_name is not null
    "#,
    schema_name = schema_name
  );

  debug!("Exécution de la requête SQL");
  let mut rows = sqlx::query_as::<_, Tweet>(&query).fetch(&pool);
  let mut count = 0;

  let mut writer = BufWriter::new(stdout());
  while let Some(row) = rows.try_next().await? {
    serde_json::to_writer(&mut writer, &row)?;
    writer.write_all(b"\n")?;
    count += 1;
  }
  debug!("{} tweets exportés", count);
  Ok(())
}
