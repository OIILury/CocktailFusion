use crate::{
    error::WebError,
    models::templates::{HtmlTemplate, ImportTemplate},
    routes::paths::{
        ProjectImport, ProjectCollect, ProjectDateRange, ProjectHashtags, ProjectRequest,
        PopupDeleteProject, PopupRenameProject, DownloadProject, PopupDuplicateProject,
        PopupAnalysisPreview, ProjectAnalysis, ProjectResults, ProjectTweetsGraph, ProjectAuthors,
        ProjectResultHashtags, Communities,
    },
    AppState,
};
use axum::{extract::{Path, State}, response::IntoResponse};
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use uuid::Uuid;
use hyper::HeaderMap;
use ory_kratos_client::apis::configuration::Configuration;
use crate::models::auth::AuthenticatedUser;
use crate::get_logout_url;

#[tracing::instrument]
pub async fn import(
    Path(ProjectImport { project_id }): Path<ProjectImport>,
    AuthenticatedUser {
        niveau,
        last_login_datetime,
        user_id,
    }: AuthenticatedUser,
    headers: HeaderMap,
    State(state): State<AppState>,
    State(kratos_configuration): State<Configuration>,
) -> Result<impl IntoResponse, WebError> {
    let logout_url = get_logout_url(kratos_configuration, headers).await;
    let project = cocktail_db_web::project(&state.db, project_id.to_hyphenated(), &user_id).await?;
    let (include_count, exclude_count) = cocktail_db_web::include_exclude_hashtag_count(&state.db, project_id.to_hyphenated(), &user_id).await?;

    let template = ImportTemplate {
        project_id: project_id.to_string(),
        import_path: ProjectImport { project_id },
        collect_path: ProjectCollect { project_id },
        is_analyzed: project.is_analyzed == 1,
        daterange_path: ProjectDateRange { project_id },
        hashtag_path: ProjectHashtags { project_id },
        request_path: ProjectRequest { project_id },
        delete_popup_path: PopupDeleteProject { project_id },
        rename_popup_path: PopupRenameProject { project_id },
        download_path: DownloadProject { project_id },
        duplicate_popup_path: PopupDuplicateProject { project_id },
        analysis_preview_popup_path: PopupAnalysisPreview { project_id },
        analysis_path: ProjectAnalysis { project_id },
        results_path: ProjectResults { project_id },
        tweets_graph_path: ProjectTweetsGraph { project_id },
        authors_path: ProjectAuthors { project_id },
        result_hashtags_path: ProjectResultHashtags { project_id },
        communities_path: Communities { project_id },
        logout_url,
        include_count,
        exclude_count,
        niveau,
        last_login_datetime,
        title: project.title,
        tweets_count: project.tweets_count,
        authors_count: project.authors_count,
    };

    Ok(HtmlTemplate(template))
} 