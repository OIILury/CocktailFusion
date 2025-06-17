use axum::{
    extract::{Json, State},
    response::IntoResponse,
    http::HeaderMap,
};
use chrono::Local;

use crate::{
    error::WebError,
    models::{auth::AuthenticatedUser, templates::{self, HtmlTemplate}},
    routes::paths::{
        ProjectCollect, StartCollection, DeleteCollection, UpdateCollection,
        ProjectDateRange, ProjectHashtags, ProjectRequest, ProjectImport,
        PopupDeleteProject, PopupRenameProject, PopupDuplicateProject,
        DownloadProject, PopupAnalysisPreview, ProjectAnalysis,
        ProjectResults, ProjectTweetsGraph, ProjectAuthors,
        ProjectResultHashtags, Communities
    },
    routes::automation::run_automation_pipeline,
    get_logout_url,
    AppState,
};

use super::types::{CollectionRequest, CollectionResponse};
use super::database::create_collection_tables;
use super::bluesky::BlueskyCollector;
use super::twitter::TwitterCollector;

// Handler for the collect page - utilise maintenant le bon template
pub async fn collect(
    paths: ProjectCollect,
    authuser: AuthenticatedUser,
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, WebError> {
    let logout_url = get_logout_url(state.kratos_configuration, headers).await;
    let project = cocktail_db_web::project(&state.db, paths.project_id.to_hyphenated(), &authuser.user_id).await?;
    let (include_count, exclude_count) =
        cocktail_db_web::include_exclude_hashtag_count(&state.db, paths.project_id.to_hyphenated(), &authuser.user_id)
            .await?;
    
    Ok(HtmlTemplate(templates::Collect {
        daterange_path: ProjectDateRange { project_id: paths.project_id },
        hashtag_path: ProjectHashtags { project_id: paths.project_id },
        request_path: ProjectRequest { project_id: paths.project_id },
        collect_path: ProjectCollect { project_id: paths.project_id },
        import_path: ProjectImport { project_id: paths.project_id },
        delete_popup_path: PopupDeleteProject { project_id: paths.project_id },
        rename_popup_path: PopupRenameProject { project_id: paths.project_id },
        duplicate_popup_path: PopupDuplicateProject { project_id: paths.project_id },
        download_path: DownloadProject { project_id: paths.project_id },
        analysis_preview_popup_path: PopupAnalysisPreview { project_id: paths.project_id },
        analysis_path: ProjectAnalysis { project_id: paths.project_id },
        is_analyzed: project.is_analyzed == 1,
        results_path: ProjectResults { project_id: paths.project_id },
        tweets_graph_path: ProjectTweetsGraph { project_id: paths.project_id },
        authors_path: ProjectAuthors { project_id: paths.project_id },
        result_hashtags_path: ProjectResultHashtags { project_id: paths.project_id },
        communities_path: Communities { project_id: paths.project_id },
        logout_url,
        include_count,
        exclude_count,
        niveau: authuser.niveau,
        last_login_datetime: authuser.last_login_datetime,
        title: project.title,
        tweets_count: project.tweets_count,
        authors_count: project.authors_count,
    }))
}

// Handler for starting a collection
pub async fn start_collection(
    path: StartCollection,
    State(_state): State<AppState>,
    _authuser: AuthenticatedUser,
    Json(req): Json<CollectionRequest>,
) -> Result<impl IntoResponse, WebError> {
    tracing::info!("Starting collection with parameters: {:?}", req);
    
    // Vérification préliminaire pour Twitter API et dates
    if req.networks.contains(&"twitter".to_string()) {
        if let Some(start_date) = &req.start_date {
            if let Ok(start_dt) = chrono::DateTime::parse_from_rfc3339(start_date) {
                let seven_days_ago = chrono::Utc::now() - chrono::Duration::days(7);
                if start_dt.with_timezone(&chrono::Utc) < seven_days_ago {
                    // Vérifier si on a un token qui supporte l'API complète
                    if let Ok(bearer_token) = std::env::var("TWITTER_BEARER_TOKEN") {
                        if !bearer_token.starts_with("AAAA") {
                            return Err(WebError::WTFError(format!(
                                "ERREUR: Vous tentez de collecter des données depuis le {} mais votre token Twitter API ne semble pas supporter l'archive complète. L'API Twitter gratuite ne permet que les 7 derniers jours. Pour des données historiques, vous avez besoin d'un compte Twitter API Pro/Enterprise.",
                                start_date
                            )));
                        }
                        tracing::info!("Détection de dates historiques, tentative d'utilisation de l'API Twitter complète");
                    } else {
                        return Err(WebError::WTFError(
                            "Token Twitter API manquant dans les variables d'environnement".to_string()
                        ));
                    }
                }
            }
        }
    }
    
    // Create schema name based on current date
    let schema_name = format!("collect_{}", Local::now().format("%Y%m%d"));
    tracing::info!("Using schema name: {}", schema_name);
    
    // Get database connection
    let pool = sqlx::PgPool::connect(&std::env::var("PG_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://cocktailuser:cocktailuser@localhost:5432/cocktail_pg".to_string()))
        .await
        .map_err(|e| WebError::WTFError(format!("DB connection error: {}", e)))?;

    // Create tables for the schema
    create_collection_tables(&pool, &schema_name).await?;

    let mut total_posts = 0;
    
    // Process each keyword
    for keyword in &req.keywords {
        // Default limit to 10 if not specified
        let limit = req.limit.unwrap_or(10);
        
        // Process each selected network
        for network in &req.networks {
            match network.as_str() {
                "bluesky" => {
                    // Get Bluesky credentials from environment
                    if let (Ok(handle), Ok(app_password)) = (
                        std::env::var("BLUESKY_HANDLE"),
                        std::env::var("BLUESKY_APP_PASSWORD")
                    ) {
                        if let Ok(collector) = BlueskyCollector::new(&handle, &app_password, schema_name.clone()).await {
                            // Search for posts with date range
                            if let Ok(posts) = collector.search_posts(keyword, limit, req.start_date.as_deref(), req.end_date.as_deref()).await {
                                // Save posts to database
                                for post in &posts {
                                    if let Err(e) = collector.save_post_to_db(post).await {
                                        tracing::warn!("Error saving Bluesky post: {}", e);
                                    } else {
                                        total_posts += 1;
                                    }
                                }
                            } else {
                                tracing::warn!("Failed to search Bluesky posts for keyword: {}", keyword);
                            }
                        } else {
                            tracing::warn!("Failed to create Bluesky collector");
                        }
                    } else {
                        tracing::warn!("Bluesky credentials not set, skipping Bluesky collection");
                    }
                },
                "twitter" => {
                    // Get Twitter credentials from environment
                    if let Ok(bearer_token) = std::env::var("TWITTER_BEARER_TOKEN") {
                        match TwitterCollector::new(&bearer_token, schema_name.clone()).await {
                            Ok(collector) => {
                                // Search for tweets with date range
                                match collector.search_tweets(keyword, limit, req.start_date.as_deref(), req.end_date.as_deref()).await {
                                    Ok(search_response) => {
                                        tracing::info!("Found {} Twitter tweets for keyword: {}", search_response.data.as_ref().map(|d| d.len()).unwrap_or(0), keyword);
                                        
                                        // Traiter les tweets avec toutes les données includes
                                        if let Some(tweets) = &search_response.data {
                                            let save_start = std::time::Instant::now();
                                            let mut saved_count = 0;
                                            
                                            for (index, tweet) in tweets.iter().enumerate() {
                                                let tweet_save_start = std::time::Instant::now();
                                                if let Err(e) = collector.save_tweet_to_db(tweet, search_response.includes.as_ref()).await {
                                                    tracing::warn!("Error saving Twitter tweet {}: {}", tweet.id, e);
                                                } else {
                                                    saved_count += 1;
                                                    total_posts += 1;
                                                    
                                                    // Log de progression tous les 10 tweets
                                                    if index % 10 == 0 || index == tweets.len() - 1 {
                                                        let tweet_save_duration = tweet_save_start.elapsed();
                                                        tracing::info!("Tweet {}/{} sauvegardé en {:?} (total sauvegardé: {})", 
                                                            index + 1, tweets.len(), tweet_save_duration, saved_count);
                                                    }
                                                }
                                            }
                                            
                                            let save_duration = save_start.elapsed();
                                            tracing::info!("Sauvegarde de {} tweets terminée en {:?} (avg: {:?}/tweet)", 
                                                saved_count, save_duration, save_duration / saved_count.max(1) as u32);
                                        }
                                    },
                                    Err(e) => {
                                        tracing::warn!("Failed to search Twitter tweets for keyword '{}': {}", keyword, e);
                                    }
                                }
                            },
                            Err(e) => {
                                tracing::warn!("Failed to create Twitter collector: {}", e);
                            }
                        }
                    } else {
                        tracing::warn!("Twitter Bearer Token not set, skipping Twitter collection");
                    }
                },
                _ => {
                    tracing::warn!("Unknown network: {}", network);
                }
            }
        }
    }
    
    // Run automation pipeline if we have collected data
    if total_posts > 0 {
        tracing::info!("Starting automation pipeline for schema {}", schema_name);
        if let Err(e) = run_automation_pipeline(&schema_name, Some(path.project_id.to_string())).await {
            tracing::error!("Error during automation pipeline: {}", e);
            return Ok(Json(CollectionResponse {
                success: false,
                message: format!("Collection successful but automation failed: {}", e),
                count: total_posts,
            }));
        }
    }
    
    Ok(Json(CollectionResponse {
        success: true,
        message: format!("Successfully collected {} posts", total_posts),
        count: total_posts,
    }))
}

// Handler for deleting collected data
pub async fn delete_collection(
    _path: DeleteCollection,
    State(_state): State<AppState>,
    _authuser: AuthenticatedUser,
) -> Result<impl IntoResponse, WebError> {
    // Get database connection
    let pool = sqlx::PgPool::connect(&std::env::var("PG_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://cocktailuser:cocktailuser@localhost:5432/cocktail_pg".to_string()))
        .await
        .map_err(|e| WebError::WTFError(format!("DB connection error: {}", e)))?;

    // Get all collection schemas (collect_YYYYMMDD pattern)
    let schema_query = "SELECT schema_name FROM information_schema.schemata WHERE schema_name LIKE 'collect_%'";
    let collections = sqlx::query_scalar::<_, String>(schema_query)
        .fetch_all(&pool)
        .await
        .map_err(|e| WebError::WTFError(format!("Failed to get collection schemas: {}", e)))?;

    // Drop each collection schema
    for schema_name in &collections {
        sqlx::query(&format!("DROP SCHEMA IF EXISTS {} CASCADE", schema_name))
            .execute(&pool)
            .await
            .map_err(|e| WebError::WTFError(format!("Failed to drop schema {}: {}", schema_name, e)))?;
    }

    Ok(Json(CollectionResponse {
        success: true,
        message: "Successfully deleted all collected data".to_string(),
        count: 0,
    }))
}

pub async fn update_collection(
    path: UpdateCollection,
    State(_state): State<AppState>,
    _authuser: AuthenticatedUser,
) -> Result<impl IntoResponse, WebError> {
    // Get the most recent collection schema (collect_YYYYMMDD pattern)
    let pool = sqlx::PgPool::connect(&std::env::var("PG_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://cocktailuser:cocktailuser@localhost:5432/cocktail_pg".to_string()))
        .await
        .map_err(|e| WebError::WTFError(format!("DB connection error: {}", e)))?;

    let schema_query = "SELECT schema_name FROM information_schema.schemata WHERE schema_name LIKE 'collect_%' ORDER BY schema_name DESC LIMIT 1";
    let schema_name = sqlx::query_scalar::<_, String>(schema_query)
        .fetch_optional(&pool)
        .await
        .map_err(|e| WebError::WTFError(format!("Failed to get collection schema: {}", e)))?
        .unwrap_or_else(|| format!("collect_{}", Local::now().format("%Y%m%d")));

    tracing::info!("Starting automation pipeline for schema {} and project {}", schema_name, path.project_id);
    
    // Use the automation pipeline to update data
    if let Err(e) = run_automation_pipeline(&schema_name, Some(path.project_id.to_string())).await {
        tracing::error!("Error during automation pipeline: {}", e);
        return Err(WebError::WTFError(format!("Failed to update data: {}", e)));
    }
    
    Ok(Json(CollectionResponse {
        success: true,
        message: "Successfully updated all data using automation pipeline".to_string(),
        count: 0,
    }))
} 