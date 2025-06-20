use crate::{
  error::WebError,
  get_logout_url,
  models::{
    auth::AuthenticatedUser,
    templates::{self, PopupAccounts, PopupKeywords, Request},
  },
};
use axum::{
  extract::{Query, State},
  response::{IntoResponse, Redirect},
  Form,
};
use cocktail_db_web::WebDatabase;

use hyper::HeaderMap;
use ory_kratos_client::apis::configuration::Configuration;
use regex::Regex;

use cocktail_db_web::Bloc;
use serde::Deserialize;

use crate::{models::templates::HtmlTemplate, routes::paths};

#[derive(Debug, Deserialize)]
pub struct UpdateRequest {
  pub block_delete_id: Option<i32>,
  pub item_delete: Option<String>,
  pub add_block: Option<String>,
  pub switch_selection: Option<i32>,
  pub switch_exclusion: Option<String>,
  pub exact_keywords: Option<String>,
  pub exact_group_keywords: Option<String>,
  pub accounts: Option<String>,
  pub block_id: Option<i32>,
}

#[tracing::instrument]
pub async fn request(
  paths::ProjectRequest { project_id }: paths::ProjectRequest,
  AuthenticatedUser {
    niveau,
    last_login_datetime,
    user_id,
  }: AuthenticatedUser,
  headers: HeaderMap,
  State(db): State<WebDatabase>,
  State(kratos_configuration): State<Configuration>,
) -> Result<impl IntoResponse, WebError> {
  if niveau != 2 {
    return Ok(
      Redirect::to(paths::ProjectHashtags { project_id }.to_string().as_str()).into_response(),
    );
  }

  let logout_url = get_logout_url(kratos_configuration, headers).await;

  let project = cocktail_db_web::project(&db, project_id.to_hyphenated(), &user_id).await?;
  let (include_count, exclude_count) =
    cocktail_db_web::include_exclude_hashtag_count(&db, project_id.to_hyphenated(), &user_id)
      .await?;

  let request_params = project.request_params.to_vec();

  Ok(
    HtmlTemplate(Request {
      daterange_path: paths::ProjectDateRange { project_id },
      hashtag_path: paths::ProjectHashtags { project_id },
      request_path: paths::ProjectRequest { project_id },
      collect_path: paths::ProjectCollect { project_id },
      import_path: paths::ProjectImport { project_id },
      export_path: paths::ProjectCsvExport { project_id },
      request_params,
      popup_hashtags_path: paths::PopupHashtags { project_id },
      popup_keywords_path: paths::PopupKeywords { project_id },
      popup_accounts_path: paths::PopupAccounts { project_id },
      delete_popup_path: paths::PopupDeleteProject { project_id },
      rename_popup_path: paths::PopupRenameProject { project_id },
      duplicate_popup_path: paths::PopupDuplicateProject { project_id },
      analysis_preview_popup_path: paths::PopupAnalysisPreview { project_id },
      analysis_path: paths::ProjectAnalysis { project_id },
      is_analyzed: project.is_analyzed == 1,
      results_path: paths::ProjectResults { project_id },
      tweets_graph_path: paths::ProjectTweetsGraph { project_id },
      authors_path: paths::ProjectAuthors { project_id },
      result_hashtags_path: paths::ProjectResultHashtags { project_id },
      communities_path: paths::Communities { project_id },
      download_path: paths::DownloadProject { project_id },
      logout_url,
      include_count,
      exclude_count,
      niveau,
      last_login_datetime,
      title: project.title,
      tweets_count: project.tweets_count,
      authors_count: project.authors_count,
    })
    .into_response(),
  )
}

#[tracing::instrument]
pub async fn request_update(
  paths::ProjectRequest { project_id }: paths::ProjectRequest,
  AuthenticatedUser {
    niveau,
    last_login_datetime,
    user_id,
  }: AuthenticatedUser,
  headers: HeaderMap,
  State(db): State<WebDatabase>,
  State(kratos_configuration): State<Configuration>,
  Form(update_request): Form<UpdateRequest>,
) -> Result<impl IntoResponse, WebError> {
  let project = cocktail_db_web::project(&db, project_id.to_hyphenated(), &user_id).await?;

  let mut request_params = project.request_params.to_vec();

  match update_request.block_delete_id {
    None => (),
    Some(block_delete_id) => {
      if block_delete_id != -1 {
        // Selection
        let index: usize = block_delete_id as usize;

        if request_params[0].len() == 1 {
          request_params[0] = vec![Bloc {
            data: vec![],
            link: "".to_string(),
          }]
        } else {
          request_params[0].remove(index);
        }
      } else {
        // Exclusion
        request_params[1] = vec![Bloc {
          data: vec![],
          link: "ET".to_string(),
        }]
      }
    }
  }

  match update_request.item_delete {
    None => (),
    Some(block_item) => {
      let regex = Regex::new(r"(?m)([-0-9]+)_([0-9]+)").unwrap();
      let result = regex.captures_iter(&block_item);

      for mat in result {
        let block_index_str = &mat[1];
        let item_index: usize = mat[2].parse().unwrap();

        if block_index_str == "-1" {
          request_params[1][0].data.remove(item_index);
        } else {
          let block_index: usize = block_index_str.parse().unwrap();
          request_params[0][block_index].data.remove(item_index);
        }
      }
    }
  }

  match update_request.add_block {
    None => (),
    Some(_) => request_params[0].push(Bloc {
      data: vec![],
      link: "ET".to_string(),
    }),
  }

  match update_request.switch_selection {
    None => (),
    Some(block_id) => {
      let index: usize = block_id as usize;
      request_params[0][index].link = if request_params[0][index].link == "ET" {
        "OU".to_string()
      } else {
        "ET".to_string()
      }
    }
  }

  match update_request.switch_exclusion {
    None => (),
    Some(_) => {
      request_params[1][0].link = if request_params[1][0].link == "ET" {
        "OU".to_string()
      } else {
        "ET".to_string()
      }
    }
  }

  let block_id = update_request.block_id.unwrap_or_default();

  match update_request.exact_group_keywords {
    None => (),
    Some(included_exact_group_keywords) => {
      if !included_exact_group_keywords.trim().is_empty() {
        if block_id != -1 {
          let index: usize = block_id as usize;
          request_params[0][index]
            .data
            .push(included_exact_group_keywords.trim().to_string());
        } else {
          request_params[1][0]
            .data
            .push(included_exact_group_keywords.trim().to_string());
        }
      }
    }
  }

  match update_request.exact_keywords {
    None => (),
    Some(included_exact_keywords) => {
      for keyword in included_exact_keywords.split_whitespace() {
        if !keyword.trim().is_empty() {
          if block_id != -1 {
            let index: usize = block_id as usize;
            request_params[0][index]
              .data
              .push(keyword.trim().to_string());
          } else {
            request_params[1][0].data.push(keyword.trim().to_string());
          }
        }
      }
    }
  }

  match update_request.accounts {
    None => (),
    Some(accounts) => {
      for account in accounts.split(",") {
        let mut value = account.trim().to_string();
        if !value.is_empty() {
          if !value.starts_with("@") {
            value = format!("@{}", value);
          }
          if block_id != -1 {
            request_params[0][block_id as usize].data.push(value);
          } else {
            request_params[1][0].data.push(value);
          }
        }
      }
    }
  }

  for i in 0..request_params[0].len() {
    request_params[0][i].data.sort();
  }

  request_params[1][0].data.sort();

  cocktail_db_web::update_project_request_params(
    &db,
    project_id.to_hyphenated(),
    &user_id,
    request_params.clone(),
  )
  .await?;

  let response = templates::BlocsUpdate {
    request_params,
    popup_hashtags_path: paths::PopupHashtags { project_id },
    popup_keywords_path: paths::PopupKeywords { project_id },
    popup_accounts_path: paths::PopupAccounts { project_id },
  };
  Ok(HtmlTemplate(response).into_response())
}

#[derive(Debug, Deserialize, Default)]
pub struct AccountsPopupParam {
  pub block_id: i32,
}
#[tracing::instrument]
pub async fn accounts_popup(
  paths::PopupAccounts { project_id }: paths::PopupAccounts,
  params: Option<Query<AccountsPopupParam>>,
  State(conn): State<WebDatabase>,
) -> Result<impl IntoResponse, WebError> {
  let params_unwraped = params.unwrap_or_default();
  let block_id = params_unwraped.block_id;

  Ok(HtmlTemplate(PopupAccounts {
    request_path: paths::ProjectRequest { project_id },
    block_id,
  }))
}

#[derive(Debug, Deserialize, Default)]
pub struct KeywordsPopupParam {
  pub block_id: i32,
}
#[tracing::instrument]
pub async fn keywords_popup(
  paths::PopupKeywords { project_id }: paths::PopupKeywords,
  params: Option<Query<KeywordsPopupParam>>,
  State(conn): State<WebDatabase>,
) -> Result<impl IntoResponse, WebError> {
  let params_unwraped = params.unwrap_or_default();
  let block_id = params_unwraped.block_id;

  Ok(HtmlTemplate(PopupKeywords {
    request_path: paths::ProjectRequest { project_id },
    block_id,
  }))
}
