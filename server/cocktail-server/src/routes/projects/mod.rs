use crate::{error::WebError, get_logout_url, models::auth::AuthenticatedUser, AppState};
use axum::{extract::State, http::HeaderMap, response::IntoResponse};
use hyper::http::HeaderValue;
use serde::{de, de::Error, Deserialize, Deserializer};
use uuid::Uuid;

use crate::{
  models::templates::{HtmlTemplate, ProjectsTemplate},
  routes::paths::Projects,
};

pub mod basket;
pub mod collect;
pub mod daterange;
pub mod delete;
pub mod download;
pub mod duplicate;
pub mod hashtags;
pub mod nouveau;
pub mod rename;
pub mod request;
pub mod update;
pub mod import;
pub mod csv_export;

#[tracing::instrument]
pub async fn projects(
  _: Projects,
  AuthenticatedUser {
    niveau: _,
    last_login_datetime,
    user_id,
  }: AuthenticatedUser,
  State(state): State<AppState>,
  headers: HeaderMap,
) -> Result<impl IntoResponse, WebError> {
  let mut response_headers = HeaderMap::new();

  if user_id == "" {
    response_headers.insert(
      "Set-cookie",
      HeaderValue::from_str(format!("user_id={}", Uuid::new_v4()).as_str()).unwrap(),
    );
  }
  let projects: Vec<cocktail_db_web::Project> = cocktail_db_web::projects(&state.db, &user_id)
    .await
    .map_err(|e| tracing::error!("impossible de récupérer la liste des projets : {}", e))
    .unwrap_or_default();

  let logout_url = get_logout_url(state.kratos_configuration, headers).await;

  let template = ProjectsTemplate {
    projects,
    last_login_datetime,
    logout_url,
  };
  Ok((response_headers, HtmlTemplate(template)))
}

// parce que il y a bug dans la librairie serde_urlencoded
// https://github.com/nox/serde_urlencoded/issues/26
pub fn de_from_str<'de, D, S>(deserializer: D) -> Result<S, D::Error>
where
  D: Deserializer<'de>,
  S: std::str::FromStr,
{
  let s = <&str as Deserialize>::deserialize(deserializer)?;
  S::from_str(s).map_err(|_| D::Error::custom("could not parse string"))
}

// Adapté de routes/auth/mod.rs
// (https://github.com/tokio-rs/axum/blob/1fe45583626a4c9c890cc01131d38c57f8728686/examples/query-params-with-empty-strings/src/main.rs#L37)
fn empty_string_as_none<'de, D, T>(de: D) -> Result<Option<T>, D::Error>
where
  D: Deserializer<'de>,
  T: std::str::FromStr,
  T::Err: std::fmt::Display,
{
  let opt = Option::<String>::deserialize(de)?;
  match opt.as_deref() {
    None | Some("") => Ok(None),
    Some(s) => std::str::FromStr::from_str(s)
      .map_err(de::Error::custom)
      .map(Some),
  }
}
