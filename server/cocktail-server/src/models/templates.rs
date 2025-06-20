use std::fmt::Display;

use askama::Template;
use axum::{
  body::Full,
  http::StatusCode,
  response::{Html, IntoResponse, Response},
};
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use cocktail_db_web::{Bloc, HashtagWithCount, TweetsChart};
use cocktail_graph_utils::{JsonDataGraph, Status};
use fts::{Author, AuthorCount, Frequence, FrequenceCooccurence, Tweet};
use hyper::header;
use uuid::Uuid;

use crate::routes::{
  paths::{
    self, ProjectAllToggle, ProjectAsideHashtag, ProjectCooccurenceToggle, ProjectHashtagToggle,
    ProjectImport, ProjectCollect, ProjectCsvExport,
  },
  study::results::FilterAuthor,
};

#[derive(Debug, Template)]
#[template(path = "projets.html")]
pub(crate) struct ProjectsTemplate {
  pub projects: Vec<cocktail_db_web::Project>,
  pub last_login_datetime: NaiveDateTime,
  pub logout_url: String,
}

#[derive(Debug, Template)]
#[template(path = "projets-popup.html")]
pub(crate) struct NewProjectsTemplate {
  pub projects: Vec<cocktail_db_web::Project>,
  pub last_login_datetime: NaiveDateTime,
  pub logout_url: String,
}

#[derive(Debug, Template)]
#[template(path = "daterange.html")]
#[allow(dead_code)]
pub(crate) struct DateRange {
  pub daterange_path: paths::ProjectDateRange,
  pub hashtag_path: paths::ProjectHashtags,
  pub request_path: paths::ProjectRequest,
  pub collect_path: paths::ProjectCollect,
  pub import_path: paths::ProjectImport,
  pub export_path: paths::ProjectCsvExport,
  pub delete_popup_path: paths::PopupDeleteProject,
  pub rename_popup_path: paths::PopupRenameProject,
  pub download_path: paths::DownloadProject,
  pub duplicate_popup_path: paths::PopupDuplicateProject,
  pub analysis_preview_popup_path: paths::PopupAnalysisPreview,
  pub analysis_path: paths::ProjectAnalysis,
  pub is_analyzed: bool,
  pub results_path: paths::ProjectResults,
  pub tweets_graph_path: paths::ProjectTweetsGraph,
  pub authors_path: paths::ProjectAuthors,
  pub result_hashtags_path: paths::ProjectResultHashtags,
  pub communities_path: paths::Communities,
  pub logout_url: String,
  pub include_count: i64,
  pub exclude_count: i64,
  pub niveau: i64,
  pub last_login_datetime: NaiveDateTime,
  pub title: String,
  pub start_date: NaiveDate,
  pub end_date: NaiveDate,
  pub show_custom_range: bool,
  pub tweets_count: i64,
  pub authors_count: i64,
}

#[derive(Debug, Template)]
#[template(path = "hashtags.html")]
pub(crate) struct Hashtags {
  pub include_basket: Vec<HashtagWithCount>,
  pub exclude_basket: Vec<HashtagWithCount>,
  pub include_count: i64,
  pub exclude_count: i64,
  pub popup_path: paths::PopupHashtags,
  pub delete_popup_path: paths::PopupDeleteProject,
  pub rename_popup_path: paths::PopupRenameProject,
  pub download_path: paths::DownloadProject,
  pub duplicate_popup_path: paths::PopupDuplicateProject,
  pub include_basket_path: paths::ProjectBasketInclude,
  pub exclude_basket_path: paths::ProjectBasketExclude,
  pub daterange_path: paths::ProjectDateRange,
  pub hashtag_path: paths::ProjectHashtags,
  pub request_path: paths::ProjectRequest,
  pub collect_path: paths::ProjectCollect,
  pub import_path: paths::ProjectImport,
  pub export_path: paths::ProjectCsvExport,
  pub analysis_preview_popup_path: paths::PopupAnalysisPreview,
  pub analysis_path: paths::ProjectAnalysis,
  pub is_analyzed: bool,
  pub results_path: paths::ProjectResults,
  pub tweets_graph_path: paths::ProjectTweetsGraph,
  pub authors_path: paths::ProjectAuthors,
  pub result_hashtags_path: paths::ProjectResultHashtags,
  pub communities_path: paths::Communities,
  pub logout_url: String,
  pub niveau: i64,
  pub last_login_datetime: NaiveDateTime,
  pub title: String,
  pub tweets_count: i64,
  pub authors_count: i64,
}

#[derive(Debug, Template)]
#[template(path = "hashtags-popup/popup.html")]
pub(crate) struct PopupHashtags {
  //pub popup_hashtags_corpus_path: paths::PopupHashtagsCorpus,
  pub popup_hashtags_topk_path: paths::PopupHashtagsTopK,
  pub popup_hashtags_search_path: paths::PopupHashtagsSearch,
  pub request_path: paths::ProjectRequest,
  pub hashtag_count: i64,
  pub exclude_popup_style: bool,
  pub block_id: Option<i32>,
}

#[derive(Debug, Template)]
#[template(path = "hashtags-popup/_corpus.html")]
pub(crate) struct PopupHashtagsCorpus;

#[derive(Debug, Template)]
#[template(path = "hashtags-popup/_topk.html")]
pub(crate) struct PopupHashtagsTopK {
  pub hashtags: Vec<Item>,
  // pub basket_path: ProjectBasket,
  pub include_basket_path: paths::ProjectBasketInclude,
  pub exclude_basket_path: paths::ProjectBasketExclude,
  pub exclude_popup_style: bool,
  pub block_id: Option<i32>,
}

#[derive(Debug, Template)]
#[template(path = "hashtags-popup/_search.html")]
pub(crate) struct PopupHashtagsSearch {
  pub hashtags_search_path: paths::PopupHashtagsSearch,
  pub exclude_popup_style: bool,
  pub block_id: Option<i32>,
}

#[derive(Debug, Template)]
#[template(path = "hashtags-popup/_search-result.html")]
pub(crate) struct PopupHashtagsSearchResult {
  pub hashtags: Vec<Item>,
  pub q: String,
  // pub basket_path: ProjectBasket,
  pub include_basket_path: paths::ProjectBasketInclude,
  pub exclude_basket_path: paths::ProjectBasketExclude,
  pub exclude_popup_style: bool,
  pub block_id: Option<i32>,
}

#[derive(Debug, Template)]
#[template(path = "delete_popup.html")]
pub(crate) struct PopupDeleteProject {
  pub project_id: Uuid,
}

#[derive(Debug, Template)]
#[template(path = "rename_popup.html")]
pub(crate) struct PopupRenameProject {
  pub project_id: Uuid,
}

#[derive(Debug, Template)]
#[template(path = "duplicate_popup.html")]
pub(crate) struct PopupDuplicateProject {
  pub project_id: Uuid,
  pub project_title: String,
}

#[derive(Template)]
#[template(path = "request.html")]
pub(crate) struct Request {
  pub daterange_path: paths::ProjectDateRange,
  pub hashtag_path: paths::ProjectHashtags,
  pub request_path: paths::ProjectRequest,
  pub collect_path: paths::ProjectCollect,
  pub import_path: paths::ProjectImport,
  pub export_path: paths::ProjectCsvExport,
  pub request_params: Vec<Vec<Bloc>>,
  pub popup_hashtags_path: paths::PopupHashtags,
  pub popup_keywords_path: paths::PopupKeywords,
  pub popup_accounts_path: paths::PopupAccounts,
  pub analysis_preview_popup_path: paths::PopupAnalysisPreview,
  pub analysis_path: paths::ProjectAnalysis,
  pub is_analyzed: bool,
  pub results_path: paths::ProjectResults,
  pub tweets_graph_path: paths::ProjectTweetsGraph,
  pub authors_path: paths::ProjectAuthors,
  pub result_hashtags_path: paths::ProjectResultHashtags,
  pub communities_path: paths::Communities,
  pub delete_popup_path: paths::PopupDeleteProject,
  pub rename_popup_path: paths::PopupRenameProject,
  pub download_path: paths::DownloadProject,
  pub duplicate_popup_path: paths::PopupDuplicateProject,
  pub logout_url: String,
  pub include_count: i64,
  pub exclude_count: i64,
  pub niveau: i64,
  pub last_login_datetime: NaiveDateTime,
  pub title: String,
  pub tweets_count: i64,
  pub authors_count: i64,
}

#[derive(Template)]
#[template(
  source = r#"
<turbo-stream target="request-view" action="update">
<template>
    {% include "_request_form.html" %}
  </template>
</turbo-stream>
  "#,
  ext = "html"
)]
pub(crate) struct BlocsUpdate {
  pub request_params: Vec<Vec<Bloc>>,
  pub popup_hashtags_path: paths::PopupHashtags,
  pub popup_keywords_path: paths::PopupKeywords,
  pub popup_accounts_path: paths::PopupAccounts,
}

#[derive(Debug, Template)]
#[template(path = "keywords_popup.html")]
pub(crate) struct PopupKeywords {
  pub request_path: paths::ProjectRequest,
  pub block_id: i32,
}

#[derive(Debug, Template)]
#[template(path = "accounts_popup.html")]
pub(crate) struct PopupAccounts {
  pub request_path: paths::ProjectRequest,
  pub block_id: i32,
}

#[derive(Template)]
#[template(path = "preview_popup.html")]
pub(crate) struct PopupAnalysisPreview {
  pub tweets: Vec<Tweet>,
  pub count: i64,
}

#[derive(Template)]
#[template(path = "results.html")]
pub(crate) struct Results {
  pub daterange_path: paths::ProjectDateRange,
  pub hashtag_path: paths::ProjectHashtags,
  pub request_path: paths::ProjectRequest,
  pub collect_path: paths::ProjectCollect,
  pub import_path: paths::ProjectImport,
  pub export_path: paths::ProjectCsvExport,
  pub analysis_preview_popup_path: paths::PopupAnalysisPreview,
  pub analysis_path: paths::ProjectAnalysis,
  pub results_path: paths::ProjectResults,
  pub tweets_graph_path: paths::ProjectTweetsGraph,
  pub authors_path: paths::ProjectAuthors,
  pub result_hashtags_path: paths::ProjectResultHashtags,
  pub communities_path: paths::Communities,
  pub delete_popup_path: paths::PopupDeleteProject,
  pub rename_popup_path: paths::PopupRenameProject,
  pub duplicate_popup_path: paths::PopupDuplicateProject,
  pub download_path: paths::DownloadProject,
  pub logout_url: String,
  pub include_count: i64,
  pub exclude_count: i64,
  pub niveau: i64,
  pub last_login_datetime: NaiveDateTime,
  pub title: String,
  pub tweets: Vec<Tweet>,
  pub frequences: Vec<Frequence>,
  pub frequences_topk: Vec<Frequence>,
  pub frequences_cooccurence: Vec<FrequenceCooccurence>,
  pub authors: Vec<FilterAuthor>,
  pub tab: String,
  pub aside_hashtag_tab: String,
  pub user_screen_name: String,
  pub date: Option<NaiveDate>,
  pub hashtag: Option<String>,
  pub page: u32,
  pub order: String,
  pub tweets_count: i64,
  pub authors_count: i64,
  pub hidden: bool,
}

#[derive(Template)]
#[template(path = "authors.html")]
pub(crate) struct Authors {
  pub daterange_path: paths::ProjectDateRange,
  pub hashtag_path: paths::ProjectHashtags,
  pub request_path: paths::ProjectRequest,
  pub collect_path: paths::ProjectCollect,
  pub import_path: paths::ProjectImport,
  pub export_path: paths::ProjectCsvExport,
  pub analysis_preview_popup_path: paths::PopupAnalysisPreview,
  pub analysis_path: paths::ProjectAnalysis,
  pub results_path: paths::ProjectResults,
  pub tweets_graph_path: paths::ProjectTweetsGraph,
  pub authors_path: paths::ProjectAuthors,
  pub result_hashtags_path: paths::ProjectResultHashtags,
  pub communities_path: paths::Communities,
  pub delete_popup_path: paths::PopupDeleteProject,
  pub rename_popup_path: paths::PopupRenameProject,
  pub download_path: paths::DownloadProject,
  pub duplicate_popup_path: paths::PopupDuplicateProject,
  pub logout_url: String,
  pub include_count: i64,
  pub exclude_count: i64,
  pub niveau: i64,
  pub last_login_datetime: NaiveDateTime,
  pub title: String,
  pub author_counts: Vec<AuthorCount>,
  pub tab: String,
  pub page: u32,
  pub tweets_count: i64,
  pub authors_count: i64,
}

#[derive(Template)]
#[template(path = "result_hashtags.html")]
pub(crate) struct ResultHashtags {
  pub daterange_path: paths::ProjectDateRange,
  pub hashtag_path: paths::ProjectHashtags,
  pub request_path: paths::ProjectRequest,
  pub collect_path: paths::ProjectCollect,
  pub import_path: paths::ProjectImport,
  pub export_path: paths::ProjectCsvExport,
  pub analysis_preview_popup_path: paths::PopupAnalysisPreview,
  pub analysis_path: paths::ProjectAnalysis,
  pub results_path: paths::ProjectResults,
  pub tweets_graph_path: paths::ProjectTweetsGraph,
  pub result_hashtags_path: paths::ProjectResultHashtags,
  pub communities_path: paths::Communities,
  pub delete_popup_path: paths::PopupDeleteProject,
  pub rename_popup_path: paths::PopupRenameProject,
  pub download_path: paths::DownloadProject,
  pub duplicate_popup_path: paths::PopupDuplicateProject,
  pub aside_hashtag_path: ProjectAsideHashtag,
  pub authors_path: paths::ProjectAuthors,
  pub logout_url: String,
  pub include_count: i64,
  pub exclude_count: i64,
  pub niveau: i64,
  pub last_login_datetime: NaiveDateTime,
  pub title: String,
  pub frequences: Vec<Frequence>,
  pub frequences_topk: Vec<Frequence>,
  pub frequences_cooccurence: Vec<FrequenceCooccurence>,
  pub frequences_superpose: Vec<Frequence>,
  pub tab: String,
  pub tweets_count: i64,
  pub authors_count: i64,
  pub aside_hashtag_tab: String,
  pub superpose: bool,
}

#[derive(Template)]
#[template(path = "tweets.html")]
pub(crate) struct Tweets {
  pub daterange_path: paths::ProjectDateRange,
  pub hashtag_path: paths::ProjectHashtags,
  pub request_path: paths::ProjectRequest,
  pub collect_path: paths::ProjectCollect,
  pub import_path: paths::ProjectImport,
  pub export_path: paths::ProjectCsvExport,
  pub analysis_preview_popup_path: paths::PopupAnalysisPreview,
  pub analysis_path: paths::ProjectAnalysis,
  pub results_path: paths::ProjectResults,
  pub tweets_graph_path: paths::ProjectTweetsGraph,
  pub authors_path: paths::ProjectAuthors,
  pub result_hashtags_path: paths::ProjectResultHashtags,
  pub communities_path: paths::Communities,
  pub delete_popup_path: paths::PopupDeleteProject,
  pub rename_popup_path: paths::PopupRenameProject,
  pub download_path: paths::DownloadProject,
  pub duplicate_popup_path: paths::PopupDuplicateProject,
  pub authors_select_path: paths::ProjectAuthorsSelect,
  pub logout_url: String,
  pub include_count: i64,
  pub exclude_count: i64,
  pub niveau: i64,
  pub last_login_datetime: NaiveDateTime,
  pub title: String,
  pub tweets_chart: TweetsChart,
  pub frequences: Vec<Frequence>,
  pub frequences_topk: Vec<Frequence>,
  pub frequences_cooccurence: Vec<FrequenceCooccurence>,
  pub tab: String,
  pub aside_tweet_tab: String,
  pub selected_author: String,
  pub tweets_count: i64,
  pub authors_count: i64,
  pub hidden: bool,
}

#[derive(Template)]
#[template(path = "_authors_select.html")]
pub(crate) struct AuthorsSelect {
  pub tweets_graph_path: paths::ProjectTweetsGraph,
  pub tab: String,
  pub authors: Vec<Author>,
  pub selected_author: String,
}

#[derive(Template)]
#[template(
  source = r#"
<turbo-stream target="basket-{{ hashtag.name }}" action="remove"></turbo-stream>
<turbo-stream action="replace" target="toggle-{{hashtag.name}}">
    <template>
    {% include "_includes/_hashtag_toggle_form.html" %}
    </template>
</turbo-stream>
<turbo-stream action="update" target="hashtag-count">
    <template>
        {{ include_count }}
    </template>
</turbo-stream>
<turbo-stream action="update" target="include-count">
    <template>
        {{ include_count }}
    </template>
</turbo-stream>
<turbo-stream action="update" target="exclude-count">
    <template>
        {{ exclude_count }}
    </template>
</turbo-stream>
  "#,
  ext = "html"
)]
pub(crate) struct IncludeHashtagRemoved {
  pub hashtag: Item,
  pub include_basket_path: paths::ProjectBasketInclude,
  pub include_count: i64,
  pub exclude_count: i64,
  pub block_id: Option<i32>,
}

#[derive(Template)]
#[template(
  source = r#"
<turbo-stream target="include-basket" action="append">
  <template>
    {% let kind = "remove" %}
    {% include "_hashtag_include_row.html" %}
  </template>
</turbo-stream>
<turbo-stream action="replace" target="toggle-{{hashtag.name}}">
    <template>
    {% include "_includes/_hashtag_toggle_form.html" %}
    </template>
</turbo-stream>
<turbo-stream action="update" target="hashtag-count">
    <template>
        {{ include_count }}
    </template>
</turbo-stream>
<turbo-stream action="update" target="include-count">
    <template>
        {{ include_count }}
    </template>
</turbo-stream>
<turbo-stream action="update" target="exclude-count">
    <template>
        {{ exclude_count }}
    </template>
</turbo-stream>
  "#,
  ext = "html"
)]
pub(crate) struct IncludeHashtagAdded {
  pub hashtag: Item,
  pub include_basket_path: paths::ProjectBasketInclude,
  pub include_count: i64,
  pub exclude_count: i64,
  pub block_id: Option<i32>,
}
#[derive(Template)]
#[template(
  source = r#"
<turbo-stream target="exclude-basket-{{ hashtag.name }}" action="remove"></turbo-stream>
<turbo-stream action="replace" target="toggle-{{hashtag.name}}">
    <template>
    {% include "_includes/_exclude_hashtag_toggle_form.html" %}
    </template>
</turbo-stream>
<turbo-stream action="update" target="hashtag-count">
    <template>
        {{ exclude_count }}
    </template>
</turbo-stream>
<turbo-stream action="update" target="include-count">
    <template>
        {{ include_count }}
    </template>
</turbo-stream>
<turbo-stream action="update" target="exclude-count">
    <template>
        {{ exclude_count }}
    </template>
</turbo-stream>
  "#,
  ext = "html"
)]
pub(crate) struct ExcludeHashtagRemoved {
  pub hashtag: Item,
  pub exclude_basket_path: paths::ProjectBasketExclude,
  pub include_count: i64,
  pub exclude_count: i64,
  pub block_id: Option<i32>,
}

#[derive(Template)]
#[template(
  source = r#"
<turbo-stream target="exclude-basket" action="append">
  <template>
    {% let kind = "remove" %}
    {% include "_hashtag_exclude_row.html" %}
  </template>
</turbo-stream>
<turbo-stream action="replace" target="toggle-{{hashtag.name}}">
    <template>
    {% include "_includes/_exclude_hashtag_toggle_form.html" %}
    </template>
</turbo-stream>
<turbo-stream action="update" target="hashtag-count">
    <template>
        {{ exclude_count }}
    </template>
</turbo-stream>
<turbo-stream action="update" target="include-count">
    <template>
        {{ include_count }}
    </template>
</turbo-stream>
<turbo-stream action="update" target="exclude-count">
    <template>
        {{ exclude_count }}
    </template>
</turbo-stream>
  "#,
  ext = "html"
)]
pub(crate) struct ExcludeHashtagAdded {
  pub hashtag: Item,
  pub exclude_basket_path: paths::ProjectBasketExclude,
  pub include_count: i64,
  pub exclude_count: i64,
  pub block_id: Option<i32>,
}

#[derive(Template)]
#[template(
  source = r#"
<turbo-stream action="update" target="title">
    <template>
        {{ title }}
    </template>
</turbo-stream>
"#,
  ext = "html"
)]
pub(crate) struct TitleChanged {
  pub title: String,
}

#[derive(Template)]
#[template(
  source = r#"
<turbo-stream action="replace" target="frequence-{{frequence.hashtag}}">
    <template>
        {% include "_includes/_toggle_hashtag_visibility.html" %}
    </template>
</turbo-stream>
"#,
  ext = "html"
)]
pub(crate) struct HashtagToggled {
  pub frequence: Frequence,
  pub hashtag_toggle_path: ProjectHashtagToggle,
}

#[derive(Template)]
#[template(
  source = r#"
<turbo-stream action="replace" target="aside-hashtags-chart">
    <template>
        {% include "_includes/_aside_hashtags_chart.html" %}
    </template>
</turbo-stream>
"#,
  ext = "html"
)]
pub(crate) struct AllToggled {
  pub hidden: bool,
  pub all_toggle_path: ProjectAllToggle,
  pub hashtag_toggle_path: paths::ProjectHashtagToggle,
  pub cooccurence_toggle_path: ProjectCooccurenceToggle,
  pub frequences: Vec<Frequence>,
  pub frequences_topk: Vec<Frequence>,
  pub frequences_cooccurence: Vec<FrequenceCooccurence>,
  pub result_hashtags_path: paths::ProjectResultHashtags,
  pub tab: String,
  pub aside_hashtag_tab: String,
  pub superpose: bool,
}

#[derive(Template)]
#[template(
  source = r#"
<turbo-stream action="replace" target="{{cooccurence.label}}">
    <template>
        {% include "_includes/_toggle_cooccurence_visibility.html" %}
    </template>
</turbo-stream>
"#,
  ext = "html"
)]
pub(crate) struct CooccurenceToggled {
  pub cooccurence: FrequenceCooccurence,
  pub cooccurence_toggle_path: ProjectCooccurenceToggle,
}

#[derive(Debug, Clone)]
pub struct Item {
  pub item_id: String,
  pub name: String,
  pub count: i64,
  pub available: bool,
  pub kind: ItemKind,
}

impl From<cocktail_db_twitter::Hashtag> for Item {
  fn from(h: cocktail_db_twitter::Hashtag) -> Self {
    Self {
      item_id: h.hashtag.clone(),
      name: h.hashtag,
      count: h.count,
      available: h.available,
      kind: ItemKind::Hashtag,
    }
  }
}

#[derive(Debug, Template)]
#[template(path = "communities.html")]
pub(crate) struct Communities {
  pub json_data: Option<JsonDataGraph>,
  pub daterange_path: paths::ProjectDateRange,
  pub hashtag_path: paths::ProjectHashtags,
  pub request_path: paths::ProjectRequest,
  pub collect_path: paths::ProjectCollect,
  pub import_path: paths::ProjectImport,
  pub export_path: paths::ProjectCsvExport,
  pub analysis_preview_popup_path: paths::PopupAnalysisPreview,
  pub analysis_path: paths::ProjectAnalysis,
  pub results_path: paths::ProjectResults,
  pub tweets_graph_path: paths::ProjectTweetsGraph,
  pub authors_path: paths::ProjectAuthors,
  pub result_hashtags_path: paths::ProjectResultHashtags,
  pub communities_path: paths::Communities,
  pub delete_popup_path: paths::PopupDeleteProject,
  pub rename_popup_path: paths::PopupRenameProject,
  pub download_path: paths::DownloadProject,
  pub duplicate_popup_path: paths::PopupDuplicateProject,
  pub logout_url: String,
  pub include_count: i64,
  pub exclude_count: i64,
  pub niveau: i64,
  pub last_login_datetime: NaiveDateTime,
  pub title: String,
  pub status: Vec<Status>,
  pub tab: String,
  pub community: String,
  pub centrality: String,
  pub max_rank: i64,
  pub show_interaction: bool,
  pub tweets_count: i64,
  pub authors_count: i64,
  pub modularity: f64,
}

impl From<String> for Item {
  fn from(hashtag: String) -> Self {
    Self {
      item_id: hashtag.clone(),
      name: hashtag,
      count: 0,
      available: true,
      kind: ItemKind::Hashtag,
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ItemKind {
  // Corpus,
  Hashtag,
}

impl Display for ItemKind {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      // ItemKind::Corpus => write!(f, "Corpus"),
      ItemKind::Hashtag => write!(f, "Hashtag"),
    }
  }
}

pub(crate) struct HtmlTemplate<T>(pub T);

impl<T> IntoResponse for HtmlTemplate<T>
where
  T: Template,
{
  fn into_response(self) -> Response {
    match self.0.render() {
      Ok(html) => Html(html).into_response(),
      Err(err) => Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .body(Full::from(format!(
          "Failed to render template. Error: {err}"
        )))
        .unwrap()
        .into_response(),
    }
  }
}

pub(crate) struct StaticFile<T>(pub T);

impl<T> IntoResponse for StaticFile<T>
where
  T: Into<String>,
{
  fn into_response(self) -> Response {
    let path = self.0.into();

    match Assets::get(path.as_str()) {
      Some(content) => {
        let body = Full::from(content.data);
        let mime = mime_guess::from_path(path).first_or_octet_stream();

        Response::builder()
          .header(header::CONTENT_TYPE, mime.as_ref())
          .body(body)
          .unwrap()
          .into_response()
      }
      None => Response::builder()
        .body(Full::from("nope"))
        .unwrap()
        .into_response(),
    }
  }
}

mod filters {
  use crate::routes::paths;
  use chrono::{Local, NaiveDate, NaiveDateTime, NaiveTime};
  use num_format::{Locale, ToFormattedString};
  use std::str::FromStr;
  use uuid::Uuid;

  pub fn daterange_path(p: &cocktail_db_web::Project) -> askama::Result<String> {
    let project_id = Uuid::from_str(p.project_id.to_string().as_str()).unwrap(); // ouais parce que ça sort de la base de données, ça aurait planté avant !
    let path = paths::ProjectDateRange { project_id };

    Ok(path.to_string())
  }

  pub fn date(p: &NaiveDate) -> askama::Result<Option<String>> {
    let from_ymd = NaiveDate::from_ymd;
    if *p == from_ymd(1970, 1, 1) {
      return Ok(None);
    }

    Ok(Some(p.format("%d/%m/%Y").to_string()))
  }

  pub fn datetime(p: &NaiveDateTime) -> askama::Result<String> {
    let default = NaiveDateTime::new(
      NaiveDate::from_ymd(1970, 1, 1),
      NaiveTime::from_hms(0, 0, 0),
    );
    if *p == default {
      return Ok("".to_string());
    }

    Ok(p.format("%d/%m/%Y à %H:%M").to_string())
  }

  pub fn datetime_str(p: &String) -> askama::Result<String> {
    if p.is_empty() {
      return Ok("".to_string());
    }
    
    // Si la string est déjà formatée, on la retourne telle quelle
    Ok(p.clone())
  }

  pub fn num_format(p: &i64) -> askama::Result<String> {
    Ok(p.to_formatted_string(&Locale::fr))
  }

  pub fn current_year(_: &str) -> askama::Result<String> {
    Ok(Local::now().naive_local().format("%Y").to_string())
  }
}

#[derive(rust_embed::RustEmbed)]
#[folder = "$CARGO_MANIFEST_DIR/templates/auth"]
pub(crate) struct Templates;

#[derive(rust_embed::RustEmbed)]
#[folder = "$CARGO_MANIFEST_DIR/templates/static"]
pub(crate) struct Assets;

#[derive(Template)]
#[template(path = "collect.html")]
pub(crate) struct Collect {
    pub daterange_path: paths::ProjectDateRange,
    pub hashtag_path: paths::ProjectHashtags,
    pub request_path: paths::ProjectRequest,
    pub collect_path: paths::ProjectCollect,
    pub import_path: paths::ProjectImport,
    pub export_path: paths::ProjectCsvExport,
    pub delete_popup_path: paths::PopupDeleteProject,
    pub rename_popup_path: paths::PopupRenameProject,
    pub duplicate_popup_path: paths::PopupDuplicateProject,
    pub download_path: paths::DownloadProject,
    pub analysis_preview_popup_path: paths::PopupAnalysisPreview,
    pub analysis_path: paths::ProjectAnalysis,
    pub is_analyzed: bool,
    pub results_path: paths::ProjectResults,
    pub tweets_graph_path: paths::ProjectTweetsGraph,
    pub authors_path: paths::ProjectAuthors,
    pub result_hashtags_path: paths::ProjectResultHashtags,
    pub communities_path: paths::Communities,
    pub logout_url: String,
    pub include_count: i64,
    pub exclude_count: i64,
    pub niveau: i64,
    pub last_login_datetime: NaiveDateTime,
    pub title: String,
    pub tweets_count: i64,
    pub authors_count: i64,
}

#[derive(Debug, Template)]
#[template(path = "import.html")]
#[allow(dead_code)]
pub struct ImportTemplate {
    pub project_id: String,
    pub import_path: paths::ProjectImport,
    pub export_path: paths::ProjectCsvExport,
    pub collect_path: paths::ProjectCollect,
    pub is_analyzed: bool,
    pub daterange_path: paths::ProjectDateRange,
    pub hashtag_path: paths::ProjectHashtags,
    pub request_path: paths::ProjectRequest,
    pub delete_popup_path: paths::PopupDeleteProject,
    pub rename_popup_path: paths::PopupRenameProject,
    pub download_path: paths::DownloadProject,
    pub duplicate_popup_path: paths::PopupDuplicateProject,
    pub analysis_preview_popup_path: paths::PopupAnalysisPreview,
    pub analysis_path: paths::ProjectAnalysis,
    pub results_path: paths::ProjectResults,
    pub tweets_graph_path: paths::ProjectTweetsGraph,
    pub authors_path: paths::ProjectAuthors,
    pub result_hashtags_path: paths::ProjectResultHashtags,
    pub communities_path: paths::Communities,
    pub logout_url: String,
    pub include_count: i64,
    pub exclude_count: i64,
    pub niveau: i64,
    pub last_login_datetime: NaiveDateTime,
    pub title: String,
    pub tweets_count: i64,
    pub authors_count: i64,
}

#[derive(Debug, Template)]
#[template(path = "csv_export.html")]
pub struct CsvExportTemplate {
    pub project_id: String,
    pub import_path: paths::ProjectImport,
    pub export_path: paths::ProjectCsvExport,
    pub collect_path: paths::ProjectCollect,
    pub is_analyzed: bool,
    pub daterange_path: paths::ProjectDateRange,
    pub hashtag_path: paths::ProjectHashtags,
    pub request_path: paths::ProjectRequest,
    pub delete_popup_path: paths::PopupDeleteProject,
    pub rename_popup_path: paths::PopupRenameProject,
    pub download_path: paths::DownloadProject,
    pub duplicate_popup_path: paths::PopupDuplicateProject,
    pub analysis_preview_popup_path: paths::PopupAnalysisPreview,
    pub analysis_path: paths::ProjectAnalysis,
    pub results_path: paths::ProjectResults,
    pub tweets_graph_path: paths::ProjectTweetsGraph,
    pub authors_path: paths::ProjectAuthors,
    pub result_hashtags_path: paths::ProjectResultHashtags,
    pub communities_path: paths::Communities,
    pub logout_url: String,
    pub include_count: i64,
    pub exclude_count: i64,
    pub niveau: i64,
    pub last_login_datetime: NaiveDateTime,
    pub title: String,
    pub tweets_count: i64,
    pub authors_count: i64,
}
