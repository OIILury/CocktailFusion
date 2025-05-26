use std::{fmt, fs, net::SocketAddr, path::PathBuf, str::FromStr};
use crate::routes::paths::StartCollection;
use crate::routes::collect::start_collection;
use crate::routes::collect::delete_collection;
use crate::routes::collect::update_collection;
use axum::{
  async_trait,
  extract::{FromRef, FromRequestParts},
  handler::Handler,
  headers::{HeaderMap, HeaderValue},
  http::{header::CONTENT_TYPE, Request, StatusCode, Uri},
  middleware::{self, Next},
  response::{IntoResponse, Response},
  routing::get,
  BoxError, Extension, Router,
};
use axum_extra::routing::RouterExt;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use cocktail_db_twitter::TopKDatabase;
use cocktail_db_web::WebDatabase;
use handlebars::Handlebars;
use models::{
  auth::AuthenticatedUser,
  templates::{StaticFile, Templates},
};
use ory_kratos_client::apis::{
  configuration::Configuration, v0alpha2_api::create_self_service_logout_flow_url_for_browsers,
};
use routes::auth::{auth_login, auth_registration};
use serde::de::{self, IntoDeserializer};
use serde_json::json;
use sqlx::{
  sqlite::{SqliteConnectOptions, SqlitePoolOptions},
  ConnectOptions,
};
use tower_http::trace::TraceLayer;
use tracing::log::LevelFilter;

use crate::{
  error::WebError,
  routes::{
    charts::{hashtags as chart_hashtags, tweets},
    home, index,
    projects::{
      basket::*, collect::*, daterange::*, delete::*, download::*, duplicate::*, hashtags::*, nouveau::*,
      projects, rename::*, request::*, update::*, import::*,
    },
    study::{analysis::*, authors::*, communities::*, results},
    csv_import,
    
  },
};
use futures::future;

pub mod error;
mod helpers;
mod models;
mod routes;

// const PATH_TEMPLATES: &str = concat!(std::env!("CARGO_MANIFEST_DIR"), "/templates/auth");
// const PATH_PARTIALS: &str = concat!(std::env!("CARGO_MANIFEST_DIR"), "/templates/auth/partials");

#[derive(Debug, Clone)]
pub struct AppState {
  pub db: WebDatabase,
  pub topk_db: TopKDatabase,
  pub index: fts::Index,
  pub kratos_configuration: Configuration,
  pub kratos_browser_url: String,
  pub directory_path: PathBuf,
  pub database_url: String,
  pub r_script: PathBuf,
  pub python_script: PathBuf,
}

impl FromRef<AppState> for Configuration {
  fn from_ref(input: &AppState) -> Self {
    input.kratos_configuration.clone()
  }
}

impl FromRef<AppState> for WebDatabase {
  fn from_ref(input: &AppState) -> Self {
    input.db.clone()
  }
}

impl FromRef<AppState> for TopKDatabase {
  fn from_ref(input: &AppState) -> Self {
    input.topk_db.clone()
  }
}

pub struct Databases {
  pub web_database_path: PathBuf,
  pub topk_database_path: PathBuf,
  pub pg_uri: String,
}

pub struct Kratos {
  pub kratos_base_path: String,
  pub kratos_browser_url: String,
}

pub struct Scripts {
  pub r_script: PathBuf,
  pub python_script: PathBuf,
}

pub async fn run(
  tantivy_path: PathBuf,
  listen_to: SocketAddr,
  databases: Databases,
  Kratos {
    kratos_base_path,
    kratos_browser_url,
  }: Kratos,
  Scripts {
    r_script,
    python_script,
  }: Scripts,
) -> Result<(), WebError> {
  cocktail_db_web::create_database(&databases.web_database_path).await;

  let mut handlebars_registry = Handlebars::new();

  handlebars_registry
    .register_embed_templates::<Templates>()
    .expect("impossible d'enregistrer les templates handlebars");

  // handlebars_registry.set_dev_mode(true);
  // handlebars_registry
  //     .register_templates_directory(".hbs", PATH_TEMPLATES)
  //     .expect("impossible d'enregistrer les templates");
  // handlebars_registry
  //     .register_templates_directory(".hbs", PATH_PARTIALS)
  //     .expect("impossible d'enregistrer les partials");
  handlebars_registry.register_helper("toUiNodePartial", Box::new(helpers::to_ui_node_partial));
  handlebars_registry.register_helper("onlyNodes", Box::new(helpers::OnlyNodes));
  handlebars_registry.register_helper("getNodeLabel", Box::new(helpers::get_node_label));

  let mut kratos_configuration = Configuration::new();
  kratos_configuration.base_path = kratos_base_path;

  let mut opts = SqliteConnectOptions::from_str(&format!(
    "sqlite:{}",
    databases
      .web_database_path
      .to_str()
      .expect("le chemin de la base de données n'est pas bon 🤔")
  ))
  .expect("ah bah marde");
  opts.log_statements(LevelFilter::Trace);

  let pool = SqlitePoolOptions::new()
    .connect_with(opts)
    .await
    .expect("erreur : impossible de se connecter à la base de données.");

  let mut opts_topk_db = SqliteConnectOptions::from_str(&format!(
    "sqlite:{}",
    databases
      .topk_database_path
      .to_str()
      .expect("le chemin de la base de données `topk` n'est pas bon 🤔")
  ))
  .expect("ah bah marde");
  opts_topk_db.log_statements(LevelFilter::Trace);

  let pool_topk = SqlitePoolOptions::new()
    .connect_with(opts_topk_db)
    .await
    .expect("erreur : impossible de se connecter à la base de données `topk`.");
  cocktail_db_web::migrate(pool.clone())
    .await
    .expect("erreur : impossible de migrer la base de données.");

  let turbo_stream = middleware::from_fn(turbo_stream);

  let search_index = fts::retrieve_index(tantivy_path.clone())?;
  let state = AppState {
    db: WebDatabase::new(pool),
    topk_db: TopKDatabase::new(pool_topk),
    index: search_index,
    kratos_configuration,
    database_url: databases.pg_uri,
    r_script,
    python_script,
    directory_path: tantivy_path,
    kratos_browser_url,
  };

  let routes: Router<AppState> = Router::with_state(state.clone())
    .typed_get(index)
    .typed_get(home)
    .typed_get(auth_registration)
    .typed_get(auth_login)
    .typed_get(projects)
    .typed_get(new_project)
    .typed_post(create_project)
    .typed_post(delete_project)
    .typed_post(rename_project.layer(turbo_stream))
    .typed_post(duplicate_project)
    .typed_get(download_project)
    .typed_get(daterange)
    .typed_get(collect)
    .typed_get(hashtags)
    .typed_get(hashtags_popup.layer(turbo_stream))
    .typed_get(delete_popup.layer(turbo_stream))
    .typed_get(rename_popup.layer(turbo_stream))
    .typed_get(duplicate_popup.layer(turbo_stream))
    .typed_get(hashtags_topk)
    .typed_get(hashtags_corpus)
    .typed_get(hashtags_search)
    .typed_get(request)
    .typed_post(request_update.layer(turbo_stream))
    .typed_get(keywords_popup.layer(turbo_stream))
    .typed_get(accounts_popup.layer(turbo_stream))
    .typed_post(add_to_include_basket.layer(turbo_stream))
    .typed_post(add_to_exclude_basket.layer(turbo_stream))
    .typed_get(title)
    .typed_post(title_update)
    .typed_post(chart_hashtags::toggle_hashtag.layer(turbo_stream))
    .typed_post(daterange_update)
    .typed_get(chart_hashtags::aside_hashtag_tab.layer(turbo_stream))
    .typed_post(chart_hashtags::toggle_all.layer(turbo_stream))
    .typed_post(chart_hashtags::toggle_hashtag_cooccurence.layer(turbo_stream))
    .typed_post(analyse)
    .typed_get(preview_analysis.layer(turbo_stream))
    .typed_get(results::results)
    .typed_get(results::results_tab)
    .typed_post(results::toggle_hashtag)
    .typed_get(tweets::tweets)
    .typed_get(tweets::tweets_tab)
    .typed_post(tweets::toggle_hashtag)
    .typed_get(tweets::authors_select)
    .typed_get(authors)
    .typed_get(authors_tab)
    .typed_get(chart_hashtags::hashtags)
    .typed_get(chart_hashtags::hashtags_tab)
    .typed_get(communities)
    .typed_get(communities_tab)
    .typed_post(reload_communities)
    .typed_post(start_collection)
    .typed_post(delete_collection)
    .typed_post(update_collection)
    .route("/static/*file", get(static_handler))
    .route("/projets/:project_id/import", get(import))
    .merge(csv_import::routes())
    .fallback(fallback)
    .layer(Extension(handlebars_registry))
    .layer(TraceLayer::new_for_http());

  // let app = routes.nest(
  //     "/static",
  //     // get_service(ServeDir::new("templates/static"))
  //     get_service(ServeDir::new(assets_path))
  //         .layer(CompressionLayer::new().br(true).gzip(false).deflate(false))
  //         .handle_error(|_e| async move { StatusCode::INTERNAL_SERVER_ERROR }),
  // );

  axum::Server::bind(&listen_to)
    .serve(routes.into_make_service())
    .await?;

  Ok(())
}

async fn fallback(uri: Uri) -> (StatusCode, String) {
  (
    StatusCode::NOT_FOUND,
    format!("cette page n'existe pas : {uri}"),
  )
}

async fn static_handler(uri: Uri) -> impl IntoResponse {
  let mut path = uri.path().trim_start_matches('/').to_string();
  path = path.replace("static/", "");

  StaticFile(path)
}

#[async_trait]
impl<S> FromRequestParts<S> for AuthenticatedUser
where
  Configuration: FromRef<S>,
  S: Send + Sync,
{
  type Rejection = Response;

  async fn from_request_parts(
    parts: &mut axum::http::request::Parts,
    state: &S,
  ) -> Result<Self, Self::Rejection> {
    let headers = HeaderMap::from_request_parts(parts, state)
      .await
      .map_err(|e| e.into_response())?;
    let kratos_configuration = Configuration::from_ref(state);

    let mut last_login_datetime = NaiveDateTime::new(
      NaiveDate::from_ymd(1970, 1, 1),
      NaiveTime::from_hms(0, 0, 0),
    );

    let session_result = ory_kratos_client::apis::v0alpha2_api::to_session(
      &kratos_configuration,
      None,
      headers.get("cookie").and_then(|c| c.to_str().ok()),
    )
    .await;

    if session_result.is_err() {
      let cookies = headers.get("cookie").and_then(|c| c.to_str().ok());

      let mut user_id = "".to_string();

      if cookies.is_some() && cookies.unwrap().contains("user_id=") {
        let cookie = cookies.unwrap()[cookies.unwrap().find("user_id=").unwrap() + 8..].to_string();
        user_id = cookie[..cookie.find(";").unwrap_or(cookie.len())].to_string();
      }
      Ok(AuthenticatedUser {
        niveau: 0,
        last_login_datetime,
        user_id,
      })
    } else {
      let session = session_result.unwrap();

      let traits = session.identity.traits.unwrap_or_default();

      let niveau = traits
        .get("niveau")
        .unwrap_or(&json!(0))
        .as_i64()
        .unwrap_or_default();
      let user_id = traits
        .get("email")
        .unwrap_or(&json!(""))
        .as_str()
        .unwrap_or_default()
        .to_string();

      if session.authenticated_at.is_some() {
        last_login_datetime =
          NaiveDateTime::parse_from_str(&session.authenticated_at.unwrap(), "%Y-%m-%dT%H:%M:%S%Z")
            .unwrap_or(NaiveDateTime::new(
              NaiveDate::from_ymd(1970, 1, 1),
              NaiveTime::from_hms(0, 0, 0),
            ));
      }

      Ok(AuthenticatedUser {
        niveau,
        last_login_datetime,
        user_id,
      })
    }
  }
}

async fn turbo_stream<B>(req: Request<B>, next: Next<B>) -> impl IntoResponse {
  let mut headers = HeaderMap::new();
  headers.insert(
    CONTENT_TYPE,
    HeaderValue::from_static("text/vnd.turbo-stream.html; charset=utf-8"),
  );
  (headers, next.run(req).await)
}

#[tracing::instrument]
async fn _handle_error(
  method: axum::http::Method,
  uri: axum::http::Uri,
  err: BoxError,
) -> impl IntoResponse {
  StatusCode::INTERNAL_SERVER_ERROR
}

pub fn deserialize_stringified_list<'de, D, I>(
  deserializer: D,
) -> std::result::Result<Vec<I>, D::Error>
where
  D: de::Deserializer<'de>,
  I: de::DeserializeOwned,
{
  struct StringVecVisitor<I>(std::marker::PhantomData<I>);

  impl<'de, I> de::Visitor<'de> for StringVecVisitor<I>
  where
    I: de::DeserializeOwned,
  {
    type Value = Vec<I>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
      formatter.write_str("a string containing a list")
    }

    fn visit_str<E>(self, v: &str) -> std::result::Result<Self::Value, E>
    where
      E: de::Error,
    {
      let mut ids: Vec<I> = Vec::new();
      for id in v.split(",") {
        if id.is_empty() {
          continue;
        }
        let id = I::deserialize(id.into_deserializer())?;

        ids.push(id);
      }
      Ok(ids)
    }
  }

  deserializer.deserialize_any(StringVecVisitor(std::marker::PhantomData::<I>))
}

pub async fn get_logout_url(kratos_configuration: Configuration, headers: HeaderMap) -> String {
  let logout_flow_url = create_self_service_logout_flow_url_for_browsers(
    &kratos_configuration,
    headers.get("cookie").and_then(|c| c.to_str().ok()),
  )
  .await;

  if logout_flow_url.is_err() {
    "/auth/login".to_string()
  } else {
    logout_flow_url.unwrap().logout_url
  }
}

pub async fn clear_anynomous_studies(databases: Databases) -> Result<(), WebError> {
  let mut opts = SqliteConnectOptions::from_str(&format!(
    "sqlite:{}",
    databases
      .web_database_path
      .to_str()
      .expect("le chemin de la base de données n'est pas bon 🤔")
  ))
  .expect("ah bah marde");
  opts.log_statements(LevelFilter::Trace);

  let pool = SqlitePoolOptions::new()
    .connect_with(opts)
    .await
    .expect("erreur : impossible de se connecter à la base de données.");
  let db = WebDatabase::new(pool);

  let projects = cocktail_db_web::get_anonymous_projects_to_clear(&db).await?;

  println!(
    "{}: {} projet(s) à supprimer",
    chrono::offset::Local::now()
      .format("%Y-%m-%d %H:%M:%S")
      .to_string(),
    projects.len(),
  );

  future::join_all(projects.iter().map(|project| async {
    let project_id = project.project_id;
    println!(
      "{}: début suppression projet {}",
      chrono::offset::Local::now()
        .format("%Y-%m-%d %H:%M:%S")
        .to_string(),
      project_id.to_string(),
    );
    let _ = cocktail_db_web::delete_chart(&db, project_id).await;

    let directory_path =
      PathBuf::from_str(format!("project-data/{}", project_id.to_string()).as_str()).unwrap();

    let graph_generator = cocktail_graph_utils::GraphGenerator::new(
      databases.pg_uri.clone(),
      project_id.to_string(),
      PathBuf::new(),
      PathBuf::new(),
      "".to_string(),
      "".to_string(),
      "".to_string(),
      200,
      false,
    );

    let _ = graph_generator.delete_schema().await;
    let _ = fs::remove_dir_all(&directory_path);

    let _ = cocktail_db_web::delete_anonymous_project(&db, project_id).await;

    println!(
      "{}: fin suppression projet {}",
      chrono::offset::Local::now()
        .format("%Y-%m-%d %H:%M:%S")
        .to_string(),
      project_id.to_string(),
    );
  }))
  .await;

  Ok(())
}
