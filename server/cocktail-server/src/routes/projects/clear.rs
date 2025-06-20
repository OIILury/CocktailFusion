use crate::{
    error::WebError,
    models::auth::AuthenticatedUser,
    routes::paths::ClearDataLatest,
};
use axum::{
    extract::State,
    response::{IntoResponse, Redirect},
    Json,
};
use serde::{Deserialize, Serialize};
use crate::AppState;

#[derive(Debug, Serialize)]
pub struct ClearDataResponse {
    pub success: bool,
    pub message: String,
}

/// Handler pour supprimer le schéma data_latest
#[tracing::instrument(skip(state))]
pub async fn clear_data_latest(
    ClearDataLatest { project_id: _ }: ClearDataLatest,
    AuthenticatedUser { .. }: AuthenticatedUser,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, WebError> {
    // Get database connection
    let pool = sqlx::PgPool::connect(&std::env::var("PG_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://cocktailuser:cocktailuser@localhost:5432/cocktail_pg".to_string()))
        .await
        .map_err(|e| WebError::WTFError(format!("DB connection error: {}", e)))?;

    // Drop the data_latest schema
    let schema_name = "data_latest";
    tracing::info!("Suppression du schéma {}", schema_name);
    
    sqlx::query(&format!("DROP SCHEMA IF EXISTS {} CASCADE", schema_name))
        .execute(&pool)
        .await
        .map_err(|e| WebError::WTFError(format!("Failed to drop schema {}: {}", schema_name, e)))?;

    tracing::info!("Schéma {} supprimé avec succès", schema_name);
    
    // Redirect back to projects list
    Ok(Redirect::to("/projets"))
} 