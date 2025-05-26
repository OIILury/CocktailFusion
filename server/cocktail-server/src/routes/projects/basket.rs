use axum::{
  extract::{Form, State},
  response::IntoResponse,
};
use serde::Deserialize;

use crate::{
  error::WebError,
  models::{
    auth::AuthenticatedUser,
    templates::{self, HtmlTemplate, ItemKind},
  },
  routes::{paths, projects::de_from_str, projects::empty_string_as_none},
  AppState,
};

pub async fn add_to_include_basket(
  paths::ProjectBasketInclude { project_id }: paths::ProjectBasketInclude,
  AuthenticatedUser {
    niveau: _,
    last_login_datetime: _,
    user_id,
  }: AuthenticatedUser,
  State(state): State<AppState>,
  Form(what): Form<IncludeItem>,
) -> Result<impl IntoResponse, WebError> {
  match what {
    IncludeItem::Add {
      item_id,
      available,
      count,
      block_id,
    } => {
      let mut include_count = 0;
      let mut exclude_count = 0;

      let hashtag: templates::Item = templates::Item {
        item_id: item_id.clone(),
        name: item_id.clone(),
        count: count.into(),
        available: false,
        kind: ItemKind::Hashtag,
      };

      // Si block_id est défini, on est dans le requeteur premium
      match block_id {
        None => {
          if let Err(e) = cocktail_db_web::add_hashtag(
            &state.db,
            project_id.to_hyphenated(),
            &user_id,
            &item_id,
            count.into(),
            available,
            false,
          )
          .await
          {
            tracing::error!(
              "Impossible d'ajouter le hashtag `{}` au projet {}: {}",
              &item_id,
              &project_id,
              e
            )
          };
          (include_count, exclude_count) = cocktail_db_web::include_exclude_hashtag_count(
            &state.db,
            project_id.to_hyphenated(),
            &user_id,
          )
          .await
          .unwrap_or_default();
        }
        Some(block_id) => {
          let project =
            cocktail_db_web::project(&state.db, project_id.to_hyphenated(), &user_id).await?;
          let mut request_params = project.request_params.to_vec();

          let index: usize = block_id as usize;
          request_params[0][index]
            .data
            .push("#".to_string() + &hashtag.name);

          cocktail_db_web::update_project_request_params(
            &state.db,
            project_id.to_hyphenated(),
            &user_id,
            request_params,
          )
          .await?;
        }
      }
      let response = templates::IncludeHashtagAdded {
        hashtag,
        include_basket_path: paths::ProjectBasketInclude { project_id },
        include_count,
        exclude_count,
        block_id,
      };

      Ok(HtmlTemplate(response).into_response())
    }
    IncludeItem::Remove {
      item_id,
      available,
      count,
      block_id,
    } => {
      let mut include_count = 0;
      let mut exclude_count = 0;

      let hashtag: templates::Item = templates::Item {
        item_id: item_id.clone(),
        name: item_id.clone(),
        count: count.into(),
        available: true,
        kind: ItemKind::Hashtag,
      };

      match block_id {
        None => {
          if let Err(e) = cocktail_db_web::add_hashtag(
            &state.db,
            project_id.to_hyphenated(),
            &user_id,
            &item_id,
            count.into(),
            available,
            false,
          )
          .await
          {
            tracing::error!(
              "Impossible d'ajouter le hashtag `{}` au projet {}: {}",
              &item_id,
              &project_id,
              e
            )
          };

          (include_count, exclude_count) = cocktail_db_web::include_exclude_hashtag_count(
            &state.db,
            project_id.to_hyphenated(),
            &user_id,
          )
          .await
          .unwrap_or_default();
        }
        Some(block_id) => {
          let project =
            cocktail_db_web::project(&state.db, project_id.to_hyphenated(), &user_id).await?;
          let mut request_params = project.request_params.to_vec();

          let index: usize = block_id as usize;
          request_params[0][index]
            .data
            .retain(|x| x != &("#".to_string() + &hashtag.name));

          cocktail_db_web::update_project_request_params(
            &state.db,
            project_id.to_hyphenated(),
            &user_id,
            request_params,
          )
          .await?;
        }
      }

      let response = templates::IncludeHashtagRemoved {
        hashtag,
        include_basket_path: paths::ProjectBasketInclude { project_id },
        include_count,
        exclude_count,
        block_id,
      };

      Ok(HtmlTemplate(response).into_response())
    }
  }
}

pub async fn add_to_exclude_basket(
  paths::ProjectBasketExclude { project_id }: paths::ProjectBasketExclude,
  AuthenticatedUser {
    niveau: _,
    last_login_datetime: _,
    user_id,
  }: AuthenticatedUser,
  State(state): State<AppState>,
  Form(what): Form<ExcludeItem>,
) -> Result<impl IntoResponse, WebError> {
  match what {
    ExcludeItem::Add {
      item_id,
      available,
      count,
      block_id,
    } => {
      let mut include_count = 0;
      let mut exclude_count = 0;
      let hashtag: templates::Item = templates::Item {
        item_id: item_id.clone(),
        name: item_id.clone(),
        count: count.into(),
        available: false,
        kind: ItemKind::Hashtag,
      };

      // Si block_id est défini, on est dans le requeteur premium
      match block_id {
        None => {
          if let Err(e) = cocktail_db_web::add_hashtag(
            &state.db,
            project_id.to_hyphenated(),
            &user_id,
            &item_id,
            count.into(),
            available,
            true,
          )
          .await
          {
            tracing::error!(
              "Impossible d'ajouter le hashtag `{}` au projet {}: {}",
              &item_id,
              &project_id,
              e
            )
          };

          (include_count, exclude_count) = cocktail_db_web::include_exclude_hashtag_count(
            &state.db,
            project_id.to_hyphenated(),
            &user_id,
          )
          .await
          .unwrap_or_default();
        }
        Some(_block_id) => {
          let project =
            cocktail_db_web::project(&state.db, project_id.to_hyphenated(), &user_id).await?;
          let mut request_params = project.request_params.to_vec();

          request_params[1][0]
            .data
            .push("#".to_string() + &hashtag.name);

          cocktail_db_web::update_project_request_params(
            &state.db,
            project_id.to_hyphenated(),
            &user_id,
            request_params,
          )
          .await?;
        }
      }

      let response = templates::ExcludeHashtagAdded {
        hashtag,
        exclude_basket_path: paths::ProjectBasketExclude { project_id },
        include_count,
        exclude_count,
        block_id,
      };

      Ok(HtmlTemplate(response).into_response())
    }
    ExcludeItem::Remove {
      item_id,
      available,
      count,
      block_id,
    } => {
      let mut include_count = 0;
      let mut exclude_count = 0;

      let hashtag: templates::Item = templates::Item {
        item_id: item_id.clone(),
        name: item_id.clone(),
        count: count.into(),
        available: true,
        kind: ItemKind::Hashtag,
      };

      match block_id {
        None => {
          if let Err(e) = cocktail_db_web::add_hashtag(
            &state.db,
            project_id.to_hyphenated(),
            &user_id,
            &item_id,
            count.into(),
            available,
            true,
          )
          .await
          {
            tracing::error!(
              "Impossible de supprimer le hashtag `{}` au projet {}: {}",
              &item_id,
              &project_id,
              e
            )
          };
          (include_count, exclude_count) = cocktail_db_web::include_exclude_hashtag_count(
            &state.db,
            project_id.to_hyphenated(),
            &user_id,
          )
          .await
          .unwrap_or_default();
        }
        Some(_block_id) => {
          let project =
            cocktail_db_web::project(&state.db, project_id.to_hyphenated(), &user_id).await?;
          let mut request_params = project.request_params.to_vec();

          request_params[1][0]
            .data
            .retain(|x| x != &("#".to_string() + &hashtag.name));

          cocktail_db_web::update_project_request_params(
            &state.db,
            project_id.to_hyphenated(),
            &user_id,
            request_params,
          )
          .await?;
        }
      }
      let response = templates::ExcludeHashtagRemoved {
        hashtag,
        exclude_basket_path: paths::ProjectBasketExclude { project_id },
        include_count,
        exclude_count,
        block_id,
      };

      Ok(HtmlTemplate(response).into_response())
    }
  }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind")]
pub enum IncludeItem {
  // Corpus(Item),
  #[serde(rename = "add")]
  Add {
    item_id: String,
    #[serde(deserialize_with = "de_from_str")]
    available: bool, /* c'est pas génial ce nom, à changer !! */
    #[serde(deserialize_with = "de_from_str", default)]
    count: i32,
    #[serde(deserialize_with = "empty_string_as_none", default)]
    block_id: Option<i32>,
  },
  #[serde(rename = "remove")]
  Remove {
    item_id: String,
    #[serde(deserialize_with = "de_from_str")]
    available: bool, /* c'est pas génial ce nom, à changer !! */
    #[serde(deserialize_with = "de_from_str", default)]
    count: i32,
    #[serde(deserialize_with = "empty_string_as_none", default)]
    block_id: Option<i32>,
  },
}

// TODO ouais c'est naze
#[derive(Debug, Deserialize)]
#[serde(tag = "kind")]
pub enum ExcludeItem {
  // Corpus(Item),
  #[serde(rename = "add")]
  Add {
    item_id: String,
    #[serde(deserialize_with = "de_from_str")]
    available: bool, /* c'est pas génial ce nom, à changer !! */
    #[serde(deserialize_with = "de_from_str", default)]
    count: i32,
    #[serde(deserialize_with = "empty_string_as_none", default)]
    block_id: Option<i32>,
  },
  #[serde(rename = "remove")]
  Remove {
    item_id: String,
    #[serde(deserialize_with = "de_from_str")]
    available: bool, /* c'est pas génial ce nom, à changer !! */
    #[serde(deserialize_with = "de_from_str", default)]
    count: i32,
    #[serde(deserialize_with = "empty_string_as_none", default)]
    block_id: Option<i32>,
  },
}
