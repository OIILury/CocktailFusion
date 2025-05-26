use std::{fs, path::PathBuf, str::FromStr};

use crate::{
  error::WebError,
  models::{
    auth::AuthenticatedUser,
    templates::{self, HtmlTemplate},
  },
  AppState,
};
use axum::{
  extract::State,
  response::{IntoResponse, Redirect},
};

use crate::routes::paths::PopupDeleteProject;

#[tracing::instrument]
pub async fn delete_popup(
  PopupDeleteProject { project_id }: PopupDeleteProject,
) -> Result<impl IntoResponse, WebError> {
  Ok(HtmlTemplate(templates::PopupDeleteProject { project_id }))
}

#[tracing::instrument(skip(state))]
pub async fn delete_project(
  PopupDeleteProject { project_id }: PopupDeleteProject,
  AuthenticatedUser {
    niveau: _,
    last_login_datetime,
    user_id,
  }: AuthenticatedUser,
  State(state): State<AppState>,
) -> Result<Redirect, WebError> {
  cocktail_db_web::delete_project(&state.db, project_id.to_hyphenated(), &user_id)
    .await
    .map_err(|e| WebError::WTFError(e.to_string()))?;
  cocktail_db_web::delete_chart(&state.db, project_id.to_hyphenated())
    .await
    .map_err(|e| WebError::WTFError(e.to_string()))?;

  let directory_path =
    PathBuf::from_str(format!("project-data/{}", project_id.to_string()).as_str())?;

  let graph_generator = cocktail_graph_utils::GraphGenerator::new(
    state.database_url,
    project_id.to_string(),
    PathBuf::new(),
    PathBuf::new(),
    "".to_string(),
    "".to_string(),
    "".to_string(),
    200,
    false,
  );

  let _ = fs::remove_dir_all(&directory_path);
  let _ = graph_generator.delete_schema().await;

  Ok(Redirect::to("/projets"))
}
