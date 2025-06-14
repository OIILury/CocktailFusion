// use crate::{error::WebError, State};
use crate::{error::WebError, models::auth::AuthenticatedUser, routes::paths::Index, AppState};
use axum::{
  extract::State,
  response::{Html, IntoResponse, Redirect},
  Extension,
};
use fts::{OrderBy, Tweet};
use handlebars::Handlebars;
use hyper::HeaderMap;
use serde::Serialize;

use self::paths::Home;

pub mod auth;
pub mod charts;
pub mod paths;
pub mod projects;
pub mod study;
pub mod csv_import;
pub mod collect;
pub mod automation;

#[derive(Debug, Serialize)]
pub struct Context {
  pub tweets: Vec<Tweet>,
  pub niveau: i64,
  pub isLogin: bool,
  pub pageTitle: String,
  pub data: Option<serde_json::Value>,
}

#[tracing::instrument]
pub async fn index(
  _: Index,
  headers: HeaderMap,
  State(state): State<AppState>,
) -> impl IntoResponse {
  Redirect::to("/auth/login")
}

#[tracing::instrument]
pub async fn home(
  _: Home,
  headers: HeaderMap,
  AuthenticatedUser {
    niveau,
    last_login_datetime: _,
    user_id,
  }: AuthenticatedUser,
  State(state): State<AppState>,
  Extension(handlebars_registry): Extension<Handlebars<'_>>,
) -> Result<impl IntoResponse, WebError> {
  if niveau == 0 {
    Ok(Redirect::to("/auth/login").into_response())
  } else {
    let directory_path = std::env::var("DIRECTORY_PATH")?;

    let tweets = fts::search_tweets(
      &fts::Index::open_in_dir(directory_path)?,
      "text:lubrizol",
      &Some(OrderBy::RetweetCount),
    )?;
    let data = Context { 
      tweets, 
      niveau,
      isLogin: true,
      pageTitle: "Accueil".to_string(),
      data: None,
    };
    let content = handlebars_registry
      .render("auth.html", &data)
      .map_err(|e| WebError::WTFError(e.to_string()))?;

    Ok(Html(content).into_response())
  }
}
