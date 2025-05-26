use std::str::FromStr;

use axum::{
  extract::{Query, State},
  response::{Html, IntoResponse, Redirect},
  Extension,
};
use fts::{OrderBy, Tweet};
use handlebars::Handlebars;
use hyper::HeaderMap;
use ory_kratos_client::{
  apis::v0alpha2_api::{get_self_service_login_flow, get_self_service_registration_flow},
  models::{SelfServiceLoginFlow, SelfServiceRegistrationFlow},
};
use serde::{de, Deserialize, Deserializer, Serialize};

use crate::routes::paths::{AuthLogin, AuthRegistration};
use crate::{error::WebError, AppState};

#[derive(Debug, Deserialize)]
pub struct Flow {
  #[serde(default, deserialize_with = "empty_string_as_none")]
  flow: Option<String>,
}

// ctrl-c ctrl-v https://github.com/tokio-rs/axum/blob/1fe45583626a4c9c890cc01131d38c57f8728686/examples/query-params-with-empty-strings/src/main.rs#L37
fn empty_string_as_none<'de, D, T>(de: D) -> Result<Option<T>, D::Error>
where
  D: Deserializer<'de>,
  T: FromStr,
  T::Err: std::fmt::Display,
{
  let opt = Option::<String>::deserialize(de)?;
  match opt.as_deref() {
    None | Some("") => Ok(None),
    Some(s) => FromStr::from_str(s).map_err(de::Error::custom).map(Some),
  }
}

#[derive(Debug, Serialize)]
pub struct RegistrationContext {
  pub data: SelfServiceRegistrationFlow,
}

#[tracing::instrument]
pub async fn auth_registration(
  _: AuthRegistration,
  Query(params): Query<Flow>,
  headers: HeaderMap,
  State(state): State<AppState>,
  Extension(handlebars_registry): Extension<Handlebars<'_>>,
) -> Result<impl IntoResponse, WebError> {
  if let Some(flow) = params.flow {
    let res = get_self_service_registration_flow(
      &state.kratos_configuration,
      &flow,
      headers.get("cookie").and_then(|c| c.to_str().ok()),
    )
    .await
    .map_err(|e| WebError::WTFError(e.to_string()))?;

    let content;
    if res.ui.messages.is_none()
      && res
        .ui
        .nodes
        .iter()
        .filter_map(|n| {
          if n.messages.len() > 0 {
            return Some(true);
          }

          None
        })
        .collect::<Vec<bool>>()
        .len()
        == 0
    {
      let data = RegistrationContext { data: res };
      content = handlebars_registry
        .render("registration_popin.html", &data)
        .map_err(|e| WebError::WTFError(e.to_string()))?;
    } else {
      let data = RegistrationContext { data: res };
      content = handlebars_registry
        .render("registration.html", &data)
        .map_err(|e| WebError::WTFError(e.to_string()))?;
    }

    Ok(Html(content).into_response())
  } else {
    let url = format!(
      "{}/self-service/registration/browser?return_to=",
      state.kratos_browser_url
    );
    Ok(Redirect::to(&url).into_response())
  }
}

#[derive(Debug, Serialize)]
pub struct LoginContext {
  pub data: SelfServiceLoginFlow,
  pub tweets: Vec<Tweet>,
}

pub async fn auth_login(
  _: AuthLogin,
  Query(params): Query<Flow>,
  headers: HeaderMap,
  State(state): State<AppState>,
  Extension(handlebars_registry): Extension<Handlebars<'_>>,
) -> Result<impl IntoResponse, WebError> {
  if let Some(flow) = params.flow {
    let res = get_self_service_login_flow(
      &state.kratos_configuration,
      &flow,
      headers.get("cookie").and_then(|c| c.to_str().ok()),
    )
    .await
    .map_err(|e| WebError::WTFError(e.to_string()))?;
    let directory_path = std::env::var("DIRECTORY_PATH")?;

    let tweets = fts::search_tweets(
      &fts::Index::open_in_dir(directory_path)?,
      "text:lubrizol",
      &Some(OrderBy::RetweetCount),
    )?;
    let data = LoginContext { data: res, tweets };
    let content = handlebars_registry
      .render("login.html", &data)
      .map_err(|e| WebError::WTFError(e.to_string()))?;

    Ok(Html(content).into_response())
  } else {
    let url = format!(
      "{}/self-service/login/browser?return_to=",
      state.kratos_browser_url
    );
    Ok(Redirect::to(&url).into_response())
  }
}
