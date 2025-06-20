use axum::{
  extract::{Query, State},
  response::{IntoResponse, Redirect},
};
use cocktail_db_web::WebDatabase;
use hyper::HeaderMap;
use ory_kratos_client::apis::configuration::Configuration;
use serde::Deserialize;

use crate::{
  error::WebError,
  get_logout_url,
  models::{
    auth::AuthenticatedUser,
    templates::{Authors, HtmlTemplate},
  },
  routes::paths,
};

#[derive(Deserialize)]
pub struct QueryParams {
  pub page: Option<String>,
}

pub async fn authors(
  paths::ProjectAuthors { project_id }: paths::ProjectAuthors,
  AuthenticatedUser {
    niveau,
    last_login_datetime,
    user_id,
  }: AuthenticatedUser,
  headers: HeaderMap,
  State(db): State<WebDatabase>,
  State(kratos_configuration): State<Configuration>,
  query_params: Query<QueryParams>,
) -> Result<impl IntoResponse, WebError> {
  if niveau != 2 {
    return Ok(
      Redirect::to(paths::ProjectResults { project_id }.to_string().as_str()).into_response(),
    );
  }

  let directory_path = format!("project-data/{}", project_id.to_string());
  let project = cocktail_db_web::project(&db, project_id.to_hyphenated(), &user_id).await?;
  let page = query_params
    .page
    .clone()
    .unwrap_or("1".to_string())
    .parse::<u32>()
    .unwrap_or(1);

  let author_counts = fts::aggregate_authors(
    &fts::Index::open_in_dir(directory_path)?,
    &"total".to_string(),
    page,
  )?;

  let logout_url = get_logout_url(kratos_configuration, headers).await;

  let (include_count, exclude_count) =
    cocktail_db_web::include_exclude_hashtag_count(&db, project_id.to_hyphenated(), &user_id)
      .await?;

  Ok(
    HtmlTemplate(Authors {
      daterange_path: paths::ProjectDateRange { project_id },
      hashtag_path: paths::ProjectHashtags { project_id },
      request_path: paths::ProjectRequest { project_id },
      collect_path: paths::ProjectCollect { project_id },
      import_path: paths::ProjectImport { project_id },
      analysis_preview_popup_path: paths::PopupAnalysisPreview { project_id },
      analysis_path: paths::ProjectAnalysis { project_id },
      results_path: paths::ProjectResults { project_id },
      tweets_graph_path: paths::ProjectTweetsGraph { project_id },
      authors_path: paths::ProjectAuthors { project_id },
      result_hashtags_path: paths::ProjectResultHashtags { project_id },
      communities_path: paths::Communities { project_id },
      delete_popup_path: paths::PopupDeleteProject { project_id },
      rename_popup_path: paths::PopupRenameProject { project_id },
      duplicate_popup_path: paths::PopupDuplicateProject { project_id },
      clear_data_path: paths::ClearDataLatest { project_id },
      logout_url,
      include_count,
      exclude_count,
      niveau,
      last_login_datetime,
      title: project.title,
      author_counts,
      tab: "total".to_string(),
      page,
      tweets_count: project.tweets_count,
      authors_count: project.authors_count,
      export_path: paths::ProjectCsvExport { project_id },
    })
    .into_response(),
  )
}

pub async fn authors_tab(
  paths::ProjectAuthorsTab { project_id, tab }: paths::ProjectAuthorsTab,
  AuthenticatedUser {
    niveau,
    last_login_datetime,
    user_id,
  }: AuthenticatedUser,
  headers: HeaderMap,
  State(db): State<WebDatabase>,
  State(kratos_configuration): State<Configuration>,
  query_params: Query<QueryParams>,
) -> Result<impl IntoResponse, WebError> {
  if niveau != 2 {
    return Ok(
      Redirect::to(paths::ProjectResults { project_id }.to_string().as_str()).into_response(),
    );
  }

  let directory_path = format!("project-data/{}", project_id.to_string());
  let project = cocktail_db_web::project(&db, project_id.to_hyphenated(), &user_id).await?;
  let page = query_params
    .page
    .clone()
    .unwrap_or("1".to_string())
    .parse::<u32>()
    .unwrap_or(1);

  let author_counts =
    fts::aggregate_authors(&fts::Index::open_in_dir(directory_path)?, &tab, page)?;

  let logout_url = get_logout_url(kratos_configuration, headers).await;

  let (include_count, exclude_count) =
    cocktail_db_web::include_exclude_hashtag_count(&db, project_id.to_hyphenated(), &user_id)
      .await?;

  Ok(
    HtmlTemplate(Authors {
      daterange_path: paths::ProjectDateRange { project_id },
      hashtag_path: paths::ProjectHashtags { project_id },
      request_path: paths::ProjectRequest { project_id },
      collect_path: paths::ProjectCollect { project_id },
      import_path: paths::ProjectImport { project_id },
      analysis_preview_popup_path: paths::PopupAnalysisPreview { project_id },
      analysis_path: paths::ProjectAnalysis { project_id },
      results_path: paths::ProjectResults { project_id },
      tweets_graph_path: paths::ProjectTweetsGraph { project_id },
      authors_path: paths::ProjectAuthors { project_id },
      result_hashtags_path: paths::ProjectResultHashtags { project_id },
      communities_path: paths::Communities { project_id },
      delete_popup_path: paths::PopupDeleteProject { project_id },
      rename_popup_path: paths::PopupRenameProject { project_id },
      duplicate_popup_path: paths::PopupDuplicateProject { project_id },
      clear_data_path: paths::ClearDataLatest { project_id },
      logout_url,
      include_count,
      exclude_count,
      niveau,
      last_login_datetime,
      title: project.title,
      author_counts,
      tab,
      page,
      tweets_count: project.tweets_count,
      authors_count: project.authors_count,
      export_path: paths::ProjectCsvExport { project_id },
    })
    .into_response(),
  )
}
