use std::{fs, path::PathBuf, str::FromStr};

use axum::{
  extract::State,
  response::{IntoResponse, Redirect},
};
use cocktail_db_twitter::TopKDatabase;
use cocktail_db_web::{ParsedProjectCriteria, TweetsChart};
use fts::{copy_index_data, create_index_config};
use futures::future;
use tokio::task;

use crate::{
  error::WebError,
  models::{
    auth::AuthenticatedUser,
    templates::{HtmlTemplate, PopupAnalysisPreview},
  },
  routes::paths,
  AppState,
};

pub async fn analyse(
  paths::ProjectAnalysis { project_id }: paths::ProjectAnalysis,
  AuthenticatedUser {
    niveau: _,
    last_login_datetime: _,
    user_id,
  }: AuthenticatedUser,
  State(state): State<AppState>,
  State(conn): State<TopKDatabase>,
) -> Result<impl IntoResponse, WebError> {
  let project = cocktail_db_web::project(&state.db, project_id.to_hyphenated(), &user_id).await?;

  let parsed_criteria = ParsedProjectCriteria::from(&project);

  let tweets = fts::search_tweets_for_analysis(
    &fts::Index::open_in_dir(state.directory_path.clone())?,
    &project.start_date,
    &project.end_date,
    &parsed_criteria.hashtag_list,
    &parsed_criteria.exclude_hashtag_list,
    &project.request_params,
  )?;

  let directory_path =
    PathBuf::from_str(format!("project-data/{}", project_id.to_string()).as_str())?;
  let _ = fs::remove_dir_all(&directory_path);
  let tweets_count: i64 = *&tweets.len() as i64;
  let mut authors_list: Vec<String> = tweets.iter().map(|t| t.user_id.to_string()).collect();
  authors_list.sort_unstable();
  authors_list.dedup();

  let authors_count = authors_list.len() as i64;

  create_index_config(&directory_path)?;

  copy_index_data(&directory_path, tweets)?;

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

  let _ = graph_generator.delete_schema().await;

  cocktail_db_web::validate_project_analysis(
    &state.db,
    project_id.to_hyphenated(),
    &user_id,
    tweets_count,
    authors_count,
  )
  .await?;

  let _ = task::spawn(async move {
    let _ = graph_generator.process_search().await;
  });

  let _ = task::spawn(async move {
    let tabs = vec!["total", "retweets", "citations", "repondus"];
    let directory_path = format!("project-data/{}", project_id.to_string());
    let index = fts::Index::open_in_dir(directory_path).unwrap();

    future::join_all(tabs.into_iter().map(|tab| async {
      let tweets_counts = fts::search_tweets_count_per_day(
        &index,
        &vec![],
        &vec![],
        &project.start_date,
        &project.end_date,
        &tab.to_string(),
      );
      if tweets_counts.is_ok() {
        let _ = cocktail_db_web::save_chart(
          &state.db,
          project_id.to_string(),
          "tweets".to_string(),
          tab.to_string(),
          TweetsChart {
            data: tweets_counts.unwrap(),
          },
        )
        .await;
      }

      let frequences = fts::search_study_hashtags_count_per_day(
        &index,
        &project.start_date,
        &project.end_date,
        &parsed_criteria.hashtag_list,
        &tab.to_string(),
      );
      let frequences_topk = fts::search_top_hashtags_count_per_day(
        &index,
        &project.start_date,
        &project.end_date,
        &tab.to_string(),
      );
      let cooccurences = cocktail_db_twitter::search_topk_hashtags_cooccurence(conn.clone())
        .await
        .unwrap();

      let frequences_cooccurence = fts::search_top_hashtags_cooccurence_count_per_day(
        &index,
        &project.start_date,
        &project.end_date,
        &tab.to_string(),
        &cooccurences
          .iter()
          .map(|c| fts::HashtagCooccurence {
            hashtag1: c.hashtag1.clone(),
            hashtag2: c.hashtag2.clone(),
          })
          .collect(),
      );

      if frequences.is_ok() && frequences_topk.is_ok() && frequences_cooccurence.is_ok() {
        let _ = cocktail_db_web::save_chart(
          &state.db,
          project_id.to_string(),
          "hashtags".to_string(),
          tab.to_string(),
          (
            frequences.unwrap(),
            frequences_topk.unwrap(),
            frequences_cooccurence.unwrap(),
          ),
        )
        .await;
      }
    }))
    .await;
  });

  Ok(Redirect::to(
    &paths::ProjectResults { project_id }.to_string(),
  ))
}

pub async fn preview_analysis(
  paths::PopupAnalysisPreview { project_id }: paths::PopupAnalysisPreview,
  AuthenticatedUser {
    niveau: _,
    last_login_datetime: _,
    user_id,
  }: AuthenticatedUser,
  State(state): State<AppState>,
) -> Result<impl IntoResponse, WebError> {
  let project = cocktail_db_web::project(&state.db, project_id.to_hyphenated(), &user_id).await?;

  let parsed_criteria = ParsedProjectCriteria::from(&project);

  let tweets_preview = fts::search_tweets_for_preview(
    &fts::Index::open_in_dir(state.directory_path.clone())?,
    &project.start_date,
    &project.end_date,
    &parsed_criteria.hashtag_list,
    &parsed_criteria.exclude_hashtag_list,
    &project.request_params,
  )?;

  Ok(HtmlTemplate(PopupAnalysisPreview {
    count: tweets_preview.count,
    tweets: tweets_preview.tweets,
  }))
}
