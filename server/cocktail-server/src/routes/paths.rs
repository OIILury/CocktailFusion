use axum_extra::routing::TypedPath;
use serde::Deserialize;
use uuid::Uuid;

#[derive(Debug, Deserialize, TypedPath)]
#[typed_path("/")]
pub struct Index;

#[derive(Debug, Deserialize, TypedPath)]
#[typed_path("/home")]
pub struct Home;

#[derive(Debug, Deserialize, TypedPath)]
#[typed_path("/projets")]
pub struct Projects;

#[derive(Debug, Deserialize, TypedPath)]
#[typed_path("/projets/nouveau")]
pub struct CreateProject;

#[derive(Debug, Deserialize, TypedPath)]
#[typed_path("/projets/:project_id/periode")]
pub struct ProjectDateRange {
  pub project_id: Uuid,
}

#[derive(Debug, Deserialize, TypedPath)]
#[typed_path("/projets/:project_id/mots-clefs")]
pub struct ProjectKeywords {
  pub project_id: Uuid,
}

#[derive(Debug, Deserialize, TypedPath)]
#[typed_path("/projets/:project_id/comptes-utilisateurs")]
pub struct ProjectAccounts {
  pub project_id: Uuid,
}

#[derive(Debug, Deserialize, TypedPath)]
#[typed_path("/projets/:project_id/requete")]
pub struct ProjectRequest {
  pub project_id: Uuid,
}

// #[derive(Debug, Deserialize, TypedPath)]
// #[typed_path("/projets/:project_id/basket")]
// pub struct ProjectBasket {
//     pub project_id: Uuid,
// }

#[derive(Debug, Deserialize, TypedPath)]
#[typed_path("/projets/:project_id/basket/include")]
pub struct ProjectBasketInclude {
  pub project_id: Uuid,
}

#[derive(Debug, Deserialize, TypedPath)]
#[typed_path("/projets/:project_id/basket/exclude")]
pub struct ProjectBasketExclude {
  pub project_id: Uuid,
}

#[derive(Debug, Deserialize, TypedPath)]
#[typed_path("/projets/:project_id/update/title")]
pub struct ProjectTitleUpdate {
  pub project_id: Uuid,
}

#[derive(Debug, Deserialize, TypedPath)]
#[typed_path("/projets/:project_id/hashtags")]
pub struct ProjectHashtags {
  pub project_id: Uuid,
}

#[derive(Debug, Deserialize, TypedPath)]
#[typed_path("/projets/:project_id/preview")]
pub struct PopupAnalysisPreview {
  pub project_id: Uuid,
}

#[derive(Debug, Deserialize, TypedPath)]
#[typed_path("/projets/:project_id/analyse")]
pub struct ProjectAnalysis {
  pub project_id: Uuid,
}

#[derive(Debug, Deserialize, TypedPath)]
#[typed_path("/projets/:project_id/resultat")]
pub struct ProjectResults {
  pub project_id: Uuid,
}

#[derive(Debug, Deserialize, TypedPath)]
#[typed_path("/projets/:project_id/resultat_hashtags")]
pub struct ProjectResultHashtags {
  pub project_id: Uuid,
}

#[derive(Debug, Deserialize, TypedPath)]
#[typed_path("/projets/:project_id/resultat_hashtags/:tab/:aside_hashtag_tab")]
pub struct ProjectResultHashtagsTab {
  pub project_id: Uuid,
  pub tab: String,
  pub aside_hashtag_tab: String,
}

#[derive(Debug, Deserialize, TypedPath)]
#[typed_path("/projets/:project_id/resultat_hashtags/action/toggle")]
pub struct ProjectHashtagToggle {
  pub project_id: Uuid,
}

#[derive(Debug, Deserialize, TypedPath)]
#[typed_path("/projets/:project_id/resultat_hashtags/:tab/:aside_hashtag_tab/action/toggle-all")]
pub struct ProjectAllToggle {
  pub project_id: Uuid,
  pub tab: String,
  pub aside_hashtag_tab: String,
}

#[derive(Debug, Deserialize, TypedPath)]
#[typed_path("/projets/:project_id/resultat_hashtags/:tab/:aside_hashtag_tab/action/aside")]
pub struct ProjectAsideHashtag {
  pub project_id: Uuid,
  pub tab: String,
  pub aside_hashtag_tab: String,
}

#[derive(Debug, Deserialize, TypedPath)]
#[typed_path("/projets/:project_id/resultat_cooccurences/action/toggle")]
pub struct ProjectCooccurenceToggle {
  pub project_id: Uuid,
}

#[derive(Debug, Deserialize, TypedPath)]
#[typed_path("/projets/:project_id/resultat/:tab/:aside_hashtag_tab")]
pub struct ProjectTweetsTab {
  pub project_id: Uuid,
  pub tab: String,
  pub aside_hashtag_tab: String,
}

#[derive(Debug, Deserialize, TypedPath)]
#[typed_path("/projets/:project_id/auteurs")]
pub struct ProjectAuthors {
  pub project_id: Uuid,
}

#[derive(Debug, Deserialize, TypedPath)]
#[typed_path("/projets/:project_id/auteurs/:tab")]
pub struct ProjectAuthorsTab {
  pub project_id: Uuid,
  pub tab: String,
}

#[derive(Debug, Deserialize, TypedPath)]
#[typed_path("/projets/:project_id/tweets")]
pub struct ProjectTweetsGraph {
  pub project_id: Uuid,
}

#[derive(Debug, Deserialize, TypedPath)]
#[typed_path("/projets/:project_id/tweets/:tab/:aside_tweet_tab")]
pub struct ProjectTweetsGraphTab {
  pub project_id: Uuid,
  pub tab: String,
  pub aside_tweet_tab: String,
}

#[derive(Debug, Deserialize, TypedPath)]
#[typed_path("/projets/:project_id/authors_select/:tab/")]
pub struct ProjectAuthorsSelect {
  pub project_id: Uuid,
  pub tab: String,
}

// reprendre les chemins !!
#[derive(Debug, Deserialize, TypedPath)]
#[typed_path("/projets/:project_id/hashtags/populaires")]
pub struct ProjectTopK {
  pub project_id: Uuid,
}

// #[derive(Debug, Deserialize, TypedPath)]
// #[typed_path("/projets/:project_id/hashtags/populaires")]
// pub struct ProjectTopK {
//     pub project_id: Uuid,
// }

#[derive(Debug, Deserialize, TypedPath)]
#[typed_path("/projets/:project_id/hashtags/popup")]
pub struct PopupHashtags {
  pub project_id: Uuid,
}

#[derive(Debug, Deserialize, TypedPath)]
#[typed_path("/projets/:project_id/keywords/popup")]
pub struct PopupKeywords {
  pub project_id: Uuid,
}

#[derive(Debug, Deserialize, TypedPath)]
#[typed_path("/projets/:project_id/accounts/popup")]
pub struct PopupAccounts {
  pub project_id: Uuid,
}

#[derive(Debug, Deserialize, TypedPath)]
#[typed_path("/projets/:project_id/delete")]
pub struct PopupDeleteProject {
  pub project_id: Uuid,
}

#[derive(Debug, Deserialize, TypedPath)]
#[typed_path("/projets/:project_id/rename")]
pub struct PopupRenameProject {
  pub project_id: Uuid,
}

#[derive(Debug, Deserialize, TypedPath)]
#[typed_path("/projets/:project_id/duplicate")]
pub struct PopupDuplicateProject {
  pub project_id: Uuid,
}

#[derive(Debug, Deserialize, TypedPath)]
#[typed_path("/projets/:project_id/download")]
pub struct DownloadProject {
  pub project_id: Uuid,
}

#[derive(Debug, Deserialize, TypedPath)]
#[typed_path("/projets/:project_id/hashtags/popup/corpus")]
pub struct PopupHashtagsCorpus {
  pub project_id: Uuid,
}

#[derive(Debug, Deserialize, TypedPath)]
#[typed_path("/projets/:project_id/hashtags/popup/topk")]
pub struct PopupHashtagsTopK {
  pub project_id: Uuid,
}

#[derive(Debug, Deserialize, TypedPath)]
#[typed_path("/projets/:project_id/hashtags/popup/search")]
pub struct PopupHashtagsSearch {
  pub project_id: Uuid,
}

#[derive(Debug, Deserialize, TypedPath)]
#[typed_path("/projets/:project_id/communautes")]
pub struct Communities {
  pub project_id: Uuid,
}

#[derive(Debug, Deserialize, TypedPath)]
#[typed_path(
  "/projets/:project_id/communautes/:tab/:community/:centrality/:max_rank/:show_interaction"
)]
pub struct CommunitiesTab {
  pub project_id: Uuid,
  pub tab: String,
  pub community: String,
  pub centrality: String,
  pub max_rank: i64,
  pub show_interaction: bool,
}

// AUTHENTICATION
#[derive(Debug, Deserialize, TypedPath)]
#[typed_path("/auth/registration")]
pub struct AuthRegistration;

#[derive(Debug, Deserialize, TypedPath)]
#[typed_path("/auth/login")]
pub struct AuthLogin;

#[derive(Debug, Deserialize, TypedPath)]
#[typed_path("/projets/:project_id/import")]
pub struct ProjectImport {
    pub project_id: Uuid,
}

#[derive(Debug, Deserialize, TypedPath)]
#[typed_path("/projets/:project_id/collect")]
pub struct ProjectCollect {
    pub project_id: Uuid,
}

#[derive(Debug, Deserialize, TypedPath)]
#[typed_path("/projets/:project_id/collect/start")]
pub struct StartCollection {
    pub project_id: Uuid,
}

#[derive(Debug, Deserialize, TypedPath)]
#[typed_path("/projets/:project_id/collect/delete")]
pub struct DeleteCollection {
    pub project_id: Uuid,
}

#[derive(Debug, Deserialize, TypedPath)]
#[typed_path("/projets/:project_id/collect/update")]
pub struct UpdateCollection {
    pub project_id: Uuid,
}
