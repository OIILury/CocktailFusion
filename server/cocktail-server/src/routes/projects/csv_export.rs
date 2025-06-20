use axum::{
    extract::State,
    response::IntoResponse,
    http::HeaderMap,
};
use ory_kratos_client::apis::configuration::Configuration;

use crate::{
    error::WebError,
    get_logout_url,
    models::{
        auth::AuthenticatedUser,
        templates::{HtmlTemplate, CsvExportTemplate},
    },
    routes::paths,
    AppState,
};

#[tracing::instrument]
pub async fn csv_export(
    paths::ProjectCsvExport { project_id }: paths::ProjectCsvExport,
    AuthenticatedUser {
        niveau,
        last_login_datetime,
        user_id,
    }: AuthenticatedUser,
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, WebError> {
    let logout_url = get_logout_url(state.kratos_configuration, headers).await;
    let project = cocktail_db_web::project(&state.db, project_id.to_hyphenated(), &user_id).await?;
    let (include_count, exclude_count) =
        cocktail_db_web::include_exclude_hashtag_count(&state.db, project_id.to_hyphenated(), &user_id)
            .await?;

    Ok(HtmlTemplate(CsvExportTemplate {
        project_id: project_id.to_string(),
        import_path: paths::ProjectImport { project_id },
        export_path: paths::ProjectCsvExport { project_id },
        collect_path: paths::ProjectCollect { project_id },
        is_analyzed: project.is_analyzed == 1,
        daterange_path: paths::ProjectDateRange { project_id },
        hashtag_path: paths::ProjectHashtags { project_id },
        request_path: paths::ProjectRequest { project_id },
        delete_popup_path: paths::PopupDeleteProject { project_id },
        rename_popup_path: paths::PopupRenameProject { project_id },
        clear_data_path: paths::ClearDataLatest { project_id },
        duplicate_popup_path: paths::PopupDuplicateProject { project_id },
        analysis_preview_popup_path: paths::PopupAnalysisPreview { project_id },
        analysis_path: paths::ProjectAnalysis { project_id },
        results_path: paths::ProjectResults { project_id },
        tweets_graph_path: paths::ProjectTweetsGraph { project_id },
        authors_path: paths::ProjectAuthors { project_id },
        result_hashtags_path: paths::ProjectResultHashtags { project_id },
        communities_path: paths::Communities { project_id },
        logout_url,
        include_count,
        exclude_count,
        niveau: niveau.into(),
        last_login_datetime,
        title: project.title,
        tweets_count: project.tweets_count,
        authors_count: project.authors_count,
    }))
} 