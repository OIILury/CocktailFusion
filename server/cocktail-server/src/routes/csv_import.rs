use axum::{
    extract::{State, Multipart},
    response::{IntoResponse, Response},
    http::StatusCode,
    Json,
    routing::post,
    Router,
};
use serde::{Deserialize, Serialize};
use std::error::Error;
use sqlx::postgres::PgPool;
use chrono::{Local, NaiveDateTime};
use tracing::{info, error, warn, debug};
use crate::AppState;

#[derive(Debug, Serialize, Deserialize)]
pub struct CsvImportResponse {
    message: String,
    rows_imported: usize,
}

pub fn routes() -> Router {
    Router::new()
        .route("/api/import/csv", post(import_csv))
}

async fn import_csv(
    multipart: Multipart,
) -> impl IntoResponse {
    info!("Début de l'importation CSV");
    // Récupérer l'URL de la base de données depuis les variables d'environnement
    let database_url = std::env::var("PG_DATABASE_URL").expect("PG_DATABASE_URL must be set");
    
    match import_csv_internal(database_url, multipart).await {
        Ok(response) => {
            info!("Importation CSV réussie");
            response
        },
        Err(e) => {
            error!("Erreur lors de l'importation CSV: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(CsvImportResponse {
                message: format!("Erreur lors de l'import : {}", e),
                rows_imported: 0,
            })).into_response()
        }
    }
}

async fn import_csv_internal(
    database_url: String,
    mut multipart: Multipart,
) -> Result<Response, Box<dyn Error>> {
    debug!("Récupération du contenu du fichier CSV");
    let mut content = String::new();
    while let Some(field) = multipart.next_field().await? {
        if field.name() == Some("file") {
            content = String::from_utf8(field.bytes().await?.to_vec())?;
            debug!("Taille du fichier CSV: {} octets", content.len());
            break;
        }
    }

    if content.is_empty() {
        warn!("Aucun fichier CSV n'a été fourni");
        return Err("Aucun fichier CSV fourni".into());
    }

    // Créer le nom du schéma basé sur la date
    let schema_name = format!("import_{}", Local::now().format("%Y%m%d"));
    info!("Création du schéma: {}", schema_name);
    
    // Créer la connexion à la base de données
    let pool = PgPool::connect(&database_url).await?;
    
    // Créer le schéma et la table
    debug!("Création de la table tweets dans le schéma {}", schema_name);
    
    // Créer d'abord le schéma
    sqlx::query(&format!(
        "CREATE SCHEMA IF NOT EXISTS {}",
        schema_name
    ))
    .execute(&pool)
    .await?;

    // Puis créer la table
    sqlx::query(&format!(
        r#"
        CREATE TABLE IF NOT EXISTS {}.tweets (
            id SERIAL PRIMARY KEY,
            tweet_id TEXT,
            created_at TIMESTAMP,
            text TEXT,
            user_id TEXT,
            user_name TEXT,
            user_screen_name TEXT,
            retweet_count INTEGER,
            favorite_count INTEGER,
            hashtags TEXT[],
            urls TEXT[]
        )
        "#,
        schema_name
    ))
    .execute(&pool)
    .await?;

    // Traiter le contenu CSV et insérer les données
    let mut rows_imported = 0;
    let mut errors = 0;
    let mut first_line = true;
    
    for line in content.lines() {
        if first_line {
            debug!("En-tête CSV: {}", line);
            first_line = false;
            continue;
        }

        let fields: Vec<&str> = line.split(',').collect();
        if fields.len() >= 10 {
            // Convertir la date en timestamp
            let created_at = match NaiveDateTime::parse_from_str(fields[1], "%Y-%m-%d %H:%M:%S") {
                Ok(dt) => dt,
                Err(_) => {
                    error!("Format de date invalide pour la ligne {}: {}", rows_imported + 1, fields[1]);
                    continue;
                }
            };

            match sqlx::query(&format!(
                r#"
                INSERT INTO {}.tweets 
                (tweet_id, created_at, text, user_id, user_name, user_screen_name, 
                retweet_count, favorite_count, hashtags, urls)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                "#,
                schema_name
            ))
            .bind(fields[0])
            .bind(created_at)
            .bind(fields[2])
            .bind(fields[3])
            .bind(fields[4])
            .bind(fields[5])
            .bind(fields[6].parse::<i32>().unwrap_or(0))
            .bind(fields[7].parse::<i32>().unwrap_or(0))
            .bind(fields[8].split('|').collect::<Vec<&str>>())
            .bind(fields[9].split('|').collect::<Vec<&str>>())
            .execute(&pool)
            .await {
                Ok(_) => rows_imported += 1,
                Err(e) => {
                    error!("Erreur lors de l'insertion de la ligne {}: {}", rows_imported + 1, e);
                    errors += 1;
                }
            }
        } else {
            warn!("Ligne ignorée (format invalide): {}", line);
            errors += 1;
        }
    }

    info!("Importation terminée: {} lignes importées, {} erreurs", rows_imported, errors);

    Ok((StatusCode::OK, Json(CsvImportResponse {
        message: format!("Import réussi avec {} erreurs", errors),
        rows_imported,
    })).into_response())
} 