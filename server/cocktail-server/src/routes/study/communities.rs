use axum::{
  extract::State,
  http::HeaderMap,
  response::{IntoResponse, Redirect},
};
use chrono::Local;
use cocktail_graph_utils::Status;
use ory_kratos_client::apis::configuration::Configuration;
use tokio::task;

use crate::{
  error::WebError,
  get_logout_url,
  models::{auth::AuthenticatedUser, templates},
  routes::paths,
};

#[tracing::instrument(skip(state))]
pub async fn communities(
  paths::Communities { project_id }: paths::Communities,
  AuthenticatedUser {
    niveau,
    last_login_datetime,
    user_id,
  }: AuthenticatedUser,
  headers: HeaderMap,
  State(state): State<crate::AppState>,
  State(kratos_configuration): State<Configuration>,
) -> Result<impl IntoResponse, WebError> {
  if niveau != 2 {
    return Ok(
      Redirect::to(paths::ProjectResults { project_id }.to_string().as_str()).into_response(),
    );
  }

  let logout_url = get_logout_url(kratos_configuration, headers).await;

  let project = cocktail_db_web::project(&state.db, project_id.to_hyphenated(), &user_id).await?;
  let (include_count, exclude_count) =
    cocktail_db_web::include_exclude_hashtag_count(&state.db, project_id.to_hyphenated(), &user_id)
      .await?;

  let graph_generator = cocktail_graph_utils::GraphGenerator::new(
    state.database_url,
    project_id.to_string(),
    state.r_script,
    state.python_script,
    "user_user_retweet".to_string(),
    "louvain_community".to_string(),
    "page_rank_centrality".to_string(),
    200,
    false,
  );

  let mut json_data = None;
  let mut status = graph_generator.get_status_info().await?;
  let graph = graph_generator.get_modularity().await?;

  if status.len() == 0 {
    let _ = task::spawn(async move {
      let _ = graph_generator.process_single_graph().await;
    });

    status = vec![Status {
      datetime: Local::now().naive_local(),
      status: "started".to_string(),
    }];
  } else if status.len() != 1 {
    json_data = graph_generator.get_json_data().await?;
  }

  Ok(
    templates::HtmlTemplate(templates::Communities {
      json_data,
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
      download_path: paths::DownloadProject { project_id },
      logout_url,
      include_count,
      exclude_count,
      niveau,
      last_login_datetime,
      status,
      title: project.title,
      tab: "user_user_retweet".to_string(),
      community: "louvain_community".to_string(),
      centrality: "page_rank_centrality".to_string(),
      max_rank: 200,
      show_interaction: false,
      tweets_count: project.tweets_count,
      authors_count: project.authors_count,
      modularity: graph
        .unwrap_or(cocktail_graph_utils::Graph { modularity: -1.0 })
        .modularity,
    })
    .into_response(),
  )
}

#[tracing::instrument(skip(state))]
pub async fn communities_tab(
  paths::CommunitiesTab {
    project_id,
    tab,
    community,
    centrality,
    max_rank,
    show_interaction,
  }: paths::CommunitiesTab,
  AuthenticatedUser {
    niveau,
    last_login_datetime,
    user_id,
  }: AuthenticatedUser,
  headers: HeaderMap,
  State(state): State<crate::AppState>,
  State(kratos_configuration): State<Configuration>,
) -> Result<impl IntoResponse, WebError> {
  if niveau != 2 {
    return Ok(
      Redirect::to(paths::ProjectResults { project_id }.to_string().as_str()).into_response(),
    );
  }

  let logout_url = get_logout_url(kratos_configuration, headers).await;

  let project = cocktail_db_web::project(&state.db, project_id.to_hyphenated(), &user_id).await?;
  let (include_count, exclude_count) =
    cocktail_db_web::include_exclude_hashtag_count(&state.db, project_id.to_hyphenated(), &user_id)
      .await?;

  let graph_generator = cocktail_graph_utils::GraphGenerator::new(
    state.database_url,
    project_id.to_string(),
    state.r_script,
    state.python_script,
    tab.clone(),
    community.clone(),
    centrality.clone(),
    max_rank,
    show_interaction,
  );

  let mut json_data = None;
  let mut status = graph_generator.get_status_info().await?;
  let graph = graph_generator.get_modularity().await?;

  if status.len() == 0 {
    let _ = task::spawn(async move {
      let _ = graph_generator.process_single_graph().await;
    });

    status = vec![Status {
      datetime: Local::now().naive_local(),
      status: "started".to_string(),
    }];
  } else if status.len() != 1 {
    json_data = graph_generator.get_json_data().await?;
  }

  Ok(
    templates::HtmlTemplate(templates::Communities {
      json_data,
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
      download_path: paths::DownloadProject { project_id },
      logout_url,
      include_count,
      exclude_count,
      niveau,
      last_login_datetime,
      title: project.title,
      tab,
      community,
      centrality,
      max_rank,
      show_interaction,
      status,
      tweets_count: project.tweets_count,
      authors_count: project.authors_count,
      modularity: graph
        .unwrap_or(cocktail_graph_utils::Graph { modularity: -1.0 })
        .modularity,
    })
    .into_response(),
  )
}

#[tracing::instrument(skip(state))]
pub async fn reload_communities(
  paths::CommunitiesTab {
    project_id,
    tab,
    community,
    centrality,
    max_rank,
    show_interaction,
  }: paths::CommunitiesTab,
  State(state): State<crate::AppState>,
) -> Result<impl IntoResponse, WebError> {
  let graph_generator = cocktail_graph_utils::GraphGenerator::new(
    state.database_url,
    project_id.to_string(),
    state.r_script,
    state.python_script,
    tab.clone(),
    community.clone(),
    centrality.clone(),
    max_rank,
    show_interaction,
  );

  let _ = graph_generator.delete_status().await?;

  let _ = task::spawn(async move {
    let _ = graph_generator.process_single_graph().await;
  });

  Ok(
    Redirect::to(
      &paths::CommunitiesTab {
        project_id,
        tab,
        community,
        centrality,
        max_rank,
        show_interaction,
      }
      .to_string(),
    )
    .into_response(),
  )
}
