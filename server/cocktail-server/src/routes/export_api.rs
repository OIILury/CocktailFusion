use axum::{
    extract::{Path, State},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use tokio::sync::RwLock;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use sqlx::Row;
use lazy_static::lazy_static;
use crate::{
    error::WebError,
    models::auth::AuthenticatedUser,
    AppState,
};

// Structures pour les requêtes et réponses
#[derive(Debug, Deserialize, Clone)]
pub struct EstimateRequest {
    pub project_id: String,
    pub columns: Vec<String>,
    pub filters: ExportFilters,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ExportRequest {
    pub project_id: String,
    pub columns: Vec<String>,
    pub filters: ExportFilters,
    pub format: String,
    pub include_headers: bool,
    pub utf8_bom: bool,
    pub custom_filename: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ExportFilters {
    pub date_filter_type: String,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub date_ranges: Option<Vec<DateRange>>,
    pub min_likes: Option<i64>,
    pub max_likes: Option<i64>,
    pub min_retweets: Option<i64>,
    pub max_retweets: Option<i64>,
    pub min_quotes: Option<i64>,
    pub max_quotes: Option<i64>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DateRange {
    pub start: String,
    pub end: String,
}

#[derive(Debug, Serialize)]
pub struct EstimateResponse {
    pub tweet_count: u64,
    pub file_size: u64,
    pub estimated_duration: u64, // en secondes
}

#[derive(Debug, Serialize)]
pub struct ExportStartResponse {
    pub export_id: String,
    pub estimated_duration: u64,
}

#[derive(Debug, Serialize)]
pub struct ProgressResponse {
    pub export_id: String,
    pub status: String,
    pub percentage: f64,
    pub status_message: String,
    pub processed_tweets: u64,
    pub total_tweets: u64,
    pub error: Option<String>,
    pub filename: Option<String>,
    pub file_size: Option<u64>,
}

// Structure pour le suivi des exports en cours
#[derive(Debug, Clone)]
pub struct ExportJob {
    pub id: String,
    pub user_id: String,
    pub project_id: String,
    pub status: String,
    pub progress: f64,
    pub total_tweets: u64,
    pub processed_tweets: u64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub filename: Option<String>,
    pub file_size: Option<u64>,
    pub error: Option<String>,
    pub request: ExportRequest,
}

// Store global pour les exports en cours
lazy_static::lazy_static! {
    static ref EXPORT_JOBS: RwLock<HashMap<String, ExportJob>> = RwLock::new(HashMap::new());
}

/// Fonction utilitaire pour trouver un schéma contenant des données pour le projet
async fn find_data_schema(pg_pool: &sqlx::PgPool, project_id: &str) -> Result<String, WebError> {
    // 1. D'abord, essayer le schéma data_latest (nouveau comportement)
    let data_latest_exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM information_schema.schemata WHERE schema_name = 'data_latest')"
    )
    .fetch_one(pg_pool)
    .await
    .map_err(|e| {
        tracing::error!("Erreur lors de la vérification du schéma data_latest: {}", e);
        WebError::WTFError(format!("Erreur vérification schéma: {}", e))
    })?;

    if data_latest_exists {
        // Vérifier s'il y a des données dans le schéma data_latest
        let has_data = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM data_latest.tweet LIMIT 1"
        )
        .fetch_one(pg_pool)
        .await
        .unwrap_or(0);

        if has_data > 0 {
            tracing::info!("Utilisation du schéma data_latest pour l'export");
            return Ok("data_latest".to_string());
        }
    }

    // 2. Si data_latest n'existe pas ou est vide, aucune donnée disponible
    tracing::warn!("Schéma data_latest vide ou inexistant pour l'export");
    
    Err(WebError::WTFError(
        "Aucune donnée disponible dans data_latest. Veuillez d'abord collecter ou importer des données.".to_string()
    ))
}

/// Estimer le nombre de tweets et la taille du fichier
#[tracing::instrument]
pub async fn estimate_export(
    State(state): State<AppState>,
    AuthenticatedUser { user_id, .. }: AuthenticatedUser,
    Json(request): Json<EstimateRequest>,
) -> Result<impl IntoResponse, WebError> {
    tracing::info!("Estimation d'export pour le projet {} par l'utilisateur {}", request.project_id, user_id);

    // Utiliser le pool PostgreSQL partagé de l'AppState
    let pg_pool = &state.pg_pool;

    // Trouver le schéma contenant les données (projet ou import)
    let data_schema = find_data_schema(pg_pool, &request.project_id).await?;

    // Construire la requête SQL pour estimer le nombre de tweets
    let mut query = format!("SELECT COUNT(*) as count FROM \"{}\".tweet WHERE 1=1", data_schema);
    let mut bind_values: Vec<String> = Vec::new();

    // Ajouter les filtres de date
    match request.filters.date_filter_type.as_str() {
        "single_range" => {
            if let (Some(start), Some(end)) = (&request.filters.start_date, &request.filters.end_date) {
                query.push_str(" AND created_at >= $1 AND created_at <= $2");
                bind_values.push(start.clone());
                bind_values.push(end.clone());
            }
        }
        "multiple_ranges" => {
            if let Some(ranges) = &request.filters.date_ranges {
                if !ranges.is_empty() {
                    let mut range_conditions = Vec::new();
                    for (i, range) in ranges.iter().enumerate() {
                        let start_param = bind_values.len() + 1;
                        let end_param = bind_values.len() + 2;
                        range_conditions.push(format!("(created_at >= ${} AND created_at <= ${})", start_param, end_param));
                        bind_values.push(range.start.clone());
                        bind_values.push(range.end.clone());
                    }
                    query.push_str(&format!(" AND ({})", range_conditions.join(" OR ")));
                }
            }
        }
        _ => {} // "all" - pas de filtre de date
    }

    // Ajouter les filtres de popularité
    if let Some(min_retweets) = request.filters.min_retweets {
        let param_index = bind_values.len() + 1;
        query.push_str(&format!(" AND retweet_count >= ${}", param_index));
        bind_values.push(min_retweets.to_string());
    }
    if let Some(max_retweets) = request.filters.max_retweets {
        let param_index = bind_values.len() + 1;
        query.push_str(&format!(" AND retweet_count <= ${}", param_index));
        bind_values.push(max_retweets.to_string());
    }
    if let Some(min_quotes) = request.filters.min_quotes {
        let param_index = bind_values.len() + 1;
        query.push_str(&format!(" AND quote_count >= ${}", param_index));
        bind_values.push(min_quotes.to_string());
    }
    if let Some(max_quotes) = request.filters.max_quotes {
        let param_index = bind_values.len() + 1;
        query.push_str(&format!(" AND quote_count <= ${}", param_index));
        bind_values.push(max_quotes.to_string());
    }

    // Exécuter la requête d'estimation
    tracing::debug!("Requête d'estimation: {}", query);
    
    let mut query_builder = sqlx::query_scalar::<_, i64>(&query);
    for value in &bind_values {
        query_builder = query_builder.bind(value);
    }
    
    let tweet_count: u64 = match query_builder.fetch_one(pg_pool).await {
        Ok(count) => count as u64,
        Err(e) => {
            tracing::warn!("Erreur lors de l'estimation: {}", e);
            0
        }
    };

    // Estimer la taille du fichier de manière plus réaliste
    let base_tweet_size = match request.columns.len() {
        1..=3 => 50,   // Colonnes de base (id, date, etc.)
        4..=8 => 150,  // Avec texte et quelques métadonnées
        9..=15 => 250, // Avec toutes les colonnes
        _ => 300,      // Sécurité
    };
    
    // Ajouter les en-têtes et la structure CSV
    let headers_size = request.columns.len() * 20; // Approximation des en-têtes
    let csv_overhead = 50; // Virgules, guillemets, retours à la ligne
    
    let file_size = (tweet_count as f64 * (base_tweet_size + csv_overhead) as f64) as u64 + headers_size as u64;

    // Estimer la durée (approximation plus réaliste)
    let tweets_per_second = 500; // tweets traités par seconde (plus conservateur)
    let estimated_duration = (tweet_count / tweets_per_second).max(1);

    Ok(Json(EstimateResponse {
        tweet_count,
        file_size,
        estimated_duration,
    }))
}

/// Démarrer un export
#[tracing::instrument]
pub async fn start_export(
    State(state): State<AppState>,
    AuthenticatedUser { user_id, .. }: AuthenticatedUser,
    Json(request): Json<ExportRequest>,
) -> Result<impl IntoResponse, WebError> {
    let export_id = Uuid::new_v4().to_string();
    
    tracing::info!("Démarrage de l'export {} pour le projet {} par l'utilisateur {}", export_id, request.project_id, user_id);

    // Créer le job d'export
    let job = ExportJob {
        id: export_id.clone(),
        user_id: user_id.clone(),
        project_id: request.project_id.clone(),
        status: "starting".to_string(),
        progress: 0.0,
        total_tweets: 0,
        processed_tweets: 0,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        filename: None,
        file_size: None,
        error: None,
        request: request.clone(),
    };

    // Ajouter le job au store
    {
        let mut jobs = EXPORT_JOBS.write().await;
        jobs.insert(export_id.clone(), job);
    }

    // Démarrer le processus d'export en arrière-plan
    let state_clone = state.clone();
    let export_id_clone = export_id.clone();
    tokio::spawn(async move {
        process_export(export_id_clone, state_clone).await;
    });

    Ok(Json(ExportStartResponse {
        export_id,
        estimated_duration: 60, // estimation par défaut
    }))
}

/// Obtenir le progrès d'un export
#[tracing::instrument]
pub async fn get_export_progress(
    Path(export_id): Path<String>,
    AuthenticatedUser { user_id, .. }: AuthenticatedUser,
) -> Result<impl IntoResponse, WebError> {
    let jobs = EXPORT_JOBS.read().await;
    
    if let Some(job) = jobs.get(&export_id) {
        if job.user_id != user_id {
            return Err(WebError::Forbidden("Accès non autorisé à cet export".to_string()));
        }

        Ok(Json(ProgressResponse {
            export_id: job.id.clone(),
            status: job.status.clone(),
            percentage: job.progress,
            status_message: get_status_message(&job.status, job.progress),
            processed_tweets: job.processed_tweets,
            total_tweets: job.total_tweets,
            error: job.error.clone(),
            filename: job.filename.clone(),
            file_size: job.file_size,
        }))
    } else {
        Err(WebError::WTFError("Export non trouvé".to_string()))
    }
}

/// Annuler un export
#[tracing::instrument]
pub async fn cancel_export(
    Path(export_id): Path<String>,
    AuthenticatedUser { user_id, .. }: AuthenticatedUser,
) -> Result<impl IntoResponse, WebError> {
    let mut jobs = EXPORT_JOBS.write().await;
    
    if let Some(job) = jobs.get_mut(&export_id) {
        if job.user_id != user_id {
            return Err(WebError::Forbidden("Accès non autorisé à cet export".to_string()));
        }

        job.status = "cancelled".to_string();
        job.updated_at = Utc::now();
        
        tracing::info!("Export {} annulé par l'utilisateur {}", export_id, user_id);
        
        Ok(Json(serde_json::json!({"status": "cancelled"})))
    } else {
        Err(WebError::WTFError("Export non trouvé".to_string()))
    }
}

/// Télécharger le fichier d'export
#[tracing::instrument]
pub async fn download_export(
    Path(export_id): Path<String>,
    AuthenticatedUser { user_id, .. }: AuthenticatedUser,
    State(state): State<AppState>,
) -> Result<Response<axum::body::Body>, WebError> {
    use axum::http::header;
    
    let jobs = EXPORT_JOBS.read().await;
    
    if let Some(job) = jobs.get(&export_id) {
        if job.user_id != user_id {
            return Err(WebError::Forbidden("Accès non autorisé à cet export".to_string()));
        }

        if job.status != "completed" {
            return Err(WebError::BadRequest("Export non terminé".to_string()));
        }

        let filename = job.filename.as_ref()
            .ok_or_else(|| WebError::InternalServerError("Nom de fichier manquant".to_string()))?;

        // Déterminer le séparateur selon le format
        let separator = match job.request.format.as_str() {
            "tsv" => "\t",
            _ => ",",
        };
        
        // Générer le contenu de démonstration
        let mut file_content = String::new();
        
        // Ajouter les en-têtes si demandé
        if job.request.include_headers {
            // Utiliser les vraies colonnes sélectionnées pour les en-têtes
            let header_names: Vec<String> = job.request.columns.iter().map(|col| {
                match col.as_str() {
                    "tweet.id" => "id".to_string(),
                    "tweet.created_at" => "created_at".to_string(),
                    "tweet.published_time" => "published_time".to_string(),
                    "tweet.user_id" => "user_id".to_string(),
                    "tweet.user_name" => "user_name".to_string(),
                    "tweet.user_screen_name" => "user_screen_name".to_string(),
                    "tweet.text" => "text".to_string(),
                    "tweet.source" => "source".to_string(),
                    "tweet.language" => "language".to_string(),
                    "tweet.coordinates_longitude" => "coordinates_longitude".to_string(),
                    "tweet.coordinates_latitude" => "coordinates_latitude".to_string(),
                    "tweet.possibly_sensitive" => "possibly_sensitive".to_string(),
                    "tweet.retweet_count" => "retweet_count".to_string(),
                    "tweet.reply_count" => "reply_count".to_string(),
                    "tweet.quote_count" => "quote_count".to_string(),
                    "hashtag.tweet_id" => "hashtag_tweet_id".to_string(),
                    "hashtag.hashtag" => "hashtag".to_string(),
                    "url.tweet_id" => "url_tweet_id".to_string(),
                    "url.url" => "url".to_string(),
                    "retweet.retweeted_tweet_id" => "retweeted_tweet_id".to_string(),
                    "reply.in_reply_to_tweet_id" => "in_reply_to_tweet_id".to_string(),
                    "quote.quoted_tweet_id" => "quoted_tweet_id".to_string(),
                    _ => col.split('.').last().unwrap_or(col).to_string(),
                }
            }).collect();
            
            file_content.push_str(&header_names.join(separator));
            file_content.push('\n');
        }
        
        // Utiliser le pool PostgreSQL partagé de l'AppState
        let pg_pool = &state.pg_pool;

        // Construire la requête SQL directe pour une seule table d'abord (plus simple)
        let mut query = format!("SELECT ");
        let mut column_selects = Vec::new();
        
        for column in &job.request.columns {
            match column.as_str() {
                // Colonnes de la table tweet
                "tweet.id" => column_selects.push("id".to_string()),
                "tweet.created_at" => column_selects.push("created_at".to_string()),
                "tweet.published_time" => column_selects.push("published_time".to_string()),
                "tweet.user_id" => column_selects.push("user_id".to_string()),
                "tweet.user_name" => column_selects.push("user_name".to_string()),
                "tweet.user_screen_name" => column_selects.push("user_screen_name".to_string()),
                "tweet.text" => column_selects.push("text".to_string()),
                "tweet.source" => column_selects.push("source".to_string()),
                "tweet.language" => column_selects.push("language".to_string()),
                "tweet.coordinates_longitude" => column_selects.push("coordinates_longitude".to_string()),
                "tweet.coordinates_latitude" => column_selects.push("coordinates_latitude".to_string()),
                "tweet.possibly_sensitive" => column_selects.push("possibly_sensitive".to_string()),
                "tweet.retweet_count" => column_selects.push("retweet_count".to_string()),
                "tweet.reply_count" => column_selects.push("reply_count".to_string()),
                "tweet.quote_count" => column_selects.push("quote_count".to_string()),
                _ => {
                    tracing::warn!("Colonne non supportée pour l'instant: {}", column);
                    // Pour l'instant, on ignore les colonnes des autres tables
                }
            }
        }
        
        if column_selects.is_empty() {
            column_selects.push("*".to_string());
        }
        
        query.push_str(&column_selects.join(", "));
        
        // Trouver le schéma contenant les données (projet ou import)
        let data_schema = match find_data_schema(pg_pool, &job.project_id).await {
            Ok(schema) => schema,
            Err(e) => {
                tracing::error!("Impossible de trouver les données pour le téléchargement: {}", e);
                return Err(e);
            }
        };
        
        query.push_str(&format!(" FROM \"{}\".tweet WHERE 1=1", data_schema));
        
        let mut bind_values: Vec<String> = Vec::new();
        
        // Ajouter les filtres de date
        match job.request.filters.date_filter_type.as_str() {
            "single_range" => {
                if let (Some(start), Some(end)) = (&job.request.filters.start_date, &job.request.filters.end_date) {
                    query.push_str(" AND created_at >= $1 AND created_at <= $2");
                    bind_values.push(start.clone());
                    bind_values.push(end.clone());
                }
            }
            "multiple_ranges" => {
                if let Some(ranges) = &job.request.filters.date_ranges {
                    if !ranges.is_empty() {
                        let mut range_conditions = Vec::new();
                        for range in ranges.iter() {
                            let start_param = bind_values.len() + 1;
                            let end_param = bind_values.len() + 2;
                            range_conditions.push(format!("(created_at >= ${} AND created_at <= ${})", start_param, end_param));
                            bind_values.push(range.start.clone());
                            bind_values.push(range.end.clone());
                        }
                        query.push_str(&format!(" AND ({})", range_conditions.join(" OR ")));
                    }
                }
            }
            _ => {} // "all" - pas de filtre de date
        }

        // Ajouter les filtres de popularité
        if let Some(min_retweets) = job.request.filters.min_retweets {
            let param_index = bind_values.len() + 1;
            query.push_str(&format!(" AND retweet_count >= ${}", param_index));
            bind_values.push(min_retweets.to_string());
        }
        if let Some(max_retweets) = job.request.filters.max_retweets {
            let param_index = bind_values.len() + 1;
            query.push_str(&format!(" AND retweet_count <= ${}", param_index));
            bind_values.push(max_retweets.to_string());
        }
        if let Some(min_quotes) = job.request.filters.min_quotes {
            let param_index = bind_values.len() + 1;
            query.push_str(&format!(" AND quote_count >= ${}", param_index));
            bind_values.push(min_quotes.to_string());
        }
        if let Some(max_quotes) = job.request.filters.max_quotes {
            let param_index = bind_values.len() + 1;
            query.push_str(&format!(" AND quote_count <= ${}", param_index));
            bind_values.push(max_quotes.to_string());
        }

        // Limiter le nombre de résultats pour éviter les problèmes de mémoire
        query.push_str(" LIMIT 10000");

        tracing::info!("Requête d'export de téléchargement: {}", query);

        // Construire et exécuter la requête
        let mut query_builder = sqlx::query(&query);
        for value in &bind_values {
            query_builder = query_builder.bind(value);
        }

        // Récupérer les vraies données de la base PostgreSQL
        let rows = match query_builder.fetch_all(pg_pool).await {
            Ok(rows) => rows,
            Err(e) => {
                tracing::error!("Erreur lors de la récupération des données: {}", e);
                return Err(WebError::WTFError(format!("Impossible de récupérer les données: {}", e)));
            }
        };

        tracing::info!("Nombre de lignes récupérées: {}", rows.len());

        for (i, row) in rows.iter().enumerate() {
            let mut row_data = Vec::new();
            
            // Extraire les données selon les colonnes demandées (en utilisant les noms des colonnes dans la requête)
            for (col_index, column) in job.request.columns.iter().enumerate() {
                let column_name = match column.as_str() {
                    "tweet.id" => "id",
                    "tweet.created_at" => "created_at",
                    "tweet.published_time" => "published_time",
                    "tweet.user_id" => "user_id",
                    "tweet.user_name" => "user_name",
                    "tweet.user_screen_name" => "user_screen_name",
                    "tweet.text" => "text",
                    "tweet.source" => "source",
                    "tweet.language" => "language",
                    "tweet.coordinates_longitude" => "coordinates_longitude",
                    "tweet.coordinates_latitude" => "coordinates_latitude",
                    "tweet.possibly_sensitive" => "possibly_sensitive",
                    "tweet.retweet_count" => "retweet_count",
                    "tweet.reply_count" => "reply_count",
                    "tweet.quote_count" => "quote_count",
                    _ => continue, // Skip unsupported columns
                };
                
                let value = match row.try_get::<Option<String>, _>(column_name) {
                    Ok(Some(v)) => v,
                    Ok(None) => "".to_string(),
                    Err(_) => {
                        // Essayer d'autres types de données
                        if let Ok(v) = row.try_get::<Option<i64>, _>(column_name) {
                            v.map_or("".to_string(), |val| val.to_string())
                        } else if let Ok(v) = row.try_get::<Option<bool>, _>(column_name) {
                            v.map_or("".to_string(), |val| val.to_string())
                        } else if let Ok(v) = row.try_get::<Option<i32>, _>(column_name) {
                            v.map_or("".to_string(), |val| val.to_string())
                        } else {
                            "".to_string()
                        }
                    }
                };
                
                // Échapper les valeurs qui contiennent le séparateur ou des guillemets
                let escaped_value = if value.contains(separator) || value.contains('"') || value.contains('\n') {
                    format!("\"{}\"", value.replace('"', "\"\"").replace('\n', " ").replace('\r', ""))
                } else {
                    value
                };
                
                row_data.push(escaped_value);
            }
            
            file_content.push_str(&row_data.join(separator));
            file_content.push('\n');
            
            // Mettre à jour le progrès
            if i % 100 == 0 && i > 0 {
                tracing::debug!("Traitement ligne {} / {}", i + 1, rows.len());
            }
        }

        tracing::info!("Export terminé: {} lignes traitées", rows.len());

        // Ajouter BOM UTF-8 si demandé
        let content = if job.request.utf8_bom {
            format!("\u{FEFF}{}", file_content)
        } else {
            file_content
        };

        // Déterminer le type MIME
        let content_type = match job.request.format.as_str() {
            "tsv" => "text/tab-separated-values",
            "excel" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            _ => "text/csv",
        };

        use axum::body::Body;

        let response = Response::builder()
            .header(header::CONTENT_TYPE, content_type)
            .header(header::CONTENT_DISPOSITION, format!("attachment; filename=\"{}\"", filename))
            .header(header::CACHE_CONTROL, "no-cache")
            .body(Body::from(content))
            .map_err(|e| WebError::WTFError(format!("Erreur création réponse: {}", e)))?;

        Ok(response)
    } else {
        Err(WebError::WTFError("Export non trouvé".to_string()))
    }
}

/// Processus d'export en arrière-plan
async fn process_export(export_id: String, state: AppState) {
    let mut job = {
        let jobs = EXPORT_JOBS.read().await;
        match jobs.get(&export_id) {
            Some(job) => job.clone(),
            None => return,
        }
    };

    // Mettre à jour le statut
    job.status = "processing".to_string();
    job.updated_at = Utc::now();
    update_job(&export_id, &job).await;

    // Simuler le processus d'export
    match perform_export(&mut job, &state).await {
        Ok(_) => {
            job.status = "completed".to_string();
            job.progress = 100.0;
            job.updated_at = Utc::now();
            update_job(&export_id, &job).await;
            
            tracing::info!("Export {} terminé avec succès", export_id);
        }
        Err(e) => {
            job.status = "failed".to_string();
            job.error = Some(e.to_string());
            job.updated_at = Utc::now();
            update_job(&export_id, &job).await;
            
            tracing::error!("Erreur lors de l'export {}: {}", export_id, e);
        }
    }
}

async fn perform_export(job: &mut ExportJob, state: &AppState) -> Result<(), WebError> {
    // Utiliser le pool PostgreSQL partagé de l'AppState
    let pg_pool = &state.pg_pool;

    // Trouver le schéma contenant les données (projet ou import)
    let data_schema = match find_data_schema(pg_pool, &job.project_id).await {
        Ok(schema) => schema,
        Err(e) => {
            tracing::error!("Impossible de trouver les données pour le projet {}: {}", job.project_id, e);
            return Err(e);
        }
    };

    tracing::info!("Utilisation du schéma {} pour l'export du projet {}", data_schema, job.project_id);

    // Construire la requête de comptage
    let mut count_query = format!("SELECT COUNT(*) FROM \"{}\".tweet WHERE 1=1", data_schema);
    let mut bind_values: Vec<String> = Vec::new();

    // Ajouter les filtres de date au comptage
    match job.request.filters.date_filter_type.as_str() {
        "single_range" => {
            if let (Some(start), Some(end)) = (&job.request.filters.start_date, &job.request.filters.end_date) {
                count_query.push_str(" AND created_at >= $1 AND created_at <= $2");
                bind_values.push(start.clone());
                bind_values.push(end.clone());
            }
        }
        "multiple_ranges" => {
            if let Some(ranges) = &job.request.filters.date_ranges {
                if !ranges.is_empty() {
                    let mut range_conditions = Vec::new();
                    for range in ranges.iter() {
                        let start_param = bind_values.len() + 1;
                        let end_param = bind_values.len() + 2;
                        range_conditions.push(format!("(created_at >= ${} AND created_at <= ${})", start_param, end_param));
                        bind_values.push(range.start.clone());
                        bind_values.push(range.end.clone());
                    }
                    count_query.push_str(&format!(" AND ({})", range_conditions.join(" OR ")));
                }
            }
        }
        _ => {} // "all" - pas de filtre de date
    }

    // Ajouter les filtres de popularité au comptage
    if let Some(min_retweets) = job.request.filters.min_retweets {
        let param_index = bind_values.len() + 1;
        count_query.push_str(&format!(" AND retweet_count >= ${}", param_index));
        bind_values.push(min_retweets.to_string());
    }
    if let Some(max_retweets) = job.request.filters.max_retweets {
        let param_index = bind_values.len() + 1;
        count_query.push_str(&format!(" AND retweet_count <= ${}", param_index));
        bind_values.push(max_retweets.to_string());
    }
    if let Some(min_quotes) = job.request.filters.min_quotes {
        let param_index = bind_values.len() + 1;
        count_query.push_str(&format!(" AND quote_count >= ${}", param_index));
        bind_values.push(min_quotes.to_string());
    }
    if let Some(max_quotes) = job.request.filters.max_quotes {
        let param_index = bind_values.len() + 1;
        count_query.push_str(&format!(" AND quote_count <= ${}", param_index));
        bind_values.push(max_quotes.to_string());
    }

    // Exécuter le comptage
    let mut count_query_builder = sqlx::query_scalar::<_, i64>(&count_query);
    for value in &bind_values {
        count_query_builder = count_query_builder.bind(value);
    }
    
    let total_tweets: u64 = match count_query_builder.fetch_one(pg_pool).await {
        Ok(count) => count as u64,
        Err(e) => {
            tracing::error!("Erreur lors du comptage des tweets: {}", e);
            return Err(WebError::WTFError(format!("Impossible de compter les tweets: {}", e)));
        }
    };

    job.total_tweets = total_tweets;
    update_job(&job.id, job).await;

    // Les données sont traitées en une seule fois
    
    // Générer le nom de fichier
    let filename = if let Some(custom_name) = &job.request.custom_filename {
        if custom_name.trim().is_empty() {
            format!("export_{}_{}.{}", 
                job.project_id, 
                Utc::now().format("%Y%m%d_%H%M%S"), 
                job.request.format
            )
        } else {
            // Nettoyer le nom personnalisé (supprimer les caractères non autorisés)
            let clean_name = custom_name
                .chars()
                .map(|c| if c.is_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
                .collect::<String>();
            format!("{}.{}", clean_name, job.request.format)
        }
    } else {
        format!("export_{}_{}.{}", 
            job.project_id, 
            Utc::now().format("%Y%m%d_%H%M%S"), 
            job.request.format
        )
    };
    
    // Simuler la création du fichier d'export
    let export_path = format!("/tmp/exports/{}", filename);
    
    // Simuler l'écriture des données - pas de création réelle de fichier pour l'instant
    tracing::info!("Simulation d'export vers {}", export_path);

    // Le traitement est maintenant fait directement dans la génération du fichier
    // Les données sont récupérées en une seule fois et traitées

    // Calculer la taille réelle du fichier basée sur le contenu généré
    let estimated_content_length = if job.request.include_headers {
        job.request.columns.join(",").len() + 1 // +1 pour le \n
    } else {
        0
    };
    
    let estimated_row_size = job.request.columns.iter().map(|col| {
        match col.as_str() {
            "tweet.id" => 10,        // "tweet_123"
            "tweet.text" => 40,      // Texte du tweet
            "tweet.created_at" => 25, // Date ISO
            "tweet.user_name" => 15,  // Nom utilisateur
            "tweet.retweet_count" | "tweet.reply_count" | "tweet.quote_count" => 3, // Nombres
            _ => 10,                  // Autres colonnes
        }
    }).sum::<usize>() + job.request.columns.len(); // +1 pour chaque virgule

    let file_size = (estimated_content_length + (estimated_row_size * job.total_tweets as usize)) as u64;

    job.filename = Some(filename);
    job.file_size = Some(file_size);

    Ok(())
}

async fn update_job(export_id: &str, job: &ExportJob) {
    let mut jobs = EXPORT_JOBS.write().await;
    jobs.insert(export_id.to_string(), job.clone());
}

fn get_status_message(status: &str, progress: f64) -> String {
    match status {
        "starting" => "Initialisation de l'export...".to_string(),
        "processing" => format!("Export en cours... ({:.1}%)", progress),
        "completed" => "Export terminé avec succès".to_string(),
        "failed" => "Erreur lors de l'export".to_string(),
        "cancelled" => "Export annulé".to_string(),
        _ => "Statut inconnu".to_string(),
    }
}

/// Construit une requête SQL intelligente selon les colonnes sélectionnées
fn build_select_query(columns: &[String], schema_name: &str) -> (String, String) {
    let mut select_parts = Vec::new();
    let mut tables_needed = HashSet::new();
    
    // Table principale tweet (toujours nécessaire)
    tables_needed.insert("tweet");
    
    for column in columns {
        match column.as_str() {
            // Colonnes de la table tweet
            "tweet.id" => select_parts.push("t.id".to_string()),
            "tweet.created_at" => select_parts.push("t.created_at".to_string()),
            "tweet.published_time" => select_parts.push("t.published_time".to_string()),
            "tweet.user_id" => select_parts.push("t.user_id".to_string()),
            "tweet.user_name" => select_parts.push("t.user_name".to_string()),
            "tweet.user_screen_name" => select_parts.push("t.user_screen_name".to_string()),
            "tweet.text" => select_parts.push("t.text".to_string()),
            "tweet.source" => select_parts.push("t.source".to_string()),
            "tweet.language" => select_parts.push("t.language".to_string()),
            "tweet.coordinates_longitude" => select_parts.push("t.coordinates_longitude".to_string()),
            "tweet.coordinates_latitude" => select_parts.push("t.coordinates_latitude".to_string()),
            "tweet.possibly_sensitive" => select_parts.push("t.possibly_sensitive".to_string()),
            "tweet.retweet_count" => select_parts.push("t.retweet_count".to_string()),
            "tweet.reply_count" => select_parts.push("t.reply_count".to_string()),
            "tweet.quote_count" => select_parts.push("t.quote_count".to_string()),
            
            // Colonnes de la table user
            "user.id" => {
                select_parts.push("usr.id".to_string());
                tables_needed.insert("user");
            },
            "user.screen_name" => {
                select_parts.push("usr.screen_name".to_string());
                tables_needed.insert("user");
            },
            "user.name" => {
                select_parts.push("usr.name".to_string());
                tables_needed.insert("user");
            },
            "user.created_at" => {
                select_parts.push("usr.created_at".to_string());
                tables_needed.insert("user");
            },
            "user.verified" => {
                select_parts.push("usr.verified".to_string());
                tables_needed.insert("user");
            },
            "user.protected" => {
                select_parts.push("usr.protected".to_string());
                tables_needed.insert("user");
            },
            
            // Colonnes de la table tweet_hashtag
            "tweet_hashtag.tweet_id" => {
                select_parts.push("th.tweet_id".to_string());
                tables_needed.insert("tweet_hashtag");
            },
            "tweet_hashtag.hashtag" => {
                select_parts.push("th.hashtag".to_string());
                tables_needed.insert("tweet_hashtag");
            },
            "tweet_hashtag.order" => {
                select_parts.push("th.\"order\"".to_string());
                tables_needed.insert("tweet_hashtag");
            },
            "tweet_hashtag.start_indice" => {
                select_parts.push("th.start_indice".to_string());
                tables_needed.insert("tweet_hashtag");
            },
            "tweet_hashtag.end_indice" => {
                select_parts.push("th.end_indice".to_string());
                tables_needed.insert("tweet_hashtag");
            },
            
            // Colonnes de la table tweet_url
            "tweet_url.tweet_id" => {
                select_parts.push("tu.tweet_id".to_string());
                tables_needed.insert("tweet_url");
            },
            "tweet_url.url" => {
                select_parts.push("tu.url".to_string());
                tables_needed.insert("tweet_url");
            },
            "tweet_url.order" => {
                select_parts.push("tu.\"order\"".to_string());
                tables_needed.insert("tweet_url");
            },
            "tweet_url.start_indice" => {
                select_parts.push("tu.start_indice".to_string());
                tables_needed.insert("tweet_url");
            },
            "tweet_url.end_indice" => {
                select_parts.push("tu.end_indice".to_string());
                tables_needed.insert("tweet_url");
            },
            
            // Colonnes de la table retweet
            "retweet.tweet_id" => {
                select_parts.push("rt.tweet_id".to_string());
                tables_needed.insert("retweet");
            },
            "retweet.retweeted_tweet_id" => {
                select_parts.push("rt.retweeted_tweet_id".to_string());
                tables_needed.insert("retweet");
            },
            
            // Colonnes de la table reply
            "reply.tweet_id" => {
                select_parts.push("r.tweet_id".to_string());
                tables_needed.insert("reply");
            },
            "reply.in_reply_to_tweet_id" => {
                select_parts.push("r.in_reply_to_tweet_id".to_string());
                tables_needed.insert("reply");
            },
            "reply.in_reply_to_user_id" => {
                select_parts.push("r.in_reply_to_user_id".to_string());
                tables_needed.insert("reply");
            },
            "reply.in_reply_to_screen_name" => {
                select_parts.push("r.in_reply_to_screen_name".to_string());
                tables_needed.insert("reply");
            },
            
            // Colonnes de la table quote
            "quote.tweet_id" => {
                select_parts.push("q.tweet_id".to_string());
                tables_needed.insert("quote");
            },
            "quote.quoted_tweet_id" => {
                select_parts.push("q.quoted_tweet_id".to_string());
                tables_needed.insert("quote");
            },
            
            // Colonnes de la table place
            "place.id" => {
                select_parts.push("p.id".to_string());
                tables_needed.insert("place");
            },
            "place.name" => {
                select_parts.push("p.name".to_string());
                tables_needed.insert("place");
            },
            "place.full_name" => {
                select_parts.push("p.full_name".to_string());
                tables_needed.insert("place");
            },
            "place.country_code" => {
                select_parts.push("p.country_code".to_string());
                tables_needed.insert("place");
            },
            "place.country" => {
                select_parts.push("p.country".to_string());
                tables_needed.insert("place");
            },
            "place.place_type" => {
                select_parts.push("p.place_type".to_string());
                tables_needed.insert("place");
            },
            "place.url" => {
                select_parts.push("p.url".to_string());
                tables_needed.insert("place");
            },
            "place.bounding_box" => {
                select_parts.push("p.bounding_box".to_string());
                tables_needed.insert("place");
            },
            "place.type_bounding_box" => {
                select_parts.push("p.type_bounding_box".to_string());
                tables_needed.insert("place");
            },
            
            // Colonnes de la table tweet_media
            "tweet_media.tweet_id" => {
                select_parts.push("tm.tweet_id".to_string());
                tables_needed.insert("tweet_media");
            },
            "tweet_media.media_url" => {
                select_parts.push("tm.media_url".to_string());
                tables_needed.insert("tweet_media");
            },
            "tweet_media.type" => {
                select_parts.push("tm.type".to_string());
                tables_needed.insert("tweet_media");
            },
            "tweet_media.order" => {
                select_parts.push("tm.\"order\"".to_string());
                tables_needed.insert("tweet_media");
            },
            "tweet_media.source_tweet_id" => {
                select_parts.push("tm.source_tweet_id".to_string());
                tables_needed.insert("tweet_media");
            },
            
            // Colonnes de la table corpus
            "corpus.tweet_id" => {
                select_parts.push("c.tweet_id".to_string());
                tables_needed.insert("corpus");
            },
            "corpus.corpus" => {
                select_parts.push("c.corpus".to_string());
                tables_needed.insert("corpus");
            },
            
            // Colonnes de la table tweet_cashtag
            "tweet_cashtag.tweet_id" => {
                select_parts.push("tca.tweet_id".to_string());
                tables_needed.insert("tweet_cashtag");
            },
            "tweet_cashtag.cashtag" => {
                select_parts.push("tca.cashtag".to_string());
                tables_needed.insert("tweet_cashtag");
            },
            "tweet_cashtag.order" => {
                select_parts.push("tca.\"order\"".to_string());
                tables_needed.insert("tweet_cashtag");
            },
            "tweet_cashtag.start_indice" => {
                select_parts.push("tca.start_indice".to_string());
                tables_needed.insert("tweet_cashtag");
            },
            "tweet_cashtag.end_indice" => {
                select_parts.push("tca.end_indice".to_string());
                tables_needed.insert("tweet_cashtag");
            },
            
            // Colonnes de la table tweet_emoji
            "tweet_emoji.tweet_id" => {
                select_parts.push("te.tweet_id".to_string());
                tables_needed.insert("tweet_emoji");
            },
            "tweet_emoji.emoji" => {
                select_parts.push("te.emoji".to_string());
                tables_needed.insert("tweet_emoji");
            },
            "tweet_emoji.order" => {
                select_parts.push("te.\"order\"".to_string());
                tables_needed.insert("tweet_emoji");
            },
            "tweet_emoji.start_indice" => {
                select_parts.push("te.start_indice".to_string());
                tables_needed.insert("tweet_emoji");
            },
            "tweet_emoji.end_indice" => {
                select_parts.push("te.end_indice".to_string());
                tables_needed.insert("tweet_emoji");
            },
            
            // Colonnes de la table tweet_user_mention
            "tweet_user_mention.tweet_id" => {
                select_parts.push("tum.tweet_id".to_string());
                tables_needed.insert("tweet_user_mention");
            },
            "tweet_user_mention.user_id" => {
                select_parts.push("tum.user_id".to_string());
                tables_needed.insert("tweet_user_mention");
            },
            "tweet_user_mention.order" => {
                select_parts.push("tum.\"order\"".to_string());
                tables_needed.insert("tweet_user_mention");
            },
            "tweet_user_mention.start_indice" => {
                select_parts.push("tum.start_indice".to_string());
                tables_needed.insert("tweet_user_mention");
            },
            "tweet_user_mention.end_indice" => {
                select_parts.push("tum.end_indice".to_string());
                tables_needed.insert("tweet_user_mention");
            },
            
            // Colonnes de la table tweet_keyword_user
            "tweet_keyword_user.tweet_id" => {
                select_parts.push("tku.tweet_id".to_string());
                tables_needed.insert("tweet_keyword_user");
            },
            "tweet_keyword_user.user_id" => {
                select_parts.push("tku.user_id".to_string());
                tables_needed.insert("tweet_keyword_user");
            },
            
            // Colonnes de la table tweet_keyword_hashtag
            "tweet_keyword_hashtag.tweet_id" => {
                select_parts.push("tkh.tweet_id".to_string());
                tables_needed.insert("tweet_keyword_hashtag");
            },
            "tweet_keyword_hashtag.hashtag" => {
                select_parts.push("tkh.hashtag".to_string());
                tables_needed.insert("tweet_keyword_hashtag");
            },
            
            // Colonnes de la table withheld_in_country
            "withheld_in_country.user_id" => {
                select_parts.push("wic.user_id".to_string());
                tables_needed.insert("withheld_in_country");
            },
            "withheld_in_country.country" => {
                select_parts.push("wic.country".to_string());
                tables_needed.insert("withheld_in_country");
            },
            
            _ => {
                // Colonne inconnue, on l'ignore ou on la traite comme une colonne tweet
                tracing::warn!("Colonne inconnue: {}", column);
            }
        }
    }
    
    // Construire la clause FROM avec les JOINs nécessaires
    let mut from_clause = format!("\"{}\".tweet t", schema_name);
    
    if tables_needed.contains("user") {
        from_clause.push_str(&format!(" LEFT JOIN \"{}\".user usr ON t.user_id = usr.id", schema_name));
    }
    if tables_needed.contains("tweet_hashtag") {
        from_clause.push_str(&format!(" LEFT JOIN \"{}\".tweet_hashtag th ON t.id = th.tweet_id", schema_name));
    }
    if tables_needed.contains("tweet_url") {
        from_clause.push_str(&format!(" LEFT JOIN \"{}\".tweet_url tu ON t.id = tu.tweet_id", schema_name));
    }
    if tables_needed.contains("retweet") {
        from_clause.push_str(&format!(" LEFT JOIN \"{}\".retweet rt ON t.id = rt.tweet_id", schema_name));
    }
    if tables_needed.contains("reply") {
        from_clause.push_str(&format!(" LEFT JOIN \"{}\".reply r ON t.id = r.tweet_id", schema_name));
    }
    if tables_needed.contains("quote") {
        from_clause.push_str(&format!(" LEFT JOIN \"{}\".quote q ON t.id = q.tweet_id", schema_name));
    }
    if tables_needed.contains("place") {
        from_clause.push_str(&format!(" LEFT JOIN \"{}\".tweet_place tp ON t.id = tp.tweet_id LEFT JOIN \"{}\".place p ON tp.place_id = p.id", schema_name, schema_name));
    }
    if tables_needed.contains("tweet_media") {
        from_clause.push_str(&format!(" LEFT JOIN \"{}\".tweet_media tm ON t.id = tm.tweet_id", schema_name));
    }
    if tables_needed.contains("corpus") {
        from_clause.push_str(&format!(" LEFT JOIN \"{}\".corpus c ON t.id = c.tweet_id", schema_name));
    }
    if tables_needed.contains("tweet_cashtag") {
        from_clause.push_str(&format!(" LEFT JOIN \"{}\".tweet_cashtag tca ON t.id = tca.tweet_id", schema_name));
    }
    if tables_needed.contains("tweet_emoji") {
        from_clause.push_str(&format!(" LEFT JOIN \"{}\".tweet_emoji te ON t.id = te.tweet_id", schema_name));
    }
    if tables_needed.contains("tweet_user_mention") {
        from_clause.push_str(&format!(" LEFT JOIN \"{}\".tweet_user_mention tum ON t.id = tum.tweet_id", schema_name));
    }
    if tables_needed.contains("tweet_keyword_user") {
        from_clause.push_str(&format!(" LEFT JOIN \"{}\".tweet_keyword_user tku ON t.id = tku.tweet_id", schema_name));
    }
    if tables_needed.contains("tweet_keyword_hashtag") {
        from_clause.push_str(&format!(" LEFT JOIN \"{}\".tweet_keyword_hashtag tkh ON t.id = tkh.tweet_id", schema_name));
    }
    if tables_needed.contains("withheld_in_country") {
        from_clause.push_str(&format!(" LEFT JOIN \"{}\".withheld_in_country wic ON t.user_id = wic.user_id", schema_name));
    }
    
    let select_clause = if select_parts.is_empty() {
        "t.*".to_string() // Fallback si aucune colonne spécifiée
    } else {
        select_parts.join(", ")
    };
    
    (select_clause, from_clause)
} 