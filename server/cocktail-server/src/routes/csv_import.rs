use axum::{
    extract::{Multipart, Query},
    response::{IntoResponse, Response},
    http::StatusCode,
    Json,
    routing::post,
    Router,
};
use serde::{Deserialize, Serialize};
use std::error::Error;
use sqlx::postgres::PgPool;
use chrono::Local;
use tracing::{info, error, warn, debug};
use crate::routes::automation::run_automation_pipeline;
use csv;
use actix_web::{web, HttpResponse, Responder};
use crate::csv_analyzer::{CsvAnalyzer, CsvAnalysis};
use std::io::Cursor;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;

const BATCH_SIZE: usize = 1000;

/// Structure de réponse pour l'API d'importation CSV
#[derive(Debug, Serialize, Deserialize)]
pub struct CsvImportResponse {
    message: String,
    rows_imported: usize,
    error_rows: usize,
    error_file: Option<String>,
}

/// Paramètres de requête pour l'importation CSV
#[derive(Debug, Deserialize)]
pub struct ImportParams {
    project_id: Option<String>,
}

/// Définit la route liée à l'importation de fichiers CSV.
pub fn routes() -> Router {
    Router::new()
        .route("/api/import/csv", post(import_csv))
}

/// Gère l'importation de fichiers CSV.
/// 
/// # Arguments
/// * `query` - Paramètres de la requête contenant l'ID du projet optionnel
/// * `multipart` - Données multipart contenant les fichiers CSV et le mode d'importation
/// 
/// # Retour
/// Retourne une réponse HTTP avec :
/// * Un message de statut, le nombre de lignes importées, un code de statut approprié
/// 
/// # Erreurs
/// Retourne une erreur 500 en cas d'échec de l'importation
async fn import_csv(
    query: Query<ImportParams>,
    multipart: Multipart,
) -> impl IntoResponse {
    info!("Début de l'importation CSV");
    let database_url = std::env::var("PG_DATABASE_URL")
        .expect("La variable d'environnement PG_DATABASE_URL n'est pas définie");
    
    match import_csv_internal(database_url, multipart, query.project_id.clone()).await {
        Ok(response) => { info!("Importation CSV réussie");
            response
        },
        Err(e) => {
            error!("Erreur lors de l'importation CSV: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(CsvImportResponse {
                message: format!("Erreur lors de l'import : {}", e),
                rows_imported: 0,
                error_rows: 0,
                error_file: None,
            })).into_response()
        }
    }
}

async fn import_csv_internal(
    database_url: String,
    mut multipart: Multipart,
    project_id: Option<String>,
) -> Result<Response, Box<dyn Error>> {
    debug!("Récupération du contenu des fichiers CSV");
    let mut files_content = Vec::new();
    let mut import_mode = String::from("single");
    let mut import_name = String::new();
    let mut source = String::from("unknown");

    while let Some(field) = multipart.next_field().await? {
        match field.name() {
            Some("files") => {
                let content = String::from_utf8(field.bytes().await?.to_vec())?;
                files_content.push(content);
            },
            Some("mode") => {
                import_mode = String::from_utf8(field.bytes().await?.to_vec())?;
            },
            Some("name") => {
                import_name = String::from_utf8(field.bytes().await?.to_vec())?;
            },
            Some("source") => {
                source = String::from_utf8(field.bytes().await?.to_vec())?;
            },
            _ => {}
        }
    }

    if files_content.is_empty() {
        warn!("Aucun fichier CSV n'a été fourni");
        return Err("Aucun fichier CSV fourni".into());
    }

    if import_name.is_empty() {
        warn!("Aucun nom d'importation fourni");
        return Err("Le nom d'importation est requis".into());
    }

    // Créer le nom du schéma basé sur le nom d'importation
    let schema_name = format!("import_{}", import_name.replace(" ", "_").to_lowercase());
    info!("Création du schéma: {}", schema_name);
    
    // Créer la connexion à la base de données
    let pool = PgPool::connect(&database_url).await?;
    
    // Supprimer le schéma s'il existe déjà
    sqlx::query(&format!(
        "DROP SCHEMA IF EXISTS {} CASCADE",
        schema_name
    ))
    .execute(&pool)
    .await?;
    
    // Créer le schéma
    sqlx::query(&format!(
        "CREATE SCHEMA IF NOT EXISTS {}",
        schema_name
    ))
    .execute(&pool)
    .await?;

    // Créer les tables
    create_tables(&pool, &schema_name).await?;

    // Créer le fichier de log pour les erreurs
    let error_log_path = format!("logs/import_errors_{}.csv", schema_name);
    let error_log_dir = Path::new("logs");
    if !error_log_dir.exists() {
        std::fs::create_dir_all(error_log_dir)?;
    }
    let mut error_log = OpenOptions::new()
        .write(true)
        .create(true)
        .append(true)
        .open(&error_log_path)?;

    let mut total_rows_imported = 0;
    let mut total_errors = 0;

    if import_mode == "single" {
        // Mode fichier unique
        if files_content.len() > 1 {
            return Err("Mode fichier unique sélectionné mais plusieurs fichiers fournis".into());
        }
        let (rows_imported, errors) = process_single_file(&pool, &schema_name, &files_content[0], &source, &mut error_log).await?;
        total_rows_imported += rows_imported;
        total_errors += errors;
    } else {
        // Mode fichiers multiples
        for content in files_content {
            let (rows_imported, errors) = process_multiple_files(&pool, &schema_name, &content, &source, &mut error_log).await?;
            total_rows_imported += rows_imported;
            total_errors += errors;
        }
    }

    info!("Importation terminée: {} lignes importées, {} erreurs", total_rows_imported, total_errors);

    // Lancer le pipeline d'automatisation avec le schéma créé et l'ID du projet
    if total_rows_imported > 0 {
        info!("Démarrage du pipeline d'automatisation pour le schéma {}", schema_name);
        if let Err(e) = run_automation_pipeline(&schema_name, project_id).await {
            error!("Erreur lors du pipeline d'automatisation: {}", e);
            return Ok((StatusCode::INTERNAL_SERVER_ERROR, Json(CsvImportResponse {
                message: format!("Import réussi mais erreur lors de l'automatisation : {}", e),
                rows_imported: total_rows_imported,
                error_rows: total_errors,
                error_file: Some(error_log_path),
            })).into_response());
        }
    }

    Ok((StatusCode::OK, Json(CsvImportResponse {
        message: format!("Import réussi avec {} erreurs", total_errors),
        rows_imported: total_rows_imported,
        error_rows: total_errors,
        error_file: if total_errors > 0 { Some(error_log_path) } else { None },
    })).into_response())
}

async fn create_tables(pool: &PgPool, schema_name: &str) -> Result<(), Box<dyn Error>> {
    // Table tweet
    sqlx::query(&format!(
        r#"
        CREATE TABLE IF NOT EXISTS {}.tweet (
            id TEXT PRIMARY KEY,
            created_at TEXT NOT NULL,
            published_time BIGINT NOT NULL,
            user_id TEXT NOT NULL,
            user_name TEXT NOT NULL,
            user_screen_name TEXT NOT NULL,
            text TEXT NOT NULL,
            source TEXT NOT NULL,
            language TEXT NOT NULL,
            coordinates_longitude TEXT,
            coordinates_latitude TEXT,
            possibly_sensitive BOOLEAN,
            retweet_count BIGINT NOT NULL DEFAULT 0,
            reply_count BIGINT NOT NULL DEFAULT 0,
            quote_count BIGINT NOT NULL DEFAULT 0
        )
        "#,
        schema_name
    ))
    .execute(pool)
    .await?;

    // Table tweet_hashtag
    sqlx::query(&format!(
        "CREATE TABLE IF NOT EXISTS {}.tweet_hashtag (tweet_id TEXT REFERENCES {}.tweet(id), hashtag TEXT)",
        schema_name, schema_name
    ))
    .execute(pool)
    .await?;

    // Table tweet_url
    sqlx::query(&format!(
        "CREATE TABLE IF NOT EXISTS {}.tweet_url (tweet_id TEXT REFERENCES {}.tweet(id), url TEXT)",
        schema_name, schema_name
    ))
    .execute(pool)
    .await?;

    // Table retweet
    sqlx::query(&format!(
        "CREATE TABLE IF NOT EXISTS {}.retweet (retweeted_tweet_id TEXT REFERENCES {}.tweet(id))",
        schema_name, schema_name
    ))
    .execute(pool)
    .await?;

    // Table reply
    sqlx::query(&format!(
        "CREATE TABLE IF NOT EXISTS {}.reply (in_reply_to_tweet_id TEXT REFERENCES {}.tweet(id))",
        schema_name, schema_name
    ))
    .execute(pool)
    .await?;

    // Table quote
    sqlx::query(&format!(
        "CREATE TABLE IF NOT EXISTS {}.quote (quoted_tweet_id TEXT REFERENCES {}.tweet(id))",
        schema_name, schema_name
    ))
    .execute(pool)
    .await?;

    Ok(())
}

async fn process_single_file(
    pool: &PgPool, 
    schema_name: &str, 
    content: &str,
    source: &str,
    error_log: &mut File
) -> Result<(usize, usize), Box<dyn Error>> {
    let mut rows_imported = 0;
    let mut errors = 0;
    let mut first_line = true;
    let mut batch = Vec::new();
    
    for line in content.lines() {
        if first_line {
            debug!("En-tête CSV: {}", line);
            first_line = false;
            continue;
        }

        let mut rdr = csv::ReaderBuilder::new()
            .has_headers(false)
            .from_reader(line.as_bytes());
        
        if let Some(result) = rdr.records().next() {
            match result {
                Ok(record) => {
                    let fields: Vec<&str> = record.iter().collect();
                    if fields.len() >= 10 {
                        batch.push(fields);
                        
                        if batch.len() >= BATCH_SIZE {
                            match process_batch(pool, schema_name, &batch, source, error_log).await {
                                Ok((imported, err)) => {
                                    rows_imported += imported;
                                    errors += err;
                                },
                            Err(e) => {
                                    error!("Erreur lors du traitement du lot: {}", e);
                                    errors += batch.len();
                                    // Écrire toutes les lignes du lot dans le fichier d'erreur
                                    for fields in &batch {
                                        writeln!(error_log, "{},{}", line, e)?;
                            }
                                }
                            }
                            batch.clear();
                        }
                    } else {
                        warn!("Ligne ignorée (format invalide): {}", line);
                        errors += 1;
                        writeln!(error_log, "{},Format invalide", line)?;
                    }
                },
                Err(e) => {
                    error!("Erreur lors de la lecture de la ligne: {}", e);
                    errors += 1;
                    writeln!(error_log, "{},Erreur de lecture: {}", line, e)?;
                }
            }
        }
    }

    // Traiter le dernier lot s'il reste des lignes
    if !batch.is_empty() {
        match process_batch(pool, schema_name, &batch, source, error_log).await {
            Ok((imported, err)) => {
                rows_imported += imported;
                errors += err;
            },
            Err(e) => {
                error!("Erreur lors du traitement du dernier lot: {}", e);
                errors += batch.len();
                for fields in &batch {
                    writeln!(error_log, "{},{}", line, e)?;
                }
            }
        }
    }

    Ok((rows_imported, errors))
}

async fn process_batch(
    pool: &PgPool,
    schema_name: &str,
    batch: &[Vec<&str>],
    source: &str,
    error_log: &mut File
) -> Result<(usize, usize), Box<dyn Error>> {
    let mut rows_imported = 0;
    let mut errors = 0;

    for fields in batch {
        match process_tweet_record(pool, schema_name, fields, source).await {
            Ok(_) => rows_imported += 1,
            Err(e) => {
                error!("Erreur lors du traitement de la ligne: {}", e);
                errors += 1;
                writeln!(error_log, "{},{}", fields.join(","), e)?;
            }
        }
    }

    Ok((rows_imported, errors))
}

async fn process_tweet_record(
    pool: &PgPool,
    schema_name: &str,
    fields: &[&str],
    source: &str
) -> Result<(), Box<dyn Error>> {
    let created_at = chrono::NaiveDateTime::parse_from_str(fields[1], "%Y-%m-%d %H:%M:%S")?;
    let published_time = created_at.timestamp_millis();

    // Insérer d'abord le tweet
    sqlx::query(&format!(
        r#"
        INSERT INTO {}.tweet 
        (id, created_at, published_time, text, user_id, user_name, user_screen_name, 
        source, language, coordinates_longitude, coordinates_latitude, possibly_sensitive,
        retweet_count, reply_count, quote_count)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
        "#,
        schema_name
    ))
    .bind(fields[0])  // id
    .bind(created_at)  // created_at
    .bind(published_time)  // published_time
    .bind(fields[6])  // text
    .bind(fields[3])  // user_id
    .bind(fields[4])  // user_name
    .bind(fields[5])  // user_screen_name
    .bind(source)  // source
    .bind(fields[8])  // language
    .bind(fields[9].parse::<f64>().ok().map(|v| v.to_string()))  // coordinates_longitude
    .bind(fields[10].parse::<f64>().ok().map(|v| v.to_string()))  // coordinates_latitude
    .bind(fields[11].to_lowercase() == "true")  // possibly_sensitive
    .bind(fields[12].parse::<i32>().unwrap_or(0))  // retweet_count
    .bind(fields[13].parse::<i32>().unwrap_or(0))  // reply_count
    .bind(fields[14].parse::<i32>().unwrap_or(0))  // quote_count
    .execute(pool)
    .await?;

    // Stocker les données à insérer dans les tables liées
    let mut hashtags_to_insert = Vec::new();
    let mut urls_to_insert = Vec::new();
    let mut is_retweet = false;
    let mut is_reply = false;
    let mut is_quote = false;

    // Collecter les hashtags
    if fields.len() > 15 {
        hashtags_to_insert = fields[15].split(',')
            .filter(|s| !s.is_empty() && s.starts_with("#"))
            .collect();
    }

    // Collecter les URLs
    if fields.len() > 16 {
        urls_to_insert = fields[16].split('|')
            .filter(|s| !s.is_empty() && s.starts_with("http"))
            .collect();
    }

    // Vérifier le type de tweet
    is_retweet = fields[6].contains("RT @");
    is_reply = fields[6].starts_with("@");
    is_quote = fields[6].contains("https://twitter.com/");

    // Insérer les hashtags
    for hashtag in hashtags_to_insert {
        sqlx::query(&format!(
            "INSERT INTO {}.tweet_hashtag (tweet_id, hashtag) VALUES ($1, $2)",
            schema_name
        ))
        .bind(fields[0])
        .bind(hashtag)
        .execute(pool)
        .await?;
    }

    // Insérer les URLs
    for url in urls_to_insert {
        sqlx::query(&format!(
            "INSERT INTO {}.tweet_url (tweet_id, url) VALUES ($1, $2)",
            schema_name
        ))
        .bind(fields[0])
        .bind(url)
        .execute(pool)
        .await?;
    }

    // Insérer le retweet si nécessaire
    if is_retweet {
        sqlx::query(&format!(
            "INSERT INTO {}.retweet (retweeted_tweet_id) VALUES ($1)",
            schema_name
        ))
        .bind(fields[0])
        .execute(pool)
        .await?;
    }

    // Insérer la réponse si nécessaire
    if is_reply {
        sqlx::query(&format!(
            "INSERT INTO {}.reply (in_reply_to_tweet_id) VALUES ($1)",
            schema_name
        ))
        .bind(fields[0])
        .execute(pool)
        .await?;
    }

    // Insérer la citation si nécessaire
    if is_quote {
        sqlx::query(&format!(
            "INSERT INTO {}.quote (quoted_tweet_id) VALUES ($1)",
            schema_name
        ))
        .bind(fields[0])
        .execute(pool)
        .await?;
    }

    Ok(())
}

async fn process_multiple_files(
    pool: &PgPool,
    schema_name: &str,
    content: &str,
    source: &str,
    error_log: &mut File
) -> Result<(usize, usize), Box<dyn Error>> {
    let mut rows_imported = 0;
    let mut errors = 0;
    let mut first_line = true;
    let mut table_name = String::new();
    
    // Stocker temporairement les données pour les insérer dans le bon ordre
    let mut tweets_to_insert = Vec::new();
    let mut hashtags_to_insert = Vec::new();
    let mut urls_to_insert = Vec::new();
    let mut retweets_to_insert = Vec::new();
    let mut replies_to_insert = Vec::new();
    let mut quotes_to_insert = Vec::new();
    
    for line in content.lines() {
        if first_line {
            debug!("En-tête CSV: {}", line);
            first_line = false;
            table_name = determine_table_name(line);
            continue;
        }

        let mut rdr = csv::ReaderBuilder::new()
            .has_headers(false)
            .from_reader(line.as_bytes());
        
        if let Some(result) = rdr.records().next() {
            match result {
                Ok(record) => {
                    let fields: Vec<String> = record.iter().map(|s| s.to_string()).collect();
                    match table_name.as_str() {
                        "tweet" => tweets_to_insert.push(fields),
                        "tweet_hashtag" => hashtags_to_insert.push(fields),
                        "tweet_url" => urls_to_insert.push(fields),
                        "retweet" => retweets_to_insert.push(fields),
                        "reply" => replies_to_insert.push(fields),
                        "quote" => quotes_to_insert.push(fields),
                        _ => {
                            error!("Type de table inconnu: {}", table_name);
                            errors += 1;
                            }
                    }
                },
                Err(e) => {
                    error!("Erreur lors de la lecture de la ligne: {}", e);
                    errors += 1;
                    writeln!(error_log, "{},Erreur de lecture: {}", line, e)?;
                }
            }
        }
    }

    // Insérer les données dans l'ordre correct
    // 1. D'abord les tweets
    for fields in tweets_to_insert {
        let str_fields: Vec<&str> = fields.iter().map(|s| s.as_str()).collect();
        match process_tweet_record(pool, schema_name, &str_fields, source).await {
            Ok(_) => rows_imported += 1,
            Err(e) => {
                error!("Erreur lors du traitement du tweet: {}", e);
                errors += 1;
                writeln!(error_log, "{},{}", fields.join(","), e)?;
            }
        }
    }

    // 2. Ensuite les hashtags
    for fields in hashtags_to_insert {
        if fields.len() >= 2 {
            match sqlx::query(&format!(
                "INSERT INTO {}.tweet_hashtag (tweet_id, hashtag) VALUES ($1, $2)",
                schema_name
            ))
            .bind(&fields[0])
            .bind(&fields[1])
            .execute(pool)
            .await {
                Ok(_) => rows_imported += 1,
                Err(e) => {
                    error!("Erreur lors de l'insertion du hashtag: {}", e);
                    errors += 1;
                    writeln!(error_log, "{},{}", fields.join(","), e)?;
                }
            }
        }
    }

    // 3. Les URLs
    for fields in urls_to_insert {
        if fields.len() >= 2 {
            match sqlx::query(&format!(
                "INSERT INTO {}.tweet_url (tweet_id, url) VALUES ($1, $2)",
                schema_name
            ))
            .bind(&fields[0])
            .bind(&fields[1])
            .execute(pool)
            .await {
                Ok(_) => rows_imported += 1,
                Err(e) => {
                    error!("Erreur lors de l'insertion de l'URL: {}", e);
                    errors += 1;
                    writeln!(error_log, "{},{}", fields.join(","), e)?;
                }
            }
        }
    }

    // 4. Les retweets
    for fields in retweets_to_insert {
        if !fields.is_empty() {
            match sqlx::query(&format!(
                "INSERT INTO {}.retweet (retweeted_tweet_id) VALUES ($1)",
                schema_name
            ))
            .bind(&fields[0])
            .execute(pool)
            .await {
                Ok(_) => rows_imported += 1,
                Err(e) => {
                    error!("Erreur lors de l'insertion du retweet: {}", e);
                    errors += 1;
                    writeln!(error_log, "{},{}", fields.join(","), e)?;
                }
            }
        }
    }

    // 5. Les réponses
    for fields in replies_to_insert {
        if !fields.is_empty() {
            match sqlx::query(&format!(
                "INSERT INTO {}.reply (in_reply_to_tweet_id) VALUES ($1)",
                schema_name
            ))
            .bind(&fields[0])
            .execute(pool)
            .await {
                Ok(_) => rows_imported += 1,
                Err(e) => {
                    error!("Erreur lors de l'insertion de la réponse: {}", e);
                    errors += 1;
                    writeln!(error_log, "{},{}", fields.join(","), e)?;
                }
            }
        }
    }

    // 6. Enfin les citations
    for fields in quotes_to_insert {
        if !fields.is_empty() {
            match sqlx::query(&format!(
                "INSERT INTO {}.quote (quoted_tweet_id) VALUES ($1)",
                schema_name
            ))
            .bind(&fields[0])
            .execute(pool)
            .await {
                Ok(_) => rows_imported += 1,
                Err(e) => {
                    error!("Erreur lors de l'insertion de la citation: {}", e);
                    errors += 1;
                    writeln!(error_log, "{},{}", fields.join(","), e)?;
                }
            }
        }
    }

    Ok((rows_imported, errors))
}

fn determine_table_name(header: &str) -> String {
    // Logique pour déterminer le type de table à partir de l'en-tête
    // À adapter selon vos besoins
    if header.contains("tweet_id") && header.contains("hashtag") {
        "tweet_hashtag".to_string()
    } else if header.contains("tweet_id") && header.contains("url") {
        "tweet_url".to_string()
    } else if header.contains("retweeted_tweet_id") {
        "retweet".to_string()
    } else if header.contains("in_reply_to_tweet_id") {
        "reply".to_string()
    } else if header.contains("quoted_tweet_id") {
        "quote".to_string()
    } else {
        "tweet".to_string()
    }
}

#[post("/api/analyze/csv")]
pub async fn analyze_csv(csv_content: web::Bytes) -> impl Responder {
    let csv_str = match String::from_utf8(csv_content.to_vec()) {
        Ok(s) => s,
        Err(_) => return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Le contenu CSV n'est pas valide UTF-8"
        }))
    };

    let analyzer = CsvAnalyzer::new();
    match analyzer.analyze(&csv_str) {
        Ok(analysis) => HttpResponse::Ok().json(analysis),
        Err(e) => HttpResponse::BadRequest().json(serde_json::json!({
            "error": e.to_string()
        }))
    }
} 