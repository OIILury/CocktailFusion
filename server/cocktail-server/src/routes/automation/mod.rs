use axum::{
    response::IntoResponse,
    http::StatusCode,
    Json,
    Router,
    routing::post,
};
use serde::{Deserialize, Serialize};
use tracing::{info, error};

mod config;
mod error;
mod steps;

use crate::routes::automation::error::AutomationError;
use crate::routes::automation::steps::{
    export::run_export,
    cleanup::run_cleanup,
    index_creation::run_index_creation,
    ingestion::run_ingestion,
    top_hashtags::run_top_hashtags,
    cooccurrence::run_cooccurrence,
    schema_copy::run_schema_copy,
};

/// Structure de réponse pour l'API d'automatisation
#[derive(Debug, Serialize, Deserialize)]
pub struct AutomationResponse {
    message: String,
    status: String,
    progress: u32,
    total_steps: u32,
}

/// Structure pour suivre la progression de l'automatisation
#[derive(Debug, Serialize, Deserialize)]
pub struct AutomationProgress {
    pub current_step: u32,
    pub total_steps: u32,
    pub step_name: String,
    pub status: AutomationStatus,
}

/// État de progression de l'automatisation
#[derive(Debug, Serialize, Deserialize)]
pub enum AutomationStatus {
    InProgress,
    Completed,
    Failed(String),
}

/// Contexte partagé entre les étapes de l'automatisation
#[derive(Debug)]
pub struct AutomationContext {
    pub schema_name: String,
    pub project_id: Option<String>,
    pub workspace_dir: std::path::PathBuf,
    pub project_dir: std::path::PathBuf,
    pub tantivy_dir: std::path::PathBuf,
    pub database_url: String,
    pub date_str: String,
    pub gzip_file: String,
}

/// Point d'entrée pour les routes d'automatisation
pub fn routes() -> Router {
    Router::new()
        .route("/api/automation/run", post(run_automation))
}

/// Endpoint principal pour lancer l'automatisation
pub async fn run_automation() -> impl IntoResponse {
    info!("Début du pipeline d'automatisation");
    
    match run_automation_pipeline("public", None).await {
        Ok(_) => {
            info!("Pipeline d'automatisation terminé avec succès");
            (StatusCode::OK, Json(AutomationResponse {
                message: "Pipeline d'automatisation terminé avec succès".to_string(),
                status: "success".to_string(),
                progress: 100,
                total_steps: 7,
            })).into_response()
        },
        Err(e) => {
            error!("Erreur lors du pipeline d'automatisation: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(AutomationResponse {
                message: format!("Erreur lors de l'automatisation : {}", e),
                status: "error".to_string(),
                progress: 0,
                total_steps: 7,
            })).into_response()
        }
    }
}

/// Fonction principale qui orchestre toutes les étapes de l'automatisation
pub async fn run_automation_pipeline(schema_name: &str, project_id: Option<String>) -> Result<(), AutomationError> {
    // Initialisation du contexte
    let tantivy_dir = std::path::PathBuf::from(format!("tantivy-data/{}", schema_name));
    let context = AutomationContext {
        schema_name: schema_name.to_string(),
        project_id: project_id,
        workspace_dir: std::env::current_dir()?,
        project_dir: tantivy_dir.parent().unwrap().to_path_buf(),
        tantivy_dir,
        database_url: std::env::var("PG_DATABASE_URL")?,
        date_str: chrono::Local::now().format("%Y_%m_%d").to_string(),
        gzip_file: format!("tweets_collecte_{}.json.gz", chrono::Local::now().format("%Y_%m_%d")),
    };

    // Exécution des étapes
    info!("Étape 1/7: Export et compression des tweets");
    run_export(&context).await?;

    info!("Étape 2/7: Nettoyage des anciens index");
    run_cleanup(&context).await?;

    info!("Étape 3/7: Création de l'index Tantivy");
    run_index_creation(&context).await?;

    info!("Étape 4/7: Ingestion des tweets");
    run_ingestion(&context).await?;

    info!("Étape 5/7: Copie des données vers le schéma du projet");
    run_schema_copy(&context).await?;

    info!("Étape 6/7: Génération des top hashtags");
    run_top_hashtags(&context).await?;

    info!("Étape 7/7: Calcul des cooccurrences");
    run_cooccurrence(&context).await?;

    info!("Pipeline d'automatisation terminé avec succès");
    Ok(())
} 