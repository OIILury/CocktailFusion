use crate::{
  error::WebError,
  get_logout_url,
  models::{
    auth::AuthenticatedUser,
    templates::{HtmlTemplate, NewProjectsTemplate},
  },
  routes::paths::ProjectDateRange,
  AppState,
};
use axum::{
  extract::State,
  response::{IntoResponse, Redirect},
  Form,
};
use hyper::{HeaderMap, StatusCode};
use serde::Deserialize;
use uuid::Uuid;

use crate::routes::paths::CreateProject;

#[derive(Debug, Deserialize)]
pub struct CreateTitle {
  pub nom_etude: String,
}

#[tracing::instrument]
pub async fn create_project(
  _: CreateProject,
  AuthenticatedUser {
    niveau,
    last_login_datetime,
    user_id,
  }: AuthenticatedUser,
  State(state): State<AppState>,
  Form(create_title): Form<CreateTitle>,
) -> Result<Redirect, StatusCode> {
  let project_id = Uuid::new_v4();
  let title = create_title.nom_etude;
  cocktail_db_web::create_project(
    &state.db,
    cocktail_db_web::Project {
      project_id: project_id.to_hyphenated(),
      user_id,
      title,
      ..Default::default()
    },
  )
  .await
  .map_err(|_op| StatusCode::INTERNAL_SERVER_ERROR)?;

  let project_path = ProjectDateRange { project_id }.to_string();
  Ok(Redirect::to(&project_path))
}

pub async fn new_project(
  _: CreateProject,
  AuthenticatedUser {
    niveau: _,
    last_login_datetime,
    user_id,
  }: AuthenticatedUser,
  State(state): State<AppState>,
  headers: HeaderMap,
) -> Result<impl IntoResponse, WebError> {
  let projects: Vec<cocktail_db_web::Project> = cocktail_db_web::projects(&state.db, &user_id)
    .await
    .map_err(|e| tracing::error!("impossible de récupérer la liste des projets : {}", e))
    .unwrap_or_default();

  let logout_url = get_logout_url(state.kratos_configuration, headers).await;

  let template = NewProjectsTemplate {
    projects,
    last_login_datetime,
    logout_url,
  };

  Ok(HtmlTemplate(template))
}
