use crate::{
  error::WebError,
  models::{
    auth::AuthenticatedUser,
    templates::{self, HtmlTemplate},
  },
  routes::paths::{PopupDuplicateProject, ProjectDateRange},
  AppState,
};
use axum::{
  extract::State,
  response::{IntoResponse, Redirect},
  Form,
};
use hyper::StatusCode;
use serde::Deserialize;
use uuid::Uuid;

#[tracing::instrument]
pub async fn duplicate_popup(
  PopupDuplicateProject { project_id }: PopupDuplicateProject,
  AuthenticatedUser {
    niveau: _,
    last_login_datetime: _,
    user_id,
  }: AuthenticatedUser,
  State(state): State<AppState>,
) -> Result<impl IntoResponse, WebError> {
  let project = cocktail_db_web::project(&state.db, project_id.to_hyphenated(), &user_id).await?;
  Ok(HtmlTemplate(templates::PopupDuplicateProject {
    project_id,
    project_title: project.title,
  }))
}

#[derive(Debug, Deserialize)]
pub struct NewTitle {
  title: String,
}

pub async fn duplicate_project(
  PopupDuplicateProject { project_id }: PopupDuplicateProject,
  AuthenticatedUser {
    niveau: _,
    last_login_datetime: _,
    user_id,
  }: AuthenticatedUser,
  State(state): State<AppState>,
  Form(new_title): Form<NewTitle>,
) -> Result<Redirect, StatusCode> {
  let new_project_id = Uuid::new_v4();
  cocktail_db_web::duplicate_project(
    &state.db,
    project_id.to_hyphenated(),
    new_project_id.to_hyphenated(),
    &user_id,
    &new_title.title,
  )
  .await
  .map_err(|_op| StatusCode::INTERNAL_SERVER_ERROR)?;
  let project_path = ProjectDateRange {
    project_id: new_project_id,
  }
  .to_string();
  Ok(Redirect::to(&project_path))
}
