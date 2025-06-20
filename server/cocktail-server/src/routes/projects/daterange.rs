use crate::{
  error::WebError,
  get_logout_url,
  models::{auth::AuthenticatedUser, templates::DateRange},
};
use axum::{extract::State, http::HeaderMap, response::IntoResponse, Form};
use chrono::{Duration, NaiveDate, Utc};
use chronoutil::shift_months;
use cocktail_db_web::WebDatabase;
use ory_kratos_client::apis::configuration::Configuration;
use serde::Deserialize;

use crate::{models::templates::HtmlTemplate, routes::paths};

#[derive(Debug, Deserialize)]
pub struct UpdateDaterange {
  pub start_date: NaiveDate,
  pub end_date: NaiveDate,
  pub duree: String,
}

#[tracing::instrument]
pub async fn daterange(
  paths::ProjectDateRange { project_id }: paths::ProjectDateRange,
  AuthenticatedUser {
    niveau,
    last_login_datetime,
    user_id,
  }: AuthenticatedUser,
  headers: HeaderMap,
  State(db): State<WebDatabase>,
  State(kratos_configuration): State<Configuration>,
) -> Result<impl IntoResponse, WebError> {
  let logout_url = get_logout_url(kratos_configuration, headers).await;

  let project = cocktail_db_web::project(&db, project_id.to_hyphenated(), &user_id).await?;
  let (include_count, exclude_count) =
    cocktail_db_web::include_exclude_hashtag_count(&db, project_id.to_hyphenated(), &user_id)
      .await?;

  let start_date_diff = shift_months(Utc::now().date().naive_utc(), -6) - project.start_date;
  let end_date_diff = Utc::now().date().naive_utc() - project.end_date;

  Ok(HtmlTemplate(DateRange {
    daterange_path: paths::ProjectDateRange { project_id },
    hashtag_path: paths::ProjectHashtags { project_id },
    request_path: paths::ProjectRequest { project_id },
    collect_path: paths::ProjectCollect { project_id },
    delete_popup_path: paths::PopupDeleteProject { project_id },
    rename_popup_path: paths::PopupRenameProject { project_id },
    duplicate_popup_path: paths::PopupDuplicateProject { project_id },
    download_path: paths::DownloadProject { project_id },
    analysis_preview_popup_path: paths::PopupAnalysisPreview { project_id },
    analysis_path: paths::ProjectAnalysis { project_id },
    is_analyzed: project.is_analyzed == 1,
    results_path: paths::ProjectResults { project_id },
    tweets_graph_path: paths::ProjectTweetsGraph { project_id },
    authors_path: paths::ProjectAuthors { project_id },
    result_hashtags_path: paths::ProjectResultHashtags { project_id },
    communities_path: paths::Communities { project_id },
    logout_url,
    include_count,
    exclude_count,
    niveau,
    last_login_datetime,
    title: project.title,
    start_date: project.start_date,
    end_date: project.end_date,
    show_custom_range: start_date_diff != Duration::days(0) || end_date_diff != Duration::days(0),
    tweets_count: project.tweets_count,
    authors_count: project.authors_count,
    import_path: paths::ProjectImport { project_id },
    export_path: paths::ProjectCsvExport { project_id },
  }))
}

#[tracing::instrument]
pub async fn daterange_update(
  paths::ProjectDateRange { project_id }: paths::ProjectDateRange,
  AuthenticatedUser {
    niveau: _,
    last_login_datetime: _,
    user_id,
  }: AuthenticatedUser,
  State(db): State<WebDatabase>,
  Form(update_daterange): Form<UpdateDaterange>,
) -> Result<impl IntoResponse, WebError> {
  if update_daterange.duree == "auto" {
    let start_date = shift_months(Utc::now().date().naive_utc(), -6);
    let end_date = Utc::now().date().naive_utc();

    cocktail_db_web::update_project_daterange(
      &db,
      project_id.to_hyphenated(),
      &user_id,
      &start_date.to_string(),
      &end_date.to_string(),
      0,
    )
    .await?;
  } else {
    cocktail_db_web::update_project_daterange(
      &db,
      project_id.to_hyphenated(),
      &user_id,
      &update_daterange.start_date.to_string(),
      &update_daterange.end_date.to_string(),
      1,
    )
    .await?;
  }

  Ok(())
}
