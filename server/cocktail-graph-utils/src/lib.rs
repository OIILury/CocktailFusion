use std::{fs::File, path::PathBuf, process::Command, str::FromStr};

use error::GraphError;
use fts::{DocAddress, Index};
use serde::{Deserialize, Serialize};
use sqlx::{postgres::PgPoolOptions, types::chrono::NaiveDateTime, FromRow, PgPool};

pub mod error;

#[derive(Debug, Deserialize, Default, FromRow)]
pub struct JsonDataGraph {
  pub nodes: serde_json::Value,
  pub edges: serde_json::Value,
}

#[derive(Debug, Deserialize, Default, FromRow)]
pub struct TableStatus {
  pub exists: bool,
}

#[derive(Debug, Deserialize, Serialize, FromRow)]
pub struct Status {
  pub datetime: NaiveDateTime,
  pub status: String,
}

#[derive(Debug, Deserialize, Serialize, FromRow)]
pub struct Graph {
  pub modularity: f64,
}

#[derive(Debug)]
pub struct GraphGenerator {
  database_url: String,
  schema: String,
  r_script: PathBuf,
  python_script: PathBuf,
  graph_name: String,
  community: String,
  centrality: String,
  max_rank: i64,
  show_interaction: bool,
}

impl GraphGenerator {
  pub fn new<P: AsRef<str>>(
    database_url: String,
    schema: P,
    r_script: PathBuf,
    python_script: PathBuf,
    graph_name: String,
    community: String,
    centrality: String,
    max_rank: i64,
    show_interaction: bool,
  ) -> Self {
    Self {
      database_url,
      schema: schema.as_ref().to_string(),
      r_script,
      python_script,
      graph_name,
      community,
      centrality,
      max_rank,
      show_interaction,
    }
  }

  #[tracing::instrument]
  pub async fn process_single_graph(&self) -> Result<(), GraphError> {
    let pool = self.get_pg_pool().await?;
    let status = self.get_status(&pool).await?;

    if status == Some("done".to_string()) {
      tracing::event!(tracing::Level::INFO, "Etude déjà analysée");
      return Err(GraphError::Searcher("Deja calculé".to_string()));
    }
    self.set_status_started(&pool).await?;

    self.create_viz().await?;

    pool.close().await;

    Ok(())
  }

  #[tracing::instrument]
  pub async fn process_search(&self) -> Result<JsonDataGraph, GraphError> {
    tracing::event!(tracing::Level::INFO, "inside process_search");
    // "postgres://cocktailuser:cocktailuser@localhost:5432/cocktail_pg",
    let pool = self.get_pg_pool().await?;

    let directory_path = PathBuf::from_str(format!("project-data/{}", self.schema).as_str())?;

    let index = fts::retrieve_index(&directory_path)?;

    let docs = fts::search_for_communities(&index)?;

    if docs.0.is_empty() {
      return Err(GraphError::Searcher("aucun document".to_string()));
    }

    self.delete_schema_c_est_mal(&pool).await?;
    self.create_schema(&pool).await?;
    self.set_status_started(&pool).await?;
    self.create_tmp_table(&pool).await?;
    self.copy_tweet_ids_to_table(&index, &pool, docs.0).await?;
    self
      .create_schema_and_stuff_for_the_current_search_query(&pool)
      .await?;
    self.create_graph_db(&pool).await?;
    self.create_data_graph().await?;
    let json_data = self.create_viz().await?;

    pool.close().await;

    Ok(json_data)
  }

  #[tracing::instrument(skip_all)]
  async fn copy_tweet_ids_to_table(
    &self,
    index: &Index,
    pool: &PgPool,
    docs: Vec<(f32, DocAddress)>,
  ) -> Result<(), GraphError> {
    let schema = index.schema();
    let id = schema.get_field("id").unwrap(); // unwrap car **on sait** que id est un champ dans l'index
    let searcher = index
      .reader()
      .map_err(|e| GraphError::Searcher(format!("Searcher: {e:?}")))?
      .searcher();

    let v: String = docs
      .iter()
      .flat_map(|(_score, doc_address)| {
        if let Ok(retrieved_doc) = searcher.doc(*doc_address) {
          retrieved_doc
            .get_first(id)
            .and_then(|i| i.as_text())
            .map(|s| s.to_string())
        } else {
          None
        }

        // format!("{}\n", iii.unwrap().as_text().unwrap_or_default())
      })
      .collect::<Vec<_>>()
      .join("\n");

    let mut copy = pool
      .copy_in_raw(&format!(
        r#"COPY "{}".searched_tweet_id FROM STDIN WITH (FORMAT CSV)"#,
        self.schema
      ))
      .await
      .unwrap(); // TODO
    copy.send(v.as_bytes()).await.unwrap();
    let rows = copy.finish().await?;

    dbg!(rows);

    Ok(())
  }

  #[tracing::instrument(skip_all)]
  async fn create_schema_and_stuff_for_the_current_search_query(
    &self,
    pool: &PgPool,
  ) -> Result<(), GraphError> {
    /*
    create table bloodymary.tweet as
    select *
    from alimentaire.tweet t
    where t.published_time between 1590969600000 and 1593561600000
    and t."text" ilike '%vegan' ;
    -- ~1min30sec

    create table bloodymary.retweet as
    select r.*
    from alimentaire.retweet r
    join bloodymary.tweet t on t.id = r.retweeted_tweet_id
    -- ~1min30sec


    create table bloodymary.user as
    select *
    from alimentaire.user;
       */
    sqlx::query(&format!(
      r#"
  CREATE TABLE "{schema}".tweet AS
  SELECT *
  FROM cockt.tweet t
  JOIN "{schema}".searched_tweet_id b USING (id)
              "#,
      schema = self.schema
    ))
    .execute(pool)
    .await
    .unwrap(); // TODO

    sqlx::query(&format!(
      r#"
CREATE TABLE "{schema}"."user" AS
SELECT distinct u.id , u.screen_name, u."name" 
FROM cockt.user u
JOIN "{schema}".tweet t ON t.user_id = u.id;
              "#,
      schema = self.schema
    ))
    .execute(pool)
    .await?;

    sqlx::query(&format!(
      r#"
CREATE TABLE "{schema}".retweet AS
SELECT r.*
FROM cockt.retweet r
JOIN "{schema}".searched_tweet_id b on b.id = r.retweeted_tweet_id
              "#,
      schema = self.schema
    ))
    .execute(pool)
    .await?;

    sqlx::query(&format!(
      r#"
CREATE TABLE "{schema}".tweet_user_mention AS
SELECT m.*
FROM cockt.tweet_user_mention m
JOIN "{schema}".searched_tweet_id b on b.id = m.tweet_id
            "#,
      schema = self.schema
    ))
    .execute(pool)
    .await?;

    sqlx::query(&format!(
      r#"
CREATE TABLE "{schema}".quote AS
SELECT q.*
FROM cockt.quote q
JOIN "{schema}".searched_tweet_id b on b.id = q.quoted_tweet_id
          "#,
      schema = self.schema
    ))
    .execute(pool)
    .await?;

    sqlx::query(&format!(
      r#"
CREATE TABLE "{schema}".tweet_hashtag AS
SELECT h.*
FROM cockt.tweet_hashtag h
JOIN "{schema}".searched_tweet_id b on b.id = h.tweet_id
        "#,
      schema = self.schema
    ))
    .execute(pool)
    .await?;

    Ok(())
  }

  #[tracing::instrument(skip_all)]
  async fn create_graph_db(&self, pool: &PgPool) -> Result<(), GraphError> {
    sqlx::query(&format!(
      r#"
CREATE TABLE IF NOT EXISTS "{schema}".graph (
    name TEXT NOT NULL PRIMARY KEY, 
    description TEXT,
    directed BOOLEAN NOT NULL DEFAULT true,
    notebook_url TEXT,
    graph_parent TEXT DEFAULT NULL REFERENCES "{schema}".graph(name) ON DELETE CASCADE,
    modularity FLOAT DEFAULT NULL
);
                "#,
      schema = self.schema
    ))
    .execute(pool)
    .await?;

    sqlx::query(&format!(
      r#"
CREATE TABLE IF NOT EXISTS "{schema}".node (
    id TEXT NOT NULL, 
    graph_name TEXT NOT NULL REFERENCES "{schema}".graph(name) ON DELETE CASCADE, 
    PRIMARY KEY(id, graph_name)
);
                "#,
      schema = self.schema
    ))
    .execute(pool)
    .await?;

    sqlx::query(&format!(
      r#"
CREATE TABLE IF NOT EXISTS "{schema}".node_attribute (
    node_id TEXT NOT NULL, 
    graph_name TEXT NOT NULL REFERENCES "{schema}".graph(name) ON DELETE CASCADE, 
    name TEXT NOT NULL, -- REFERENCES attribute_name(name) ON DELETE CASCADE,
    value FLOAT DEFAULT NULL,
    PRIMARY KEY(node_id, graph_name, name),
    FOREIGN KEY (node_id, graph_name) REFERENCES "{schema}".node(id, graph_name) ON DELETE CASCADE
);
                "#,
      schema = self.schema
    ))
    .execute(pool)
    .await?;

    sqlx::query(&format!(
      r#"
CREATE TABLE IF NOT EXISTS "{schema}".link (
    node_out TEXT NOT NULL,
    node_in TEXT NOT NULL, 
    graph_name TEXT NOT NULL, 
    weight INTEGER,
    PRIMARY KEY(node_in, node_out, graph_name),
    FOREIGN KEY (node_out, graph_name) REFERENCES "{schema}".node(id, graph_name) ON DELETE CASCADE,
    FOREIGN KEY (node_in, graph_name) REFERENCES "{schema}".node(id, graph_name) ON DELETE CASCADE
);
                "#,
      schema = self.schema
    ))
    .execute(pool)
    .await?;

    Ok(())
  }

  #[tracing::instrument(skip_all)]
  async fn get_status(&self, pool: &PgPool) -> Result<Option<String>, GraphError> {
    let res = match sqlx::query_as::<_, TableStatus>(&format!(
      r#"SELECT EXISTS (
        SELECT FROM information_schema.tables
        WHERE  table_schema = '{schema}'
        AND    table_name   = 'status'
        );"#,
      schema = self.schema,
    ))
    .fetch_one(pool)
    .await?
    .exists
    {
      true => match sqlx::query_as::<_, Status>(&format!(
        r#"SELECT datetime, status from "{schema}".status WHERE graph_name = '{graph_name}'
        AND community = '{community}'
        AND centrality = '{centrality}'
        AND max_rank = {max_rank}
        AND show_interaction = {show_interaction} ORDER BY datetime DESC"#,
        schema = self.schema,
        graph_name = self.graph_name,
        community = self.community,
        centrality = self.centrality,
        max_rank = self.max_rank,
        show_interaction = self.show_interaction
      ))
      .fetch_optional(pool)
      .await?
      {
        Some(e) => Some(e.status),
        None => None,
      },
      false => None,
    };

    Ok(res)
  }

  #[tracing::instrument(skip_all)]
  pub async fn delete_status(&self) -> Result<(), GraphError> {
    let pool = self.get_pg_pool().await?;

    if sqlx::query_as::<_, TableStatus>(&format!(
      r#"SELECT EXISTS (
        SELECT FROM information_schema.tables
        WHERE  table_schema = '{schema}'
        AND    table_name   = 'status'
        );"#,
      schema = self.schema,
    ))
    .fetch_one(&pool)
    .await?
    .exists
    {
      sqlx::query(&format!(
        r#"DELETE FROM "{schema}".status WHERE graph_name = '{graph_name}'
        AND community = '{community}'
        AND centrality = '{centrality}'
        AND max_rank = {max_rank}
        AND show_interaction = {show_interaction}"#,
        schema = self.schema,
        graph_name = self.graph_name,
        community = self.community,
        centrality = self.centrality,
        max_rank = self.max_rank,
        show_interaction = self.show_interaction,
      ))
      .execute(&pool)
      .await?;
    }

    if sqlx::query_as::<_, TableStatus>(&format!(
      r#"SELECT EXISTS (
        SELECT FROM information_schema.tables
        WHERE  table_schema = '{schema}'
        AND    table_name   = 'vis'
        );"#,
      schema = self.schema,
    ))
    .fetch_one(&pool)
    .await?
    .exists
    {
      sqlx::query(&format!(
        r#"DELETE FROM "{schema}".vis WHERE graph_name = '{graph_name}'
        AND community = '{community}'
        AND centrality = '{centrality}'
        AND max_rank = {max_rank}
        AND show_interaction = {show_interaction}"#,
        schema = self.schema,
        graph_name = self.graph_name,
        community = self.community,
        centrality = self.centrality,
        max_rank = self.max_rank,
        show_interaction = self.show_interaction,
      ))
      .execute(&pool)
      .await?;
    }

    pool.close().await;

    Ok(())
  }

  #[tracing::instrument(skip_all)]
  pub async fn get_status_info(&self) -> Result<Vec<Status>, GraphError> {
    let pool = self.get_pg_pool().await?;

    let res = match sqlx::query_as::<_, TableStatus>(&format!(
      r#"SELECT EXISTS (
        SELECT FROM information_schema.tables 
        WHERE  table_schema = '{schema}'
        AND    table_name   = 'status'
        );"#,
      schema = self.schema,
    ))
    .fetch_one(&pool)
    .await?
    .exists
    {
      true => {
        sqlx::query_as::<_, Status>(&format!(
          r#"SELECT datetime, status from "{schema}".status WHERE graph_name = '{graph_name}'
        AND community = '{community}'
        AND centrality = '{centrality}'
        AND max_rank = {max_rank}
        AND show_interaction = {show_interaction} ORDER BY datetime DESC"#,
          schema = self.schema,
          graph_name = self.graph_name,
          community = self.community,
          centrality = self.centrality,
          max_rank = self.max_rank,
          show_interaction = self.show_interaction,
        ))
        .fetch_all(&pool)
        .await?
      }
      false => vec![],
    };

    pool.close().await;

    Ok(res)
  }

  #[tracing::instrument(skip_all)]
  pub async fn get_modularity(&self) -> Result<Option<Graph>, GraphError> {
    let pool = self.get_pg_pool().await?;

    let res = match sqlx::query_as::<_, TableStatus>(&format!(
      r#"SELECT EXISTS (
        SELECT FROM information_schema.tables
        WHERE  table_schema = '{schema}'
        AND    table_name   = 'graph'
        );"#,
      schema = self.schema,
    ))
    .fetch_one(&pool)
    .await?
    .exists
    {
      true => {
        sqlx::query_as::<_, Graph>(&format!(
          r#"SELECT modularity from "{schema}".graph WHERE name = '{graph_name}_{community}'"#,
          schema = self.schema,
          graph_name = self.graph_name,
          community = self.community
        ))
        .fetch_optional(&pool)
        .await?
      }
      false => None,
    };

    pool.close().await;

    Ok(res)
  }

  #[tracing::instrument(skip_all)]
  pub async fn delete_schema(&self) -> Result<(), GraphError> {
    let pool = self.get_pg_pool().await?;

    self.delete_schema_c_est_mal(&pool).await?;

    pool.close().await;

    Ok(())
  }

  #[tracing::instrument(skip_all)]
  async fn delete_schema_c_est_mal(&self, pool: &PgPool) -> Result<(), GraphError> {
    sqlx::query(&format!(
      r#"DROP SCHEMA IF EXISTS "{}" CASCADE"#,
      self.schema
    ))
    .execute(pool)
    .await?;

    Ok(())
  }

  #[tracing::instrument(skip_all)]
  async fn create_schema(&self, pool: &PgPool) -> Result<(), GraphError> {
    sqlx::query(&format!(r#"CREATE SCHEMA "{}""#, self.schema))
      .execute(pool)
      .await?;

    Ok(())
  }

  #[tracing::instrument(skip_all)]
  async fn create_tmp_table(&self, pool: &PgPool) -> Result<(), GraphError> {
    // TODO TEMP TABLE
    sqlx::query(&format!(
      r#"CREATE TABLE "{schema}".searched_tweet_id (id TEXT)"#,
      schema = self.schema
    ))
    // sqlx::query("CREATE TEMP TABLE searched_tweet_id (id TEXT)")
    .execute(pool)
    .await?;

    Ok(())
  }

  #[tracing::instrument(skip_all)]
  async fn set_status_started(&self, pool: &PgPool) -> Result<(), GraphError> {
    // TODO TEMP TABLE
    sqlx::query(&format!(
      r#"CREATE TABLE IF NOT EXISTS "{schema}".status (
        datetime timestamp NOT NULL DEFAULT (now() AT TIME ZONE 'CET'),
        status VARCHAR(50) NOT NULL,
        graph_name VARCHAR NOT NULL,
        community VARCHAR NOT NULL,
        centrality VARCHAR NOT NULL,
        max_rank INTEGER NOT NULL,
        show_interaction BOOLEAN NOT NULL
      )"#,
      schema = self.schema
    ))
    // sqlx::query("CREATE TEMP TABLE searched_tweet_id (id TEXT)")
    .execute(pool)
    .await?;

    sqlx::query(&format!(
      r#"INSERT INTO "{schema}".status
      (status, graph_name, community, centrality, max_rank, show_interaction)
      VALUES('started', '{graph_name}', '{community}', '{centrality}', '{max_rank}', '{show_interaction}')
      "#,
      schema = self.schema,
      graph_name = self.graph_name,
      community = self.community,
      centrality = self.centrality,
      max_rank = self.max_rank,
      show_interaction = self.show_interaction,
    ))
    // sqlx::query("CREATE TEMP TABLE searched_tweet_id (id TEXT)")
    .execute(pool)
    .await?;

    Ok(())
  }

  #[tracing::instrument]
  async fn create_data_graph(&self) -> Result<(), GraphError> {
    let log = File::create(format!("./log/{}.log", &self.schema)).expect("failed to open log");
    let log_error =
      File::create(format!("./log/{}-error.log", &self.schema)).expect("failed to open log");

    // R --no-save < create-data-graph.R
    let spawned = Command::new("Rscript")
      .stderr(log_error)
      .stdout(log)
      .args(&[self.r_script.to_str().unwrap_or_default(), &self.schema])
      .spawn()?;

    // en mode docker pour mac / windows qu'on pourrait surement faire par env je ne sais pas comment
    /*
    let spawned = Command::new("docker")
      .stderr(Stdio::piped())
      .stdout(Stdio::piped())
      .args(&["run", "--rm", "--network", "host", "my-r-image", &self.schema])
      .current_dir("/Users/amo/git/cocktail-front/server/scripts-ub")
      .spawn()?;
    */

    let output = spawned.wait_with_output()?;

    if output.status.success() {
      Ok(())
    } else {
      let err = String::from_utf8(output.stderr).unwrap_or_default();
      Err(GraphError::Script(err))
    }
  }

  #[tracing::instrument]
  async fn create_viz(&self) -> Result<JsonDataGraph, GraphError> {
    let log = File::options()
      .append(true)
      .open(format!("./log/{}.log", &self.schema))
      .expect("failed to open log");
    let log_error = File::options()
      .append(true)
      .open(format!("./log/{}-error.log", &self.schema))
      .expect("failed to open log");

    let spawned = Command::new("python3")
      .stderr(log_error)
      .stdout(log)
      .args(&[
        self.python_script.to_str().unwrap_or_default(),
        &self.schema,
        &self.graph_name,
        &self.community,
        &self.centrality,
        &self.max_rank.to_string(),
        &self.show_interaction.to_string(),
      ])
      .spawn()?;

    let output = spawned.wait_with_output()?;

    if output.status.success() {
      tracing::info!(
        "{}",
        String::from_utf8(output.stdout.clone()).unwrap_or_default()
      );
      let j = serde_json::from_slice::<JsonDataGraph>(&output.stdout)
        .map_err(|e| GraphError::Searcher(format!("parse json error: {}", e)))?;
      Ok(j)
    } else {
      tracing::info!(
        "{}",
        String::from_utf8(output.stderr.clone()).unwrap_or_default()
      );
      let err = String::from_utf8(output.stderr).unwrap_or_default();
      Err(GraphError::Script(err))
    }
  }

  #[tracing::instrument(skip_all)]
  async fn get_pg_pool(&self) -> Result<PgPool, GraphError> {
    let pool = PgPoolOptions::new()
      .connect(self.database_url.as_ref())
      .await?;

    Ok(pool)
  }

  #[tracing::instrument(skip_all)]
  pub async fn get_json_data(&self) -> Result<Option<JsonDataGraph>, GraphError> {
    let pool = self.get_pg_pool().await?;

    let res = match sqlx::query_as::<_, TableStatus>(&format!(
      r#"SELECT EXISTS (
        SELECT FROM information_schema.tables 
        WHERE  table_schema = '{schema}'
        AND    table_name   = 'vis'
        );"#,
      schema = self.schema,
    ))
    .fetch_one(&pool)
    .await?
    .exists
    {
      true => {
        sqlx::query_as::<_, JsonDataGraph>(&format!(
          r#"SELECT nodes, edges 
            FROM "{schema}".vis 
            WHERE graph_name = '{graph_name}'
            AND community = '{community}'
            AND centrality = '{centrality}'
            AND max_rank = {max_rank}
            AND show_interaction = {show_interaction}"#,
          schema = self.schema,
          graph_name = self.graph_name,
          community = self.community,
          centrality = self.centrality,
          max_rank = self.max_rank,
          show_interaction = self.show_interaction,
        ))
        .fetch_optional(&pool)
        .await?
      }
      false => None,
    };

    pool.close().await;
    Ok(res)
  }
}
