use axum::{
  extract::{Query, State},
  response::{IntoResponse, Redirect},
  Form,
};
use cocktail_db_twitter::TopKDatabase;
use cocktail_db_web::{TweetsChart, WebDatabase};
use fts::Author;
use hyper::HeaderMap;
use ory_kratos_client::apis::configuration::Configuration;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
  deserialize_stringified_list,
  error::WebError,
  get_logout_url,
  models::{
    auth::AuthenticatedUser,
    templates::{AuthorsSelect, HtmlTemplate, Tweets},
  },
  routes::paths,
};

use super::hashtags::get_hashtags_chart;

#[derive(Deserialize)]
pub struct QueryParams {
  pub auteur: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ToggleElement {
  #[serde(deserialize_with = "deserialize_stringified_list")]
  pub hashtag: Vec<String>,
  pub hidden: bool,
  pub query_auteur: String,
}

async fn get_chart(
  db: &WebDatabase,
  project_id: &Uuid,
  user_id: &String,
  tab: &String,
  hidden_hashtags: &Vec<String>,
  author: Option<String>,
) -> Result<TweetsChart, WebError> {
  let directory_path = format!("project-data/{}", project_id.to_string());
  let project = cocktail_db_web::project(&db, project_id.to_hyphenated(), &user_id).await?;
  if author.is_none() && hidden_hashtags.len() == 0 {
    let chart =
      cocktail_db_web::get_chart(&db, &project_id.to_string(), &"tweets".to_string(), &tab).await;
    let tweets_chart: TweetsChart;

    if chart.is_ok() {
      tweets_chart = serde_json::from_str(chart.unwrap().as_str())?;
      let mut count = 0;
      tweets_chart
        .data
        .iter()
        .for_each(|e| count = count + e.frequence);

      if count == project.tweets_count as u64 {
        return Ok(tweets_chart);
      }
    }
  }
  let index = fts::Index::open_in_dir(directory_path)?;

  let tweets_counts = fts::search_tweets_count_per_day(
    &index,
    &match author.clone() {
      Some(val) => vec![val],
      None => vec![],
    },
    hidden_hashtags,
    &project.start_date,
    &project.end_date,
    &tab,
  )?;

  if author.is_none() && hidden_hashtags.len() == 0 {
    let _ = cocktail_db_web::save_chart(
      &db,
      project_id.to_string(),
      "tweets".to_string(),
      tab.to_string(),
      TweetsChart {
        data: tweets_counts.clone(),
      },
    )
    .await;
  }

  Ok(TweetsChart {
    data: tweets_counts,
  })
}

pub async fn get_results(
  paths::ProjectTweetsGraphTab {
    project_id,
    tab,
    aside_tweet_tab,
  }: paths::ProjectTweetsGraphTab,
  AuthenticatedUser {
    niveau,
    last_login_datetime,
    user_id,
  }: AuthenticatedUser,
  headers: HeaderMap,
  State(db): State<WebDatabase>,
  State(conn): State<TopKDatabase>,
  State(kratos_configuration): State<Configuration>,
  query_params: Query<QueryParams>,
) -> Result<impl IntoResponse, WebError> {
  let project = cocktail_db_web::project(&db, project_id.to_hyphenated(), &user_id).await?;
  let author = query_params.auteur.clone().unwrap_or("".to_string());
  let hidden_hashtags =
    cocktail_db_web::hidden_hashtag_tweet_graph_list(&db, project_id.to_hyphenated(), &user_id)
      .await?;

  let tweets_chart = get_chart(
    &db,
    &project_id,
    &user_id,
    &tab.to_string(),
    &hidden_hashtags,
    query_params.auteur.clone(),
  )
  .await?;

  let logout_url = get_logout_url(kratos_configuration, headers).await;

  let (include_count, exclude_count) =
    cocktail_db_web::include_exclude_hashtag_count(&db, project_id.to_hyphenated(), &user_id)
      .await?;

  let cooccurences = cocktail_db_twitter::search_topk_hashtags_cooccurence(conn.clone()).await?;

  let (frequences, frequences_topk, frequences_cooccurence) = get_hashtags_chart(
    &db,
    &project_id,
    &user_id,
    &tab.to_string(),
    &hidden_hashtags,
    &cooccurences,
  )
  .await?;

  Ok(HtmlTemplate(Tweets {
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
    delete_popup_path: paths::PopupDeleteProject { project_id },
    rename_popup_path: paths::PopupRenameProject { project_id },
    duplicate_popup_path: paths::PopupDuplicateProject { project_id },
    download_path: paths::DownloadProject { project_id },
    result_hashtags_path: paths::ProjectResultHashtags { project_id },
    communities_path: paths::Communities { project_id },
    authors_select_path: paths::ProjectAuthorsSelect {
      project_id,
      tab: tab.clone(),
    },
    logout_url,
    include_count,
    exclude_count,
    niveau,
    last_login_datetime,
    title: project.title,
    tweets_chart,
    tab,
    aside_tweet_tab,
    frequences,
    frequences_topk,
    frequences_cooccurence,
    selected_author: author,
    tweets_count: project.tweets_count,
    authors_count: project.authors_count,
    hidden: !&hidden_hashtags.is_empty(),
  }))
}

pub async fn tweets(
  paths::ProjectTweetsGraph { project_id }: paths::ProjectTweetsGraph,
  AuthenticatedUser {
    niveau,
    last_login_datetime,
    user_id,
  }: AuthenticatedUser,
  headers: HeaderMap,
  State(db): State<WebDatabase>,
  State(conn): State<TopKDatabase>,
  State(kratos_configuration): State<Configuration>,
  query_params: Query<QueryParams>,
) -> Result<impl IntoResponse, WebError> {
  get_results(
    paths::ProjectTweetsGraphTab {
      project_id,
      tab: "total".to_string(),
      aside_tweet_tab: "project".to_string(),
    },
    AuthenticatedUser {
      niveau,
      last_login_datetime,
      user_id,
    },
    headers,
    State(db),
    State(conn),
    State(kratos_configuration),
    query_params,
  )
  .await
}

pub async fn tweets_tab(
  paths::ProjectTweetsGraphTab {
    project_id,
    tab,
    aside_tweet_tab,
  }: paths::ProjectTweetsGraphTab,
  AuthenticatedUser {
    niveau,
    last_login_datetime,
    user_id,
  }: AuthenticatedUser,
  headers: HeaderMap,
  State(db): State<WebDatabase>,
  State(conn): State<TopKDatabase>,

  State(kratos_configuration): State<Configuration>,
  query_params: Query<QueryParams>,
) -> Result<impl IntoResponse, WebError> {
  if niveau != 2 && tab != "total" {
    return Ok(
      Redirect::to(
        paths::ProjectTweetsGraph { project_id }
          .to_string()
          .as_str(),
      )
      .into_response(),
    );
  }
  Ok(
    get_results(
      paths::ProjectTweetsGraphTab {
        project_id,
        tab,
        aside_tweet_tab,
      },
      AuthenticatedUser {
        niveau,
        last_login_datetime,
        user_id,
      },
      headers,
      State(db),
      State(conn),
      State(kratos_configuration),
      query_params,
    )
    .await
    .into_response(),
  )
}
#[tracing::instrument]
pub async fn toggle_hashtag(
  paths::ProjectTweetsGraphTab {
    project_id,
    tab,
    aside_tweet_tab,
  }: paths::ProjectTweetsGraphTab,
  AuthenticatedUser {
    niveau: _,
    last_login_datetime: _,
    user_id,
  }: AuthenticatedUser,
  State(db): State<WebDatabase>,
  Form(toggle): Form<ToggleElement>,
) -> Result<impl IntoResponse, WebError> {
  cocktail_db_web::toggle_hashtag_tweets_graph_list(
    &db,
    project_id.to_hyphenated(),
    &user_id,
    toggle.hashtag,
    toggle.hidden,
  )
  .await?;

  let mut query = "".to_string();
  if &toggle.query_auteur != "" {
    query.push_str("?auteur=");
    query.push_str(&toggle.query_auteur);
  }

  let mut url = paths::ProjectTweetsGraphTab {
    project_id,
    tab,
    aside_tweet_tab,
  }
  .to_string();

  url.push_str(&query);

  Ok(Redirect::to(&url).into_response())
}

pub async fn authors_select(
  paths::ProjectAuthorsSelect { project_id, tab }: paths::ProjectAuthorsSelect,
  query_params: Query<QueryParams>,
) -> Result<impl IntoResponse, WebError> {
  let directory_path = format!("project-data/{}", project_id.to_string());
  let index = fts::Index::open_in_dir(directory_path)?;

  let authors_infos = fts::aggregate_authors(&index, &"total".to_string(), 0)?;
  let authors: Vec<Author> = authors_infos.into_iter().map(|e| e.author).collect();
  let selected_author = query_params.auteur.clone().unwrap_or("".to_string());

  Ok(HtmlTemplate(AuthorsSelect {
    tweets_graph_path: paths::ProjectTweetsGraph { project_id },
    tab,
    authors,
    selected_author,
  }))
}
