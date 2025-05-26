use crate::{
  error::WebError,
  models::{
    auth::AuthenticatedUser,
    templates::{self, HtmlTemplate, TitleChanged},
  },
  AppState,
};
use axum::{extract::State, response::IntoResponse, Form};
use hyper::StatusCode;
use serde::Deserialize;

use crate::routes::paths::PopupRenameProject;

#[tracing::instrument]
pub async fn rename_popup(
  PopupRenameProject { project_id }: PopupRenameProject,
) -> Result<impl IntoResponse, WebError> {
  Ok(HtmlTemplate(templates::PopupRenameProject { project_id }))
}

#[derive(Debug, Deserialize)]
pub struct UpdateTitle {
  title: String,
}

pub async fn rename_project(
  PopupRenameProject { project_id }: PopupRenameProject,
  AuthenticatedUser {
    niveau: _,
    last_login_datetime: _,
    user_id,
  }: AuthenticatedUser,
  State(state): State<AppState>,
  Form(update_title): Form<UpdateTitle>,
) -> Result<impl IntoResponse, StatusCode> {
  cocktail_db_web::rename_project(
    &state.db,
    project_id.to_hyphenated(),
    &user_id,
    &update_title.title,
  )
  .await
  .map_err(|_op| StatusCode::INTERNAL_SERVER_ERROR)?;
  Ok(HtmlTemplate(TitleChanged {
    title: update_title.title,
  }))
}
