use axum::{
  extract::{Query, State},
  response::{IntoResponse, Redirect},
  Form,
};
use chrono::{NaiveDate, Utc};
use cocktail_db_twitter::TopKDatabase;
use cocktail_db_web::{HiddenElementTweetsList, ParsedProjectCriteria, WebDatabase};
use fts::{Frequence, FrequenceCooccurence, OrderBy};
use hyper::HeaderMap;
use ory_kratos_client::apis::configuration::Configuration;
use serde::Deserialize;

use crate::{
  deserialize_stringified_list,
  error::WebError,
  get_logout_url,
  models::{
    auth::AuthenticatedUser,
    templates::{HtmlTemplate, Results},
  },
  routes::paths,
};

#[derive(Deserialize)]
pub struct QueryParams {
  pub page: Option<String>,
  pub auteur: Option<String>,
  pub date: Option<String>,
  pub hashtag: Option<String>,
  pub ordre: Option<String>,
}

pub struct FilterAuthor {
  pub user_name: String,
  pub user_screen_name: String,
  pub hidden: bool,
}

#[derive(Debug, Deserialize)]
pub struct ToggleElement {
  #[serde(deserialize_with = "deserialize_stringified_list")]
  pub hashtag: Vec<String>,
  #[serde(deserialize_with = "deserialize_stringified_list")]
  pub author: Vec<String>,
  pub hidden: bool,
  pub query_page: String,
  pub query_auteur: String,
  pub query_date: String,
  pub query_hashtag: String,
  pub query_ordre: String,
}

pub async fn get_results(
  paths::ProjectTweetsTab {
    project_id,
    tab,
    aside_hashtag_tab,
  }: paths::ProjectTweetsTab,
  AuthenticatedUser {
    niveau,
    last_login_datetime,
    user_id,
  }: AuthenticatedUser,
  headers: HeaderMap,
  State(db): State<WebDatabase>,
  State(kratos_configuration): State<Configuration>,
  State(conn): State<TopKDatabase>,
  query_params: Query<QueryParams>,
) -> Result<impl IntoResponse, WebError> {
  let directory_path = format!("project-data/{}", project_id.to_string());
  let project = cocktail_db_web::project(&db, project_id.to_hyphenated(), &user_id).await?;
  let page = query_params
    .page
    .clone()
    .unwrap_or("1".to_string())
    .parse::<u32>()
    .unwrap_or(1);
  let author = query_params.auteur.clone().unwrap_or("".to_string());
  let date = match query_params.date.clone() {
    Some(date_string) => Some(
      NaiveDate::parse_from_str(&date_string, "%Y-%m-%d").unwrap_or(Utc::now().date().naive_utc()),
    ),
    None => None,
  };
  let hashtag = query_params.hashtag.clone();
  let order = query_params
    .ordre
    .clone()
    .unwrap_or("decroissant".to_string());

  let parsed_criteria = ParsedProjectCriteria::from(&project);

  let mut exclude_retweets = false;

  if query_params.auteur.is_some() || query_params.date.is_some() {
    //Affiche ou non le retweet sur on filtre par auteur
    exclude_retweets = false;
  }

  let hidden_element: HiddenElementTweetsList =
    cocktail_db_web::hidden_hashtag_tweet_list(&db, project_id.to_hyphenated(), &user_id).await?;

  let index = fts::Index::open_in_dir(directory_path)?;

  let tweets = fts::search_tweets_for_result(
    &index,
    &match query_params.auteur.clone() {
      Some(author) => vec![author],
      None => vec![],
    },
    &hidden_element.hidden_hashtag_list,
    &hidden_element.hidden_author_list,
    exclude_retweets,
    OrderBy::from(&*tab),
    &order,
    &date,
    &hashtag,
    page,
  )?;
  let cooccurences = cocktail_db_twitter::search_topk_hashtags_cooccurence(conn.clone()).await?;

  let frequences = fts::search_study_hashtags_count_per_day(
    &index,
    &project.start_date,
    &project.end_date,
    &parsed_criteria.hashtag_list,
    &"total".to_string(),
  )?;
  let frequences_topk = fts::search_top_hashtags_count_per_day(
    &index,
    &project.start_date,
    &project.end_date,
    &"total".to_string(),
  )?;
  let frequences_cooccurence = fts::search_top_hashtags_cooccurence_count_per_day(
    &index,
    &project.start_date,
    &project.end_date,
    &tab,
    &cooccurences
      .into_iter()
      .map(|c| fts::HashtagCooccurence {
        hashtag1: c.hashtag1,
        hashtag2: c.hashtag2,
      })
      .collect(),
  )?;

  let mut authors: Vec<fts::AuthorCount> = fts::aggregate_authors(&index, &"total".to_string(), 1)?;

  authors.append(&mut fts::aggregate_authors(
    &index,
    &"total".to_string(),
    2,
  )?);
  authors.append(&mut fts::aggregate_authors(
    &index,
    &"total".to_string(),
    3,
  )?);

  let logout_url = get_logout_url(kratos_configuration, headers).await;

  let (include_count, exclude_count) =
    cocktail_db_web::include_exclude_hashtag_count(&db, project_id.to_hyphenated(), &user_id)
      .await?;

  Ok(HtmlTemplate(Results {
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
    download_path: paths::DownloadProject { project_id },
    duplicate_popup_path: paths::PopupDuplicateProject { project_id },
    logout_url,
    include_count,
    exclude_count,
    niveau,
    last_login_datetime,
    title: project.title,
    tweets,
    frequences: frequences
      .into_iter()
      .map(|frequence| {
        let hidden = hidden_element
          .hidden_hashtag_list
          .iter()
          .any(|h| h == &frequence.hashtag);
        Frequence {
          hashtag: frequence.hashtag,
          hidden,
          data: frequence.data,
        }
      })
      .collect(),
    frequences_topk: frequences_topk
      .into_iter()
      .map(|frequence| {
        let hidden = hidden_element
          .hidden_hashtag_list
          .iter()
          .any(|h| h == &frequence.hashtag);
        Frequence {
          hashtag: frequence.hashtag,
          hidden,
          data: frequence.data,
        }
      })
      .collect(),
    frequences_cooccurence: frequences_cooccurence
      .into_iter()
      .map(|frequence| {
        let hidden = hidden_element
          .hidden_hashtag_list
          .iter()
          .any(|h| h == &frequence.label);
        FrequenceCooccurence {
          label: frequence.label,
          hidden,
          data: frequence.data,
        }
      })
      .collect(),
    authors: authors
      .into_iter()
      .map(|count| FilterAuthor {
        user_name: count.author.user_name,
        user_screen_name: count.author.user_screen_name.to_string(),
        hidden: hidden_element
          .hidden_author_list
          .iter()
          .any(|a| a == &count.author.user_screen_name),
      })
      .collect(),
    tab,
    user_screen_name: author,
    date,
    hashtag,
    page,
    order,
    tweets_count: project.tweets_count,
    authors_count: project.authors_count,
    hidden: match aside_hashtag_tab.clone().as_str() {
      "auteur" => hidden_element.hidden_author_list.len() > 0,
      _ => hidden_element.hidden_hashtag_list.len() > 0,
    },
    aside_hashtag_tab,
  }))
}

pub async fn results(
  paths::ProjectResults { project_id }: paths::ProjectResults,
  AuthenticatedUser {
    niveau,
    last_login_datetime,
    user_id,
  }: AuthenticatedUser,
  headers: HeaderMap,
  State(db): State<WebDatabase>,
  State(kratos_configuration): State<Configuration>,
  State(conn): State<TopKDatabase>,
  query_params: Query<QueryParams>,
) -> Result<impl IntoResponse, WebError> {
  get_results(
    paths::ProjectTweetsTab {
      project_id,
      tab: "total".to_string(),
      aside_hashtag_tab: "project".to_string(),
    },
    AuthenticatedUser {
      niveau,
      last_login_datetime,
      user_id,
    },
    headers,
    State(db),
    State(kratos_configuration),
    State(conn),
    query_params,
  )
  .await
}

pub async fn results_tab(
  paths::ProjectTweetsTab {
    project_id,
    tab,
    aside_hashtag_tab,
  }: paths::ProjectTweetsTab,
  AuthenticatedUser {
    niveau,
    last_login_datetime,
    user_id,
  }: AuthenticatedUser,
  headers: HeaderMap,
  State(db): State<WebDatabase>,
  State(kratos_configuration): State<Configuration>,
  State(conn): State<TopKDatabase>,
  query_params: Query<QueryParams>,
) -> Result<impl IntoResponse, WebError> {
  if niveau != 2 && tab == "engageants" {
    return Ok(
      Redirect::to(paths::ProjectResults { project_id }.to_string().as_str()).into_response(),
    );
  }
  Ok(
    get_results(
      paths::ProjectTweetsTab {
        project_id,
        tab,
        aside_hashtag_tab,
      },
      AuthenticatedUser {
        niveau,
        last_login_datetime,
        user_id,
      },
      headers,
      State(db),
      State(kratos_configuration),
      State(conn),
      query_params,
    )
    .await
    .into_response(),
  )
}

#[tracing::instrument]
pub async fn toggle_hashtag(
  paths::ProjectTweetsTab {
    project_id,
    tab,
    aside_hashtag_tab,
  }: paths::ProjectTweetsTab,
  AuthenticatedUser {
    niveau: _,
    last_login_datetime,
    user_id,
  }: AuthenticatedUser,
  State(db): State<WebDatabase>,
  Form(toggle): Form<ToggleElement>,
) -> Result<impl IntoResponse, WebError> {
  if toggle.hashtag.len() > 0 {
    cocktail_db_web::toggle_hashtag_tweets_list(
      &db,
      project_id.to_hyphenated(),
      &user_id,
      toggle.hashtag,
      toggle.hidden,
    )
    .await?;
  }

  if toggle.author.len() > 0 {
    cocktail_db_web::toggle_author_tweets_list(
      &db,
      project_id.to_hyphenated(),
      &user_id,
      toggle.author,
      toggle.hidden,
    )
    .await?;
  }

  let mut query = "?page=".to_string();
  query.push_str(&toggle.query_page);
  if &toggle.query_date != "" {
    query.push_str("&date=");
    query.push_str(&toggle.query_date);
  }
  if &toggle.query_hashtag != "" {
    query.push_str("&hashtag=");
    query.push_str(&toggle.query_hashtag);
  }
  if &toggle.query_auteur != "" {
    query.push_str("&auteur=");
    query.push_str(&toggle.query_auteur);
  }
  query.push_str("&ordre=");
  query.push_str(&toggle.query_ordre);

  let mut url = paths::ProjectTweetsTab {
    project_id,
    tab,
    aside_hashtag_tab,
  }
  .to_string();

  url.push_str(&query);

  Ok(Redirect::to(&url).into_response())
}
