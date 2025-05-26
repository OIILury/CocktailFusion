use crate::{error::WebError, models::auth::AuthenticatedUser, AppState};
use askama::Template;
use axum::{
  extract::{Form, State},
  response::IntoResponse,
};
use serde::Deserialize;

use crate::{models::templates::HtmlTemplate, routes::paths::ProjectTitleUpdate};

#[derive(Template)]
#[template(
  ext = "html",
  source = r#"
    <turbo-frame id="update-title">
      <form
        action="{{update_title_path}}"
        method="POST"
        x-data
        x-init="$refs.title.select()">
        <input
          type="text"
          name="title"
          value="{{header_title}}"
          required
          autofocus
          x-ref="title"
          @keyup.escape="$refs.cancel.click();$refs.submit.click()" />
        <input type="submit" x-ref="submit" />
        <button type="reset" x-ref="cancel">annuler</button>
      </form>
    </turbo-frame>
    "#
)]
struct TitleUpdate {
  header_title: String,
  update_title_path: ProjectTitleUpdate,
}

pub async fn title(
  ProjectTitleUpdate { project_id }: ProjectTitleUpdate,
  AuthenticatedUser {
    niveau: _,
    last_login_datetime: _,
    user_id,
  }: AuthenticatedUser,
  State(state): State<AppState>,
) -> Result<impl IntoResponse, WebError> {
  let project = cocktail_db_web::project(&state.db, project_id.to_hyphenated(), &user_id).await?;

  let response = HtmlTemplate(TitleUpdate {
    header_title: project.title,
    update_title_path: ProjectTitleUpdate { project_id },
  });

  Ok(response)
}

#[derive(Template)]
#[template(
  ext = "html",
  source = r#"
    <turbo-frame id="update-title">
        {% include "_includes/_title_header.html" %}
    </turbo-frame>
    "#
)]
struct TitleUpdated {
  header_title: String,
  update_title_path: ProjectTitleUpdate,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTitle {
  title: String,
}
pub async fn title_update(
  ProjectTitleUpdate { project_id }: ProjectTitleUpdate,
  AuthenticatedUser {
    niveau: _,
    last_login_datetime: _,
    user_id,
  }: AuthenticatedUser,
  State(state): State<AppState>,
  Form(update_title): Form<UpdateTitle>,
) -> Result<impl IntoResponse, WebError> {
  cocktail_db_web::update_project_title(
    &state.db,
    project_id.to_hyphenated(),
    &user_id,
    update_title.title.as_str(),
  )
  .await?;

  let response = HtmlTemplate(TitleUpdated {
    header_title: update_title.title,
    update_title_path: ProjectTitleUpdate { project_id },
  });

  Ok(response)
}
