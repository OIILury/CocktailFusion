use axum::{
  extract::{Query, State},
  response::{IntoResponse, Redirect},
  Form,
};
use cocktail_db_twitter::{HashtagCooccurence, TopKDatabase};
use cocktail_db_web::{ParsedProjectCriteria, WebDatabase};
use fts::{Frequence, FrequenceCooccurence};
use hyper::HeaderMap;
use ory_kratos_client::apis::configuration::Configuration;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
  error::WebError,
  get_logout_url,
  models::{
    auth::AuthenticatedUser,
    templates::{AllToggled, CooccurenceToggled, HashtagToggled, HtmlTemplate, ResultHashtags},
  },
  routes::paths::{
    self, ProjectAllToggle, ProjectAsideHashtag, ProjectCooccurenceToggle, ProjectHashtagToggle,
    ProjectResultHashtags,
  },
};

#[derive(Debug, Deserialize)]
pub struct ToggleHashtag {
  pub hashtag: String,
  pub hidden: bool,
}

#[derive(Debug, Deserialize)]
pub struct ToggleCooccurence {
  pub label: String,
  pub hidden: bool,
}

#[derive(Debug, Deserialize, Default)]
pub struct ToggleAll {
  pub hidden: bool,
  pub superpose: bool,
}

#[derive(Deserialize)]
pub struct QueryParams {
  pub superpose: Option<bool>,
}

pub async fn get_hashtags_chart(
  db: &WebDatabase,
  project_id: &Uuid,
  user_id: &String,
  tab: &String,
  hidden_hashtags: &Vec<String>,
  cooccurences: &Vec<HashtagCooccurence>,
) -> Result<(Vec<Frequence>, Vec<Frequence>, Vec<FrequenceCooccurence>), WebError> {
  let directory_path = format!("project-data/{}", project_id.to_string());
  let project = cocktail_db_web::project(&db, project_id.to_hyphenated(), &user_id).await?;
  let parsed_criteria = ParsedProjectCriteria::from(&project);
  let chart =
    cocktail_db_web::get_chart(&db, &project_id.to_string(), &"hashtags".to_string(), &tab).await;
  let mut is_error = true;
  let mut frequences: Vec<Frequence> = vec![];
  let mut frequences_topk: Vec<Frequence> = vec![];
  let mut frequences_cooccurence: Vec<FrequenceCooccurence> = vec![];

  if chart.is_ok() {
    (frequences, frequences_topk, frequences_cooccurence) =
      serde_json::from_str::<(Vec<Frequence>, Vec<Frequence>, Vec<FrequenceCooccurence>)>(
        chart.unwrap().as_str(),
      )?;
    is_error = frequences.len() != parsed_criteria.hashtag_list.len()
      || frequences_topk.len() == 0
      || frequences_cooccurence.len() == 0;
  }

  if is_error {
    let index = fts::Index::open_in_dir(directory_path)?;
    let parsed_criteria = ParsedProjectCriteria::from(&project);
    frequences = fts::search_study_hashtags_count_per_day(
      &index,
      &project.start_date,
      &project.end_date,
      &parsed_criteria.hashtag_list,
      &tab,
    )?;
    frequences_topk =
      fts::search_top_hashtags_count_per_day(&index, &project.start_date, &project.end_date, &tab)?;
    frequences_cooccurence = fts::search_top_hashtags_cooccurence_count_per_day(
      &index,
      &project.start_date,
      &project.end_date,
      &tab,
      &cooccurences
        .iter()
        .map(|c| fts::HashtagCooccurence {
          hashtag1: c.hashtag1.clone(),
          hashtag2: c.hashtag2.clone(),
        })
        .collect(),
    )?;

    let _ = cocktail_db_web::save_chart(
      &db,
      project_id.to_string(),
      "hashtags".to_string(),
      tab.to_string(),
      (
        frequences.clone(),
        frequences_topk.clone(),
        frequences_cooccurence.clone(),
      ),
    )
    .await;
  }

  Ok((
    frequences
      .into_iter()
      .map(|frequence| {
        let hidden = hidden_hashtags.iter().any(|h| h == &frequence.hashtag);
        Frequence {
          hashtag: frequence.hashtag,
          hidden,
          data: frequence.data,
        }
      })
      .collect(),
    frequences_topk
      .into_iter()
      .map(|frequence| {
        let hidden = hidden_hashtags.iter().any(|h| h == &frequence.hashtag);
        Frequence {
          hashtag: frequence.hashtag,
          hidden,
          data: frequence.data,
        }
      })
      .collect(),
    frequences_cooccurence
      .into_iter()
      .map(|frequence| {
        let hidden = hidden_hashtags.iter().any(|h| h == &frequence.label);
        FrequenceCooccurence {
          label: frequence.label,
          hidden,
          data: frequence.data,
        }
      })
      .collect(),
  ))
}

pub async fn get_results(
  paths::ProjectResultHashtagsTab {
    project_id,
    tab,
    aside_hashtag_tab,
  }: paths::ProjectResultHashtagsTab,
  AuthenticatedUser {
    niveau,
    last_login_datetime,
    user_id,
  }: AuthenticatedUser,
  headers: HeaderMap,
  State(conn): State<TopKDatabase>,
  State(db): State<WebDatabase>,
  State(kratos_configuration): State<Configuration>,
  query_params: Query<QueryParams>,
) -> Result<impl IntoResponse, WebError> {
  let project = cocktail_db_web::project(&db, project_id.to_hyphenated(), &user_id).await?;

  let hidden_hashtags =
    cocktail_db_web::hidden_hashtag_list(&db, project_id.to_hyphenated(), &user_id).await?;
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

  let mut frequences_superpose = frequences.clone();

  frequences_topk.clone().iter().for_each(|f| {
    if frequences_superpose
      .iter()
      .position(|e| e.hashtag == f.hashtag)
      .is_none()
    {
      let hidden = hidden_hashtags.iter().any(|h| h == &f.hashtag);

      frequences_superpose.push(Frequence {
        hashtag: f.hashtag.to_string(),
        hidden,
        data: f.data.clone(),
      })
    }
  });

  frequences_cooccurence.clone().iter().for_each(|f| {
    let hidden = hidden_hashtags.iter().any(|h| h == &f.label);

    frequences_superpose.push(Frequence {
      hashtag: f.label.to_string(),
      hidden,
      data: f.data.clone(),
    })
  });

  let logout_url = get_logout_url(kratos_configuration, headers).await;

  let (include_count, exclude_count) =
    cocktail_db_web::include_exclude_hashtag_count(&db, project_id.to_hyphenated(), &user_id)
      .await?;

  Ok(HtmlTemplate(ResultHashtags {
    daterange_path: paths::ProjectDateRange { project_id },
    hashtag_path: paths::ProjectHashtags { project_id },
    request_path: paths::ProjectRequest { project_id },
    collect_path: paths::ProjectCollect { project_id },
    import_path: paths::ProjectImport { project_id },
    export_path: paths::ProjectCsvExport { project_id },
    analysis_preview_popup_path: paths::PopupAnalysisPreview { project_id },
    analysis_path: paths::ProjectAnalysis { project_id },
    results_path: paths::ProjectResults { project_id },
    tweets_graph_path: paths::ProjectTweetsGraph { project_id },
    result_hashtags_path: paths::ProjectResultHashtags { project_id },
    communities_path: paths::Communities { project_id },
    delete_popup_path: paths::PopupDeleteProject { project_id },
    rename_popup_path: paths::PopupRenameProject { project_id },
    duplicate_popup_path: paths::PopupDuplicateProject { project_id },
    clear_data_path: paths::ClearDataLatest { project_id },
    authors_path: paths::ProjectAuthors { project_id },
    aside_hashtag_path: ProjectAsideHashtag {
      project_id,
      tab: tab.to_string(),
      aside_hashtag_tab: aside_hashtag_tab.to_string(),
    },
    logout_url,
    include_count,
    exclude_count,
    niveau,
    last_login_datetime,
    title: project.title,
    frequences,
    frequences_topk,
    frequences_cooccurence,
    frequences_superpose,
    tab,
    tweets_count: project.tweets_count,
    authors_count: project.authors_count,
    aside_hashtag_tab,
    superpose: match query_params.superpose {
      Some(true) => true,
      _ => false,
    },
  }))
}

pub async fn hashtags(
  paths::ProjectResultHashtags { project_id }: paths::ProjectResultHashtags,
  AuthenticatedUser {
    niveau,
    last_login_datetime,
    user_id,
  }: AuthenticatedUser,
  headers: HeaderMap,
  State(conn): State<TopKDatabase>,
  State(db): State<WebDatabase>,
  State(kratos_configuration): State<Configuration>,
  query_params: Query<QueryParams>,
) -> Result<impl IntoResponse, WebError> {
  get_results(
    paths::ProjectResultHashtagsTab {
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
    State(conn),
    State(db),
    State(kratos_configuration),
    query_params,
  )
  .await
}

pub async fn hashtags_tab(
  paths::ProjectResultHashtagsTab {
    project_id,
    tab,
    aside_hashtag_tab,
  }: paths::ProjectResultHashtagsTab,
  AuthenticatedUser {
    niveau,
    last_login_datetime,
    user_id,
  }: AuthenticatedUser,
  headers: HeaderMap,
  State(conn): State<TopKDatabase>,
  State(db): State<WebDatabase>,
  State(kratos_configuration): State<Configuration>,
  query_params: Query<QueryParams>,
) -> Result<impl IntoResponse, WebError> {
  if niveau != 2 && tab != "total" {
    return Ok(
      Redirect::to(
        paths::ProjectResultHashtags { project_id }
          .to_string()
          .as_str(),
      )
      .into_response(),
    );
  }
  Ok(
    get_results(
      paths::ProjectResultHashtagsTab {
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
      State(conn),
      State(db),
      State(kratos_configuration),
      query_params,
    )
    .await
    .into_response(),
  )
}

#[tracing::instrument]
pub async fn toggle_hashtag(
  ProjectHashtagToggle { project_id }: ProjectHashtagToggle,
  AuthenticatedUser {
    niveau: _,
    last_login_datetime: _,
    user_id,
  }: AuthenticatedUser,
  State(db): State<WebDatabase>,
  Form(toggle): Form<ToggleHashtag>,
) -> Result<impl IntoResponse, WebError> {
  cocktail_db_web::toggle_hashtag(
    &db,
    project_id.to_hyphenated(),
    &user_id,
    &toggle.hashtag,
    toggle.hidden,
  )
  .await?;

  Ok(HtmlTemplate(HashtagToggled {
    hashtag_toggle_path: ProjectHashtagToggle { project_id },
    frequence: Frequence {
      hashtag: toggle.hashtag,
      hidden: toggle.hidden,
      data: vec![],
    },
  }))
}

#[tracing::instrument]
pub async fn toggle_all(
  ProjectAllToggle {
    project_id,
    tab,
    aside_hashtag_tab,
  }: ProjectAllToggle,
  AuthenticatedUser {
    niveau: _,
    last_login_datetime: _,
    user_id,
  }: AuthenticatedUser,
  State(db): State<WebDatabase>,
  State(conn): State<TopKDatabase>,
  Form(toggle): Form<ToggleAll>,
) -> Result<impl IntoResponse, WebError> {
  let cooccurences = cocktail_db_twitter::search_topk_hashtags_cooccurence(conn.clone()).await?;

  let (mut frequences, mut frequences_topk, mut frequences_cooccurence) = get_hashtags_chart(
    &db,
    &project_id,
    &user_id,
    &"total".to_string(),
    &vec![],
    &cooccurences,
  )
  .await?;

  frequences = frequences
    .iter()
    .map(|e| Frequence {
      hashtag: e.hashtag.to_string(),
      hidden: toggle.hidden,
      data: vec![],
    })
    .collect();

  frequences_topk = frequences_topk
    .iter()
    .map(|e| Frequence {
      hashtag: e.hashtag.to_string(),
      hidden: toggle.hidden,
      data: vec![],
    })
    .collect();

  frequences_cooccurence = frequences_cooccurence
    .into_iter()
    .map(|e| FrequenceCooccurence {
      label: e.label,
      hidden: toggle.hidden,
      data: vec![],
    })
    .collect();

  if aside_hashtag_tab == "project" {
    cocktail_db_web::toggle_all(
      &db,
      project_id.to_hyphenated(),
      &user_id,
      toggle.hidden,
      frequences.iter().map(|e| e.hashtag.to_string()).collect(),
    )
    .await?;
  } else if aside_hashtag_tab == "top" {
    cocktail_db_web::toggle_all(
      &db,
      project_id.to_hyphenated(),
      &user_id,
      toggle.hidden,
      frequences_topk
        .iter()
        .map(|e| e.hashtag.to_string())
        .collect(),
    )
    .await?;
  }

  let mut hidden_hashtags = match toggle.hidden {
    false => frequences.clone().into_iter().map(|f| f.hashtag).collect(),
    _ => vec![],
  };

  if !toggle.hidden {
    hidden_hashtags.append(
      &mut frequences_topk
        .clone()
        .into_iter()
        .map(|f| f.hashtag)
        .collect(),
    );
  }

  Ok(HtmlTemplate(AllToggled {
    all_toggle_path: ProjectAllToggle {
      project_id,
      tab: tab.to_string(),
      aside_hashtag_tab: aside_hashtag_tab.to_string(),
    },
    hashtag_toggle_path: ProjectHashtagToggle { project_id },
    cooccurence_toggle_path: ProjectCooccurenceToggle { project_id },
    hidden: toggle.hidden,
    frequences,
    frequences_topk,
    frequences_cooccurence,
    result_hashtags_path: ProjectResultHashtags { project_id },
    tab,
    aside_hashtag_tab,
    superpose: toggle.superpose,
  }))
}

pub async fn aside_hashtag_tab(
  ProjectAsideHashtag {
    project_id,
    tab,
    aside_hashtag_tab,
  }: ProjectAsideHashtag,
  AuthenticatedUser {
    niveau: _,
    last_login_datetime: _,
    user_id,
  }: AuthenticatedUser,
  State(db): State<WebDatabase>,
  State(conn): State<TopKDatabase>,
  query_params: Query<QueryParams>,
) -> Result<impl IntoResponse, WebError> {
  let cooccurences = cocktail_db_twitter::search_topk_hashtags_cooccurence(conn.clone()).await?;

  let (frequences, frequences_topk, frequences_cooccurence) = get_hashtags_chart(
    &db,
    &project_id,
    &user_id,
    &"total".to_string(),
    &vec![],
    &cooccurences,
  )
  .await?;

  let hidden_hashtags =
    cocktail_db_web::hidden_hashtag_list(&db, project_id.to_hyphenated(), &user_id).await?;
  let hidden = !&hidden_hashtags.is_empty();

  Ok(HtmlTemplate(AllToggled {
    all_toggle_path: ProjectAllToggle {
      project_id,
      tab: tab.to_string(),
      aside_hashtag_tab: aside_hashtag_tab.to_string(),
    },
    hashtag_toggle_path: ProjectHashtagToggle { project_id },
    cooccurence_toggle_path: ProjectCooccurenceToggle { project_id },
    hidden,
    frequences: frequences
      .into_iter()
      .map(|frequence| {
        let hidden = hidden_hashtags.iter().any(|h| h == &frequence.hashtag);
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
        let hidden = hidden_hashtags.iter().any(|h| h == &frequence.hashtag);
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
        let hidden = hidden_hashtags.iter().any(|h| h == &frequence.label);
        FrequenceCooccurence {
          label: frequence.label,
          hidden,
          data: frequence.data,
        }
      })
      .collect(),
    result_hashtags_path: ProjectResultHashtags { project_id },
    tab,
    aside_hashtag_tab,
    superpose: match query_params.superpose {
      Some(true) => true,
      _ => false,
    },
  }))
}

#[tracing::instrument]
pub async fn toggle_hashtag_cooccurence(
  ProjectCooccurenceToggle { project_id }: ProjectCooccurenceToggle,
  AuthenticatedUser {
    niveau: _,
    last_login_datetime: _,
    user_id,
  }: AuthenticatedUser,
  State(db): State<WebDatabase>,
  Form(toggle): Form<ToggleCooccurence>,
) -> Result<impl IntoResponse, WebError> {
  cocktail_db_web::toggle_hashtag(
    &db,
    project_id.to_hyphenated(),
    &user_id,
    &toggle.label,
    toggle.hidden,
  )
  .await?;

  Ok(HtmlTemplate(CooccurenceToggled {
    cooccurence_toggle_path: ProjectCooccurenceToggle { project_id },
    cooccurence: FrequenceCooccurence {
      label: toggle.label,
      hidden: toggle.hidden,
      data: vec![],
    },
  }))
}
