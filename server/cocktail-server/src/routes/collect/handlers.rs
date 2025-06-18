use axum::{
    extract::{Json, State},
    response::IntoResponse,
    http::HeaderMap,
};
use chrono::Local;
use futures::future;

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
    
    // Optimisation majeure : traitement parallèle des mots-clés et réseaux
    let limit = req.limit.unwrap_or(10);
    
    // Créer toutes les tâches de collecte en parallèle
    let mut collection_tasks = Vec::new();
    
    for keyword in &req.keywords {
        for network in &req.networks {
            let keyword = keyword.clone();
            let network = network.clone();
            let schema_name = schema_name.clone();
            let start_date = req.start_date.clone();
            let end_date = req.end_date.clone();
            
            let task = tokio::spawn(async move {
                collect_network_keyword(network, keyword, limit, schema_name, start_date, end_date).await
            });
            
            collection_tasks.push(task);
        }
    }
    
    // Attendre toutes les tâches de collecte en parallèle
    tracing::info!("Starting {} parallel collection tasks", collection_tasks.len());
    let results = future::join_all(collection_tasks).await;
    
    // Compter les résultats
    for result in results {
        match result {
            Ok(count) => total_posts += count,
            Err(e) => tracing::error!("Collection task failed: {}", e),
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

// Fonction optimisée pour collecter un réseau/mot-clé en parallèle
async fn collect_network_keyword(
    network: String,
    keyword: String,
    limit: usize,
    schema_name: String,
    start_date: Option<String>,
    end_date: Option<String>
) -> usize {
    tracing::info!("Starting collection for network: {}, keyword: {}, limit: {}", network, keyword, limit);
    let collection_start = std::time::Instant::now();
    
    // Cloner keyword pour éviter le move
    let keyword_for_log = keyword.clone();
    
    let posts_collected = match network.as_str() {
        "bluesky" => {
            collect_bluesky_optimized(keyword, limit, schema_name, start_date, end_date).await
        },
        "twitter" => {
            collect_twitter_optimized(keyword, limit, schema_name, start_date, end_date).await
        },
        _ => {
            tracing::warn!("Unknown network: {}", network);
            0
        }
    };
    
    let collection_duration = collection_start.elapsed();
    tracing::info!("Collection completed for {}/{}: {} posts in {:?} ({:.2} posts/sec)", 
        network, keyword_for_log, posts_collected, collection_duration,
        posts_collected as f64 / collection_duration.as_secs_f64());
    
    posts_collected
}

// Collecte optimisée pour Bluesky
async fn collect_bluesky_optimized(
    keyword: String,
    limit: usize,
    schema_name: String,
    start_date: Option<String>,
    end_date: Option<String>
) -> usize {
    // Get Bluesky credentials from environment
    if let (Ok(handle), Ok(app_password)) = (
        std::env::var("BLUESKY_HANDLE"),
        std::env::var("BLUESKY_APP_PASSWORD")
    ) {
        if let Ok(collector) = BlueskyCollector::new(&handle, &app_password, schema_name).await {
            // Search for posts with date range
            if let Ok(posts) = collector.search_posts(&keyword, limit, start_date.as_deref(), end_date.as_deref()).await {
                tracing::info!("Found {} Bluesky posts for keyword: {}", posts.len(), keyword);
                
                // Utiliser la nouvelle méthode de traitement en batch optimisée
                match collector.save_posts_batch_to_db(&posts).await {
                    Ok(saved_count) => {
                        tracing::info!("Successfully saved {}/{} Bluesky posts for keyword: {}", saved_count, posts.len(), keyword);
                        saved_count
                    },
                    Err(e) => {
                        tracing::error!("Error saving Bluesky posts batch for keyword {}: {}", keyword, e);
                        0
                    }
                }
            } else {
                tracing::warn!("Failed to search Bluesky posts for keyword: {}", keyword);
                0
            }
        } else {
            tracing::warn!("Failed to create Bluesky collector for keyword: {}", keyword);
            0
        }
    } else {
        tracing::warn!("Bluesky credentials not set, skipping collection for keyword: {}", keyword);
        0
    }
}

// Collecte optimisée pour Twitter
async fn collect_twitter_optimized(
    keyword: String,
    limit: usize,
    schema_name: String,
    start_date: Option<String>,
    end_date: Option<String>
) -> usize {
    // Get Twitter credentials from environment
    if let Ok(bearer_token) = std::env::var("TWITTER_BEARER_TOKEN") {
        match TwitterCollector::new(&bearer_token, schema_name).await {
            Ok(collector) => {
                // Search for tweets with date range
                match collector.search_tweets(&keyword, limit, start_date.as_deref(), end_date.as_deref()).await {
                    Ok(search_response) => {
                        tracing::info!("Found {} Twitter tweets for keyword: {}", 
                            search_response.data.as_ref().map(|d| d.len()).unwrap_or(0), keyword);
                        
                        // Utiliser la nouvelle méthode de traitement en batch optimisée
                        if let Some(tweets) = &search_response.data {
                            match collector.save_tweets_batch_to_db(tweets, search_response.includes.as_ref()).await {
                                Ok(saved_count) => {
                                    tracing::info!("Successfully saved {}/{} Twitter tweets for keyword: {}", 
                                        saved_count, tweets.len(), keyword);
                                    saved_count
                                },
                                Err(e) => {
                                    tracing::error!("Error saving Twitter tweets batch for keyword {}: {}", keyword, e);
                                    0
                                }
                            }
                        } else {
                            0
                        }
                    },
                    Err(e) => {
                        tracing::warn!("Failed to search Twitter tweets for keyword '{}': {}", keyword, e);
                        0
                    }
                }
            },
            Err(e) => {
                tracing::warn!("Failed to create Twitter collector for keyword '{}': {}", keyword, e);
                0
            }
        }
    } else {
        tracing::warn!("Twitter Bearer Token not set, skipping collection for keyword: {}", keyword);
        0
    }
} 