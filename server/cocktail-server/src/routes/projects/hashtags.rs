use std::ops::Not;

use axum::{
  extract::{Query, State},
  http::HeaderMap,
  response::IntoResponse,
};
use cocktail_db_twitter::{HashtagQuery, TopKDatabase};
use cocktail_db_web::WebDatabase;
use ory_kratos_client::apis::configuration::Configuration;
use serde::Deserialize;

use crate::{
  error::WebError,
  get_logout_url,
  models::{
    auth::AuthenticatedUser,
    templates::{self, HtmlTemplate},
  },
  routes::paths::{self, ProjectHashtags},
};

#[tracing::instrument]
pub async fn hashtags(
  ProjectHashtags { project_id }: ProjectHashtags,
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

  let include_hashtag_list =
    cocktail_db_web::hashtag_list(db.clone(), project_id.to_hyphenated(), &user_id).await?;

  let project = cocktail_db_web::project(&db, project_id.to_hyphenated(), &user_id).await?;
  let exclude_hashtag_list =
    cocktail_db_web::exclude_hashtag_list(db.clone(), project_id.to_hyphenated(), &user_id).await?;
  let (include_count, exclude_count) =
    cocktail_db_web::include_exclude_hashtag_count(db, project_id.to_hyphenated(), &user_id)
      .await?;

  let content = templates::Hashtags {
    include_basket: include_hashtag_list.into_iter().collect(),
    exclude_basket: exclude_hashtag_list.into_iter().collect(),
    include_count,
    exclude_count,
    popup_path: paths::PopupHashtags { project_id },
    delete_popup_path: paths::PopupDeleteProject { project_id },
    rename_popup_path: paths::PopupRenameProject { project_id },
    duplicate_popup_path: paths::PopupDuplicateProject { project_id },
    download_path: paths::DownloadProject { project_id },
    include_basket_path: paths::ProjectBasketInclude { project_id },
    exclude_basket_path: paths::ProjectBasketExclude { project_id },
    daterange_path: paths::ProjectDateRange { project_id },
    hashtag_path: paths::ProjectHashtags { project_id },
    request_path: paths::ProjectRequest { project_id },
    collect_path: paths::ProjectCollect { project_id },
    analysis_preview_popup_path: paths::PopupAnalysisPreview { project_id },
    analysis_path: paths::ProjectAnalysis { project_id },
    is_analyzed: project.is_analyzed == 1,
    results_path: paths::ProjectResults { project_id },
    tweets_graph_path: paths::ProjectTweetsGraph { project_id },
    authors_path: paths::ProjectAuthors { project_id },
    result_hashtags_path: paths::ProjectResultHashtags { project_id },
    communities_path: paths::Communities { project_id },
    logout_url,

    niveau,
    last_login_datetime,
    title: project.title,
    tweets_count: project.tweets_count,
    authors_count: project.authors_count,
    import_path: paths::ProjectImport { project_id },
  };

  Ok(HtmlTemplate(content))
}

#[derive(Debug, Deserialize, Default)]
pub struct ExcludeQueryParam {
  pub exclude: bool,
  pub block_id: Option<i32>,
}

#[tracing::instrument]
pub async fn hashtags_popup(
  paths::PopupHashtags { project_id }: paths::PopupHashtags,
  AuthenticatedUser {
    niveau: _,
    last_login_datetime: _,
    user_id,
  }: AuthenticatedUser,
  params: Option<Query<ExcludeQueryParam>>,
  State(conn): State<WebDatabase>,
) -> Result<impl IntoResponse, WebError> {
  let params_unwraped = params.unwrap_or_default();
  let exclude = params_unwraped.exclude;
  let block_id = params_unwraped.block_id;

  let stats = {
    if block_id.is_none() {
      if exclude {
        cocktail_db_web::exclude_hashtag_count(conn, project_id.to_hyphenated(), &user_id).await?
      } else {
        cocktail_db_web::include_hashtag_count(conn, project_id.to_hyphenated(), &user_id).await?
      }
    } else {
      0
    }
  };

  Ok(HtmlTemplate(templates::PopupHashtags {
    //popup_hashtags_corpus_path: paths::PopupHashtagsCorpus { project_id },
    popup_hashtags_topk_path: paths::PopupHashtagsTopK { project_id },
    popup_hashtags_search_path: paths::PopupHashtagsSearch { project_id },
    request_path: paths::ProjectRequest { project_id },
    hashtag_count: stats,
    exclude_popup_style: exclude,
    block_id,
  }))
}

#[tracing::instrument]
pub async fn hashtags_corpus(
  paths::PopupHashtagsCorpus { project_id }: paths::PopupHashtagsCorpus,
  State(conn): State<TopKDatabase>,
) -> Result<impl IntoResponse, WebError> {
  Ok(HtmlTemplate(templates::PopupHashtagsCorpus))
}

#[tracing::instrument]
pub async fn hashtags_topk(
  paths::PopupHashtagsTopK { project_id }: paths::PopupHashtagsTopK,
  AuthenticatedUser {
    niveau: _,
    last_login_datetime: _,
    user_id,
  }: AuthenticatedUser,
  params: Option<Query<ExcludeQueryParam>>,
  State(conn): State<TopKDatabase>,
  State(db): State<WebDatabase>,
) -> Result<impl IntoResponse, WebError> {
  let params_unwraped = params.unwrap_or_default();
  let exclude = params_unwraped.exclude;
  let block_id = params_unwraped.block_id;

  let mut topk =
    cocktail_db_twitter::search_topk_hashtags(conn.clone(), HashtagQuery::default()).await?;

  match block_id {
    None => {
      // TODO han c'est moche
      let hashtags_in_basket =
        cocktail_db_web::hashtag_list(db.clone(), project_id.to_hyphenated(), &user_id).await?;
      let exclude_hashtag_list =
        cocktail_db_web::exclude_hashtag_list(db, project_id.to_hyphenated(), &user_id).await?;

      topk.iter_mut().for_each(|t| {
        t.available = {
          if exclude {
            exclude_hashtag_list
              .iter()
              .any(|h| &t.hashtag == &h.name)
              .not()
          } else {
            hashtags_in_basket
              .iter()
              .any(|h| &t.hashtag == &h.name)
              .not()
          }
        };
      });
    }
    Some(block_id) => {
      let hashtags = cocktail_db_web::hashtag_list_premium_request(
        db.clone(),
        project_id.to_hyphenated(),
        &user_id,
        block_id,
      )
      .await?;

      topk
        .iter_mut()
        .for_each(|t| t.available = hashtags.iter().any(|h| &t.hashtag == h).not());
    }
  }

  let topk = topk.into_iter().map(Into::into).collect();

  Ok(HtmlTemplate(templates::PopupHashtagsTopK {
    hashtags: topk,
    include_basket_path: paths::ProjectBasketInclude { project_id },
    exclude_basket_path: paths::ProjectBasketExclude { project_id },
    exclude_popup_style: exclude,
    block_id,
  }))
}

#[derive(Debug, Deserialize, Default)]
pub struct QueryAndExcludeQueryParam {
  pub exclude: bool,
  #[serde(rename = "q")]
  pub query: Option<String>,
  pub block_id: Option<i32>,
}
#[tracing::instrument]
pub async fn hashtags_search(
  paths::PopupHashtagsSearch { project_id }: paths::PopupHashtagsSearch,
  AuthenticatedUser {
    niveau: _,
    last_login_datetime: _,
    user_id,
  }: AuthenticatedUser,
  Query(params): Query<QueryAndExcludeQueryParam>,
  State(conn): State<TopKDatabase>,
  State(db): State<WebDatabase>,
) -> Result<impl IntoResponse, WebError> {
  match params.query {
    Some(query) => {
      let block_id = params.block_id;

      let hashtag_query: HashtagQuery = query.to_string().into();
      let mut headers = axum::headers::HeaderMap::new();
      headers.insert(
        axum::http::header::CONTENT_TYPE,
        axum::headers::HeaderValue::from_static("text/vnd.turbo-stream.html; charset=utf-8"),
      );

      let mut hashtags = if query.is_empty() {
        vec![]
      } else {
        cocktail_db_twitter::search_topk_hashtags(conn.clone(), hashtag_query.clone()).await?
      };

      match block_id {
        None => {
          let hashtags_in_basket =
            cocktail_db_web::hashtag_list(db, project_id.to_hyphenated(), &user_id).await?;
          hashtags.iter_mut().for_each(|t| {
            t.available = hashtags_in_basket
              .iter()
              .any(|h| &t.hashtag == &h.name)
              .not();
          });
          hashtags = hashtags.into_iter().map(Into::into).collect();
        }
        Some(block_id) => {
          let hashtags_in_db = cocktail_db_web::hashtag_list_premium_request(
            db.clone(),
            project_id.to_hyphenated(),
            &user_id,
            block_id,
          )
          .await?;

          hashtags
            .iter_mut()
            .for_each(|t| t.available = hashtags_in_db.iter().any(|h| &t.hashtag == h).not());
        }
      }

      let hashtags = hashtags.into_iter().map(Into::into).collect();

      let content = templates::PopupHashtagsSearchResult {
        hashtags,
        q: hashtag_query.to_string(),
        include_basket_path: paths::ProjectBasketInclude { project_id },
        exclude_basket_path: paths::ProjectBasketExclude { project_id },
        exclude_popup_style: params.exclude,
        block_id,
      };

      Ok((headers, HtmlTemplate(content)).into_response())
    }
    None => Ok(
      HtmlTemplate(templates::PopupHashtagsSearch {
        hashtags_search_path: paths::PopupHashtagsSearch { project_id },
        exclude_popup_style: params.exclude,
        block_id: params.block_id,
      })
      .into_response(),
    ),
  }
}
