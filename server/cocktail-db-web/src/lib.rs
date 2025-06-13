use std::fmt::Debug;
use chrono::{Local, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{types::Json, Decode, SqlitePool};
use std::collections::HashSet;

use chronoutil::shift_months;
use fts::FrequenceByDate;
use uuid::Uuid;

pub use fts::Bloc;

pub use migration::*;

mod migration;

#[derive(Debug, Clone)]
pub struct WebDatabase(SqlitePool);

impl WebDatabase {
  pub fn new(pool: SqlitePool) -> Self {
    Self(pool)
  }
}

impl AsRef<SqlitePool> for WebDatabase {
  fn as_ref(&self) -> &SqlitePool {
    &self.0
  }
}

#[tracing::instrument]
pub async fn projects<S>(pool: S, user_id: &String) -> sqlx::Result<Vec<Project>>
where
  S: AsRef<SqlitePool> + Debug,
{
  sqlx::query_as!(
    Project,
    r#"
        SELECT project_id AS "project_id!: String",
            user_id AS "user_id!: String",
            title AS "title!: String",
            event_count AS "event_count!: i64",
            tweets_count AS "tweets_count!: i64",
            authors_count AS "authors_count!: i64",
            updated_at AS "updated_at!: NaiveDate",
            start_date AS "start_date!: NaiveDate",
            end_date AS "end_date!: NaiveDate",
            is_custom_date AS "is_custom_date!: i64",
            hashtag_list AS "hashtag_list!: Json<HashSet<HashtagWithCount>>",
            exclude_hashtag_list AS "exclude_hashtag_list!: Json<HashSet<HashtagWithCount>>",
            request_params AS "request_params!: Json<Vec<Vec<Bloc>>>",
            is_analyzed AS "is_analyzed!: i64"
        FROM project
        WHERE user_id = $1
        "#,
    user_id
  )
  .fetch_all(pool.as_ref())
  .await
}

#[tracing::instrument]
pub async fn project<S>(pool: S, project_id: String, user_id: &String) -> sqlx::Result<Project>
where
  S: AsRef<SqlitePool> + Debug,
{
  sqlx::query_as!(
    Project,
    r#"
        SELECT project_id AS "project_id!: String",
            user_id AS "user_id!: String",
            title AS "title!: String",
            event_count AS "event_count!: i64",
            tweets_count AS "tweets_count!: i64",
            authors_count AS "authors_count!: i64",
            updated_at AS "updated_at!: NaiveDate",
            start_date AS "start_date!: NaiveDate",
            end_date AS "end_date!: NaiveDate",
            is_custom_date AS "is_custom_date!: i64",
            hashtag_list AS "hashtag_list!: Json<HashSet<HashtagWithCount>>",
            exclude_hashtag_list AS "exclude_hashtag_list!: Json<HashSet<HashtagWithCount>>",
            request_params AS "request_params!: Json<Vec<Vec<Bloc>>>",
            is_analyzed AS "is_analyzed!: i64"
        FROM project
        WHERE project_id = $1 AND user_id = $2"#,
    project_id,
    user_id
  )
  .fetch_one(pool.as_ref())
  .await
}

#[tracing::instrument]
pub async fn update_project_title<S>(
  pool: S,
  project_id: String,
  user_id: &String,
  title: &str,
) -> sqlx::Result<i64>
where
  S: AsRef<SqlitePool> + Debug,
{
  let today = chrono::offset::Local::now().date_naive().format("%Y-%m-%d").to_string();

  let result = sqlx::query!(
    r#"UPDATE project SET title = $1, updated_at = $2 WHERE project_id = $3 AND user_id = $4"#,
    title,
    today,
    project_id,
    user_id
  )
  .execute(pool.as_ref())
  .await?;

  Ok(result.rows_affected() as i64)
}

#[tracing::instrument]
pub async fn update_project_daterange<S>(
  pool: S,
  project_id: String,
  user_id: &String,
  start_date: &String,
  end_date: &String,
  is_custom_date: i64,
) -> sqlx::Result<i64>
where
  S: AsRef<SqlitePool> + Debug,
{
  let today = chrono::offset::Local::now().date_naive().format("%Y-%m-%d").to_string();

  let result = sqlx::query!(
        r#"UPDATE project SET start_date = $1, end_date = $2, is_custom_date = $3, updated_at = $4 WHERE project_id = $5 AND user_id = $6"#,
        start_date,
        end_date,
        is_custom_date,
        today,
        project_id,
        user_id
    )
    .execute(pool.as_ref())
    .await?;

  Ok(result.rows_affected() as i64)
}

#[tracing::instrument]
pub async fn update_project_request_params<S>(
  pool: S,
  project_id: String,
  user_id: &String,
  request_params: Vec<Vec<Bloc>>,
) -> sqlx::Result<i64>
where
  S: AsRef<SqlitePool> + Debug,
{
  let today = chrono::offset::Local::now().date_naive().format("%Y-%m-%d").to_string();

  let json_request_params = Json(request_params);

  let result = sqlx::query!(
    r#"
        UPDATE project SET request_params = $1,
            updated_at = $2
        WHERE project_id = $3 AND user_id = $4"#,
    json_request_params,
    today,
    project_id,
    user_id
  )
  .execute(pool.as_ref())
  .await?;

  Ok(result.rows_affected() as i64)
}

#[tracing::instrument]
pub async fn create_project<S>(pool: S, project: Project) -> sqlx::Result<i64>
where
  S: AsRef<SqlitePool> + Debug,
{
  let today = chrono::offset::Local::now().date_naive().format("%Y-%m-%d").to_string();

  let mut tx = pool.as_ref().begin().await?;

  let result = sqlx::query!(
        r#"
INSERT INTO project (project_id, user_id, title, event_count, tweet_count, updated_at, start_date, end_date)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
    "#,
        project.project_id,
        project.user_id,
        project.title,
        project.event_count,
        project.tweets_count,
        today,
        project.start_date,
        project.end_date,
    )
    .execute(&mut *tx)
    .await?;

  tx.commit().await?;

  Ok(result.rows_affected() as i64)
}

#[tracing::instrument]
pub async fn delete_project<S>(
  pool: S,
  project_id: String,
  user_id: &String,
) -> sqlx::Result<i64>
where
  S: AsRef<SqlitePool> + Debug,
{
  let result = sqlx::query!(
    r#"
    DELETE FROM project
    WHERE project_id = $1
    AND user_id = $2
    "#,
    project_id,
    user_id
  )
  .execute(pool.as_ref())
  .await?;

  Ok(result.rows_affected() as i64)
}

#[tracing::instrument]
pub async fn delete_anonymous_project<S>(pool: S, project_id: String) -> sqlx::Result<i64>
where
  S: AsRef<SqlitePool> + Debug,
{
  let result = sqlx::query!(
    r#"
    DELETE FROM project
    WHERE project_id = $1
    "#,
    project_id,
  )
  .execute(pool.as_ref())
  .await?;

  Ok(result.rows_affected() as i64)
}

#[tracing::instrument]
pub async fn delete_chart<S>(pool: S, project_id: String) -> sqlx::Result<()>
where
  S: AsRef<SqlitePool> + Debug,
{
  sqlx::query!(
    r#"
    DELETE FROM chart
    WHERE project_id = $1
    "#,
    project_id,
  )
  .execute(pool.as_ref())
  .await?;

  Ok(())
}

#[tracing::instrument]
pub async fn rename_project<S>(
  pool: S,
  project_id: String,
  user_id: &String,
  title: &str,
) -> sqlx::Result<()>
where
  S: AsRef<SqlitePool> + Debug,
{
  let today = chrono::offset::Local::now().date_naive().format("%Y-%m-%d").to_string();
  sqlx::query!(
    r#"UPDATE project SET title = $1, updated_at = $2 WHERE project_id = $3 AND user_id = $4"#,
    title,
    today,
    project_id,
    user_id
  )
  .execute(pool.as_ref())
  .await?;

  Ok(())
}

#[tracing::instrument]
pub async fn duplicate_project<S>(
  pool: S,
  project_id: String,
  new_project_id: String,
  user_id: &String,
  title: &str,
) -> sqlx::Result<()>
where
  S: AsRef<SqlitePool> + Debug,
{
  sqlx::query!(
    r#"
      INSERT INTO project(project_id, title, user_id, event_count, tweet_count, start_date, 
        end_date, is_custom_date, hashtag_list, exclude_hashtag_list) 
      SELECT $1, $2, $4, event_count, tweet_count, start_date, end_date, 
        is_custom_date, hashtag_list, exclude_hashtag_list
      FROM project
      WHERE project_id = $3 AND user_id = $4"#,
    new_project_id,
    title,
    project_id,
    user_id
  )
  .execute(pool.as_ref())
  .await?;

  Ok(())
}

#[tracing::instrument]
pub async fn add_hashtag<S>(
  pool: S,
  project_id: String,
  user_id: &String,
  hashtag: &str,
  count: i64,
  do_add: bool,
  exclude: bool,
) -> sqlx::Result<()>
where
  S: AsRef<SqlitePool> + Debug,
{
  let mut tx = pool.as_ref().begin().await?;
  
  let mut hashtags_row = sqlx::query!(
    r#"
        SELECT hashtag_list AS "hashtag_list!: Json<HashSet<HashtagWithCount>>",
               complete_hashtag_list AS "complete_hashtag_list!: Json<HashSet<HashtagWithCount>>",
               exclude_hashtag_list AS "exclude_hashtag_list!: Json<HashSet<HashtagWithCount>>"
        FROM "project" WHERE project_id = $1 AND user_id = $2"#,
    project_id,
    user_id
  )
  .fetch_one(&mut *tx)
  .await?;

  let hashtag_with_count = HashtagWithCount {
    name: hashtag.to_string(),
    count,
  };

  match (do_add, exclude) {
    (true, true) => {
      hashtags_row
        .exclude_hashtag_list
        .insert(hashtag_with_count.clone());
    }
    (true, false) => {
      hashtags_row.hashtag_list.insert(hashtag_with_count.clone());
    }
    (false, true) => {
      hashtags_row
        .exclude_hashtag_list
        .remove(&hashtag_with_count);
    }
    (false, false) => {
      hashtags_row.hashtag_list.remove(&hashtag_with_count);
    }
  }

  let hashtag_list: HashSet<_> = hashtags_row.hashtag_list.iter().collect();
  let exclude_hashtag_list: HashSet<_> = hashtags_row.exclude_hashtag_list.iter().collect();
  let mut complete_hashtag_list = hashtags_row
    .complete_hashtag_list
    .0
    .iter()
    .collect::<HashSet<_>>();

  if do_add {
    complete_hashtag_list.insert(&hashtag_with_count)
  } else {
    complete_hashtag_list.remove(&hashtag_with_count)
  };

  let hashtag_list = Json(hashtag_list.clone());
  let complete_hashtag_list = Json(complete_hashtag_list);
  let exclude_hashtag_list = Json(exclude_hashtag_list);

  sqlx::query!(
        r#"UPDATE project SET hashtag_list = $1, complete_hashtag_list = $2, exclude_hashtag_list = $3 WHERE project_id = $4 AND user_id = $5"#,
        hashtag_list,
        complete_hashtag_list,
        exclude_hashtag_list,
        project_id,
        user_id
    )
    .execute(&mut *tx)
    .await?;

  tx.commit().await?;
  Ok(())
}

#[derive(Debug)]
pub struct HiddenElementsRow {
  pub list: Json<HashSet<String>>,
}

#[derive(Debug)]
pub struct HiddenElementTweetsList {
  pub hidden_hashtag_list: Vec<String>,
  pub hidden_author_list: Vec<String>,
}

#[tracing::instrument]
pub async fn toggle_hashtag<S>(
  pool: S,
  project_id: String,
  user_id: &String,
  hashtag: &str,
  hide: bool,
) -> sqlx::Result<i64>
where
  S: AsRef<SqlitePool> + Debug,
{
  let mut tx = pool.as_ref().begin().await?;

  let hidden_hashtag_list: HiddenElementsRow = sqlx::query_as!(
    HiddenElementsRow,
    r#"
    SELECT hidden_hashtag_list AS "list!: Json<HashSet<String>>"
    FROM project
    WHERE project_id = $1 AND user_id = $2
    "#,
    project_id,
    user_id
  )
  .fetch_one(&mut *tx)
  .await?;

  let mut hidden_hashtag_list = hidden_hashtag_list.list.0;
  if hide {
    hidden_hashtag_list.insert(hashtag.to_string());
  } else {
    hidden_hashtag_list.remove(hashtag);
  }
  let today = chrono::offset::Local::now().date_naive().format("%Y-%m-%d").to_string();

  let json_data = Json(hidden_hashtag_list);
  let result = sqlx::query!(
    r#"
    UPDATE project SET hidden_hashtag_list = $1, updated_at = $2
    WHERE project_id = $3 AND user_id = $4
    "#,
    json_data,
    today,
    project_id,
    user_id
  )
  .execute(&mut *tx)
  .await?;

  tx.commit().await?;

  Ok(result.rows_affected() as i64)
}

#[tracing::instrument]
pub async fn toggle_all<S>(
  pool: S,
  project_id: String,
  user_id: &String,
  hide: bool,
  hashtags: Vec<String>,
) -> sqlx::Result<i64>
where
  S: AsRef<SqlitePool> + Debug,
{
  let mut tx = pool.as_ref().begin().await?;

  let hidden_hashtag_list: HiddenElementsRow = sqlx::query_as!(
    HiddenElementsRow,
    r#"
    SELECT hidden_hashtag_list AS "list!: Json<HashSet<String>>"
    FROM project
    WHERE project_id = $1 AND user_id = $2
    "#,
    project_id,
    user_id
  )
  .fetch_one(&mut *tx)
  .await?;

  let mut hidden_hashtag_list = hidden_hashtag_list.list.0;
  for hashtag in hashtags {
    if hide {
      hidden_hashtag_list.insert(hashtag);
    } else {
      hidden_hashtag_list.remove(&hashtag);
    }
  }
  let today = chrono::offset::Local::now().date_naive().format("%Y-%m-%d").to_string();

  let json_data = Json(hidden_hashtag_list);
  let result = sqlx::query!(
    r#"
    UPDATE project SET hidden_hashtag_list = $1, updated_at = $2
    WHERE project_id = $3 AND user_id = $4
    "#,
    json_data,
    today,
    project_id,
    user_id
  )
  .execute(&mut *tx)
  .await?;

  tx.commit().await?;

  Ok(result.rows_affected() as i64)
}

#[tracing::instrument]
pub async fn hashtag_list<S>(
  pool: S,
  project_id: String,
  user_id: &String,
) -> sqlx::Result<Vec<HashtagWithCount>>
where
  S: AsRef<SqlitePool> + Debug,
{
  let result = sqlx::query!(
    r#"
        SELECT hashtag_list AS "hashtag_list!: Json<HashSet<HashtagWithCount>>"
        FROM "project" WHERE project_id = $1 AND user_id = $2"#,
    project_id,
    user_id
  )
  .fetch_one(pool.as_ref())
  .await?;

  let mut ordered: Vec<HashtagWithCount> = Vec::from_iter(result.hashtag_list.0.clone());
  ordered.sort_by(|h1, h2| h1.name.cmp(&h2.name));

  Ok(ordered)
}

#[tracing::instrument]
pub async fn hashtag_list_premium_request<S>(
  pool: S,
  project_id: String,
  user_id: &String,
  block_id: i32,
) -> sqlx::Result<Vec<String>>
where
  S: AsRef<SqlitePool> + Debug,
{
  let project = project(pool, project_id, user_id).await?;

  let mut i = 0;
  let mut index = block_id as usize;
  if block_id == -1 {
    i = 1;
    index = 0
  }
  let mut hashtags = Vec::from_iter(project.request_params[i][index].data.clone());

  hashtags.retain(|element| element.starts_with("#"));

  hashtags
    .iter_mut()
    .for_each(|element| *element = element[1..].to_string());

  Ok(hashtags)
}

#[tracing::instrument]
pub async fn hidden_hashtag_list<S>(
  pool: S,
  project_id: String,
  user_id: &String,
) -> sqlx::Result<Vec<String>>
where
  S: AsRef<SqlitePool> + Debug,
{
  let result = sqlx::query!(
    r#"
        SELECT hidden_hashtag_list AS "hidden_hashtag_list!: Json<HashSet<String>>"
        FROM "project" WHERE project_id = $1 AND user_id = $2"#,
    project_id,
    user_id
  )
  .fetch_one(pool.as_ref())
  .await?;

  Ok(Vec::from_iter(result.hidden_hashtag_list.0))
}

#[tracing::instrument]
pub async fn hidden_hashtag_tweet_list<S>(
  pool: S,
  project_id: String,
  user_id: &String,
) -> sqlx::Result<HiddenElementTweetsList>
where
  S: AsRef<SqlitePool> + Debug,
{
  let hidden_hashtag_list: HiddenElementsRow = sqlx::query_as!(
    HiddenElementsRow,
    r#"SELECT hidden_hashtag_tweets_list AS "list!: Json<HashSet<String>>" FROM "project" WHERE project_id = $1 AND user_id = $2"#,
    project_id,
    user_id
  )
  .fetch_one(pool.as_ref())
  .await?;

  let hidden_author_list: HiddenElementsRow = sqlx::query_as!(
    HiddenElementsRow,
    r#"SELECT hidden_author_tweets_list AS "list!: Json<HashSet<String>>" FROM "project" WHERE project_id = $1 AND user_id = $2"#,
    project_id,
    user_id
  )
  .fetch_one(pool.as_ref())
  .await?;

  Ok(HiddenElementTweetsList {
    hidden_hashtag_list: hidden_hashtag_list.list.0.into_iter().collect(),
    hidden_author_list: hidden_author_list.list.0.into_iter().collect(),
  })
}

#[tracing::instrument]
pub async fn hidden_hashtag_tweet_graph_list<S>(
  pool: S,
  project_id: String,
  user_id: &String,
) -> sqlx::Result<Vec<String>>
where
  S: AsRef<SqlitePool> + Debug,
{
  let hidden_hashtag_list: HiddenElementsRow = sqlx::query_as!(
    HiddenElementsRow,
    r#"SELECT hidden_hashtag_tweets_graph_list AS "list!: Json<HashSet<String>>" FROM "project" WHERE project_id = $1 AND user_id = $2"#,
    project_id,
    user_id
  )
  .fetch_one(pool.as_ref())
  .await?;

  Ok(hidden_hashtag_list.list.0.into_iter().collect())
}

#[tracing::instrument]
pub async fn toggle_hashtag_tweets_list<S>(
  pool: S,
  project_id: String,
  user_id: &String,
  hashtags: Vec<String>,
  hide: bool,
) -> sqlx::Result<i64>
where
  S: AsRef<SqlitePool> + Debug,
{
  let mut tx = pool.as_ref().begin().await?;

  let hidden_hashtag_list: HiddenElementsRow = sqlx::query_as!(
    HiddenElementsRow,
    r#"SELECT hidden_hashtag_tweets_list AS "list!: Json<HashSet<String>>" FROM "project" WHERE project_id = $1 AND user_id = $2"#,
    project_id,
    user_id
  )
  .fetch_one(&mut *tx)
  .await?;

  let mut hidden_hashtag_list = hidden_hashtag_list.list.0;
  for hashtag in hashtags {
    if hide {
      hidden_hashtag_list.insert(hashtag);
    } else {
      hidden_hashtag_list.remove(&hashtag);
    }
  }
  let today = chrono::offset::Local::now().date_naive().format("%Y-%m-%d").to_string();

  let json_data = Json(hidden_hashtag_list);
  let result = sqlx::query!(
    r#"
    UPDATE project SET hidden_hashtag_tweets_list = $1, updated_at = $2
    WHERE project_id = $3 AND user_id = $4
    "#,
    json_data,
    today,
    project_id,
    user_id
  )
  .execute(&mut *tx)
  .await?;

  tx.commit().await?;

  Ok(result.rows_affected() as i64)
}

#[tracing::instrument]
pub async fn toggle_author_tweets_list<S>(
  pool: S,
  project_id: String,
  user_id: &String,
  authors: Vec<String>,
  hide: bool,
) -> sqlx::Result<i64>
where
  S: AsRef<SqlitePool> + Debug,
{
  let mut tx = pool.as_ref().begin().await?;

  let hidden_author_list: HiddenElementsRow = sqlx::query_as!(
    HiddenElementsRow,
    r#"SELECT hidden_author_tweets_list AS "list!: Json<HashSet<String>>" FROM "project" WHERE project_id = $1 AND user_id = $2"#,
    project_id,
    user_id
  )
  .fetch_one(&mut *tx)
  .await?;

  let mut hidden_author_list = hidden_author_list.list.0;
  for author in authors {
    if hide {
      hidden_author_list.insert(author);
    } else {
      hidden_author_list.remove(&author);
    }
  }
  let today = chrono::offset::Local::now().date_naive().format("%Y-%m-%d").to_string();

  let json_data = Json(hidden_author_list);
  let result = sqlx::query!(
    r#"
    UPDATE project SET hidden_author_tweets_list = $1, updated_at = $2
    WHERE project_id = $3 AND user_id = $4
    "#,
    json_data,
    today,
    project_id,
    user_id
  )
  .execute(&mut *tx)
  .await?;

  tx.commit().await?;

  Ok(result.rows_affected() as i64)
}

#[tracing::instrument]
pub async fn toggle_hashtag_tweets_graph_list<S>(
  pool: S,
  project_id: String,
  user_id: &String,
  hashtags: Vec<String>,
  hide: bool,
) -> sqlx::Result<i64>
where
  S: AsRef<SqlitePool> + Debug,
{
  let mut tx = pool.as_ref().begin().await?;

  let hidden_hashtag_list: HiddenElementsRow = sqlx::query_as!(
    HiddenElementsRow,
    r#"SELECT hidden_hashtag_tweets_graph_list AS "list!: Json<HashSet<String>>" FROM "project" WHERE project_id = $1 AND user_id = $2"#,
    project_id,
    user_id
  )
  .fetch_one(&mut *tx)
  .await?;

  let mut hidden_hashtag_list = hidden_hashtag_list.list.0;
  for hashtag in hashtags {
    if hide {
      hidden_hashtag_list.insert(hashtag);
    } else {
      hidden_hashtag_list.remove(&hashtag);
    }
  }
  let today = chrono::offset::Local::now().date_naive().format("%Y-%m-%d").to_string();

  let json_data = Json(hidden_hashtag_list);
  let result = sqlx::query!(
    r#"
    UPDATE project SET hidden_hashtag_tweets_graph_list = $1, updated_at = $2
    WHERE project_id = $3 AND user_id = $4
    "#,
    json_data,
    today,
    project_id,
    user_id
  )
  .execute(&mut *tx)
  .await?;

  tx.commit().await?;

  Ok(result.rows_affected() as i64)
}

#[tracing::instrument]
pub async fn exclude_hashtag_list<S>(
  pool: S,
  project_id: String,
  user_id: &String,
) -> sqlx::Result<Vec<HashtagWithCount>>
where
  S: AsRef<SqlitePool> + Debug,
{
  let result = sqlx::query!(
    r#"
        SELECT exclude_hashtag_list AS "exclude_hashtag_list!: Json<HashSet<HashtagWithCount>>"
        FROM "project" WHERE project_id = $1 AND user_id = $2"#,
    project_id,
    user_id
  )
  .fetch_one(pool.as_ref())
  .await?;

  Ok(Vec::from_iter(result.exclude_hashtag_list.0))
}

#[tracing::instrument]
pub async fn exclude_hashtag_count<S>(
  pool: S,
  project_id: String,
  user_id: &String,
) -> sqlx::Result<i64>
where
  S: AsRef<SqlitePool> + Debug,
{
  let r = sqlx::query!(
    r#"
        SELECT CAST(COALESCE(json_array_length(exclude_hashtag_list), 0) AS INTEGER) as "total!: i64"
        FROM project
        WHERE project_id = $1 AND user_id = $2
        "#,
    project_id,
    user_id
  )
  .fetch_one(pool.as_ref())
  .await?;

  Ok(r.total)
}

#[tracing::instrument]
pub async fn include_hashtag_count<S>(
  pool: S,
  project_id: String,
  user_id: &String,
) -> sqlx::Result<i64>
where
  S: AsRef<SqlitePool> + Debug,
{
  let r = sqlx::query!(
    r#"
        SELECT CAST(COALESCE(json_array_length(hashtag_list), 0) AS INTEGER) as "total!: i64"
        FROM project
        WHERE project_id = $1 AND user_id = $2
        "#,
    project_id,
    user_id
  )
  .fetch_one(pool.as_ref())
  .await?;

  Ok(r.total)
}

#[tracing::instrument]
pub async fn include_exclude_hashtag_count<S>(
  pool: S,
  project_id: String,
  user_id: &String,
) -> sqlx::Result<(i64, i64)>
where
  S: AsRef<SqlitePool> + Debug,
{
  let r = sqlx::query!(
    r#"
        SELECT 
            CAST(COALESCE(json_array_length(hashtag_list), 0) AS INTEGER) as "include_count!: i64",
            CAST(COALESCE(json_array_length(exclude_hashtag_list), 0) AS INTEGER) as "exclude_count!: i64"
        FROM project
        WHERE project_id = $1 AND user_id = $2
        "#,
    project_id,
    user_id
  )
  .fetch_one(pool.as_ref())
  .await?;

  Ok((r.include_count, r.exclude_count))
}

#[derive(Debug)]
pub struct CorporaRow {
  pub corpus_list: Json<HashSet<String>>,
  pub complete_hashtag_list: Json<HashSet<String>>,
}

pub struct HashtagTimeSeries {
  pub complete_hashtag_list: Json<HashSet<String>>,
  pub hidden_hashtag_list: Json<HashSet<String>>,
}

#[tracing::instrument]
pub async fn validate_project_analysis<S>(
  pool: S,
  project_id: String,
  user_id: &String,
  tweets_count: i64,
  authors_count: i64,
) -> sqlx::Result<()>
where
  S: AsRef<SqlitePool> + Debug,
{
  sqlx::query!(
    r#"UPDATE project SET is_analyzed = 1,
      tweets_count = $1,
      authors_count = $2,
      hidden_hashtag_list = '[]',
      hidden_hashtag_tweets_list = '[]',
      hidden_author_tweets_list = '[]',
      hidden_hashtag_tweets_graph_list = '[]' 
    WHERE project_id = $3  AND user_id = $4"#,
    tweets_count,
    authors_count,
    project_id,
    user_id
  )
  .execute(pool.as_ref())
  .await?;

  Ok(())
}

#[tracing::instrument]
pub async fn save_chart<S>(
  pool: S,
  project_id: String,
  title: String,
  tab: String,
  chart: impl Serialize + Debug,
) -> sqlx::Result<()>
where
  S: AsRef<SqlitePool> + Debug,
{
  let json = serde_json::to_string(&chart).unwrap();
  let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

  sqlx::query!(
    r#"
INSERT OR REPLACE INTO chart (project_id, title, tab, json, date)
VALUES ($1, $2, $3, $4, $5)
    "#,
    project_id,
    title,
    tab,
    json,
    now,
  )
  .execute(pool.as_ref())
  .await?;

  Ok(())
}

#[tracing::instrument]
pub async fn get_chart<S>(
  pool: S,
  project_id: &String,
  title: &String,
  tab: &String,
) -> sqlx::Result<String>
where
  S: AsRef<SqlitePool> + Debug,
{
  let data = sqlx::query!(
    r#"
SELECT json FROM chart WHERE project_id = $1 AND title = $2 AND tab = $3
"#,
    project_id,
    title,
    tab,
  )
  .fetch_one(pool.as_ref())
  .await?;

  Ok(data.json)
}

#[tracing::instrument]
pub async fn get_anonymous_projects_to_clear<S>(pool: S) -> sqlx::Result<Vec<ProjectId>>
where
  S: AsRef<SqlitePool> + Debug,
{
  let today = chrono::offset::Local::now().date_naive().format("%Y-%m-%d").to_string();

  sqlx::query_as!(
    ProjectId,
    r#"
      SELECT project_id AS "project_id: String"
      FROM project
      WHERE updated_at < $1
        AND user_id NOT LIKE '%_@__%.__%'
    "#,
    today
  )
  .fetch_all(pool.as_ref())
  .await
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TweetsChart {
  pub data: Vec<FrequenceByDate>,
}

#[derive(Debug, Clone, Decode, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct HashtagWithCount {
  pub name: String,
  pub count: i64,
}

#[derive(Debug, Clone)]
pub struct ProjectId {
  pub project_id: String,
}

#[derive(Debug, Clone)]
pub struct Project {
  pub project_id: String,
  pub user_id: String,
  pub title: String,
  pub event_count: i64,
  pub tweets_count: i64,
  pub authors_count: i64,
  pub updated_at: NaiveDate,
  pub start_date: NaiveDate,
  pub end_date: NaiveDate,
  pub is_custom_date: i64,
  pub hashtag_list: Json<HashSet<HashtagWithCount>>,
  pub exclude_hashtag_list: Json<HashSet<HashtagWithCount>>,
  pub request_params: Json<Vec<Vec<Bloc>>>,
  pub is_analyzed: i64,
}

impl Default for Project {
  fn default() -> Self {
    let now = Utc::now();
    Self {
      project_id: Uuid::new_v4().to_string(),
      user_id: Default::default(),
      title: Default::default(),
      event_count: Default::default(),
      tweets_count: Default::default(),
      authors_count: Default::default(),
      updated_at: now.date_naive(),
      start_date: shift_months(now.date_naive(), -6),
      end_date: now.date_naive(),
      is_custom_date: Default::default(),
      hashtag_list: Json::default(),
      exclude_hashtag_list: Json::default(),
      request_params: Json::default(),
      is_analyzed: Default::default(),
    }
  }
}

pub struct ParsedProjectCriteria {
  pub hashtag_list: Vec<String>,
  pub exclude_hashtag_list: Vec<String>,
}

impl From<&Project> for ParsedProjectCriteria {
  fn from(project: &Project) -> Self {
    let mut hashtag_list: Vec<String> = project
      .hashtag_list
      .iter()
      .map(|e| e.name.to_string())
      .collect();

    project.request_params[0].iter().for_each(|bloc| {
      let mut param_hashtags: Vec<String> = bloc
        .data
        .iter()
        .filter_map(|value| {
          if value.starts_with("#") {
            return Some(value[1..].to_string());
          }
          None
        })
        .collect::<Vec<String>>();
      hashtag_list.append(&mut param_hashtags);
    });
    hashtag_list.sort_unstable();
    hashtag_list.dedup();

    ParsedProjectCriteria {
      hashtag_list,
      exclude_hashtag_list: project
        .exclude_hashtag_list
        .iter()
        .map(|e| e.name.to_string())
        .collect(),
    }
  }
}
