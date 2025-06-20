use axum::{
    extract::{Multipart, Query},
    response::{IntoResponse, Response},
    http::StatusCode,
    Json,
    routing::{post, get},
    Router,
};
use serde::{Deserialize, Serialize};
use std::error::Error;
use sqlx::postgres::PgPool;
use chrono::Local;
use tracing::{info, error, warn, debug};
use crate::routes::automation::run_automation_pipeline;
use crate::utils::csv_analyzer::{CsvAnalyzer, CsvAnalysis};
use csv;

#[derive(Debug, Serialize, Deserialize)]
pub struct CsvImportResponse {
    message: String,
    rows_imported: usize,
    errors: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CsvAnalysisResponse {
    analysis: CsvAnalysis,
    schema_name: String,
}

#[derive(Debug, Deserialize)]
pub struct ImportParams {
    project_id: Option<String>,
    schema_name: Option<String>,
    source: Option<String>,
    analysis: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AnalysisParams {
    project_id: Option<String>,
}

pub fn routes() -> Router {
    Router::new()
        .route("/api/import/csv/analyze", post(analyze_csv))
        .route("/api/import/csv", post(import_csv))
}

async fn analyze_csv(
    query: Query<AnalysisParams>,
    multipart: Multipart,
) -> impl IntoResponse {
    info!("Début de l'analyse CSV");
    
    match analyze_csv_internal(multipart, query.project_id.clone()).await {
        Ok(response) => {
            info!("Analyse CSV réussie");
            response
        },
        Err(e) => {
            error!("Erreur lors de l'analyse CSV: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(CsvAnalysisResponse {
                analysis: CsvAnalysis {
                    total_rows: 0,
                    total_columns: 0,
                    headers: Vec::new(),
                    preview: Vec::new(),
                    encoding: String::new(),
                    delimiter: ',',
                    potential_issues: Vec::new(),
                    data_types: std::collections::HashMap::new(),
                },
                schema_name: String::new(),
            })).into_response()
        }
    }
}

async fn analyze_csv_internal(
    mut multipart: Multipart,
    _project_id: Option<String>,
) -> Result<Response, Box<dyn Error>> {
    debug!("Récupération du contenu des fichiers CSV pour analyse");
    let mut files_content = Vec::new();
    let mut _import_mode = String::from("single");

    while let Some(mut field) = multipart.next_field().await? {
        match field.name() {
            Some("files") => {
                // Lire le contenu par morceaux
                let mut content = Vec::new();
                let mut chunk = Vec::with_capacity(8192); // Buffer de 8KB
                
                while let Some(data) = field.chunk().await? {
                    chunk.extend_from_slice(&data);
                    if chunk.len() >= 8192 {
                        content.extend_from_slice(&chunk);
                        chunk.clear();
                    }
                }
                if !chunk.is_empty() {
                    content.extend_from_slice(&chunk);
                }
                
                let content_str = String::from_utf8(content)?;
                files_content.push(content_str);
            },
            Some("mode") => {
                _import_mode = String::from_utf8(field.bytes().await?.to_vec())?;
            },
            _ => {}
        }
    }

    if files_content.is_empty() {
        warn!("Aucun fichier CSV n'a été fourni pour l'analyse");
        return Err("Aucun fichier CSV fourni".into());
    }

    // Créer le nom du schéma basé sur la date
    let schema_name = format!("import_{}", Local::now().format("%Y%m%d"));
    
    // Analyser le premier fichier
    let mut analyzer = CsvAnalyzer::new(files_content[0].clone());
    let analysis = analyzer.analyze()?;

    Ok((StatusCode::OK, Json(CsvAnalysisResponse {
        analysis,
        schema_name,
    })).into_response())
}

// Modification de la fonction handle_error pour retourner le bon type
fn handle_error(error: Box<dyn Error>) -> Response {
    let error_message = match error.to_string().as_str() {
        "Format de données invalide" => "Le format des données n'est pas valide",
        "Format de date invalide" => "Le format de date n'est pas valide",
        "Caractères non autorisés détectés" => "Le fichier contient des caractères non autorisés",
        "Champ trop long" => "Certains champs dépassent la longueur maximale autorisée",
        _ => "Une erreur est survenue lors de l'importation"
    };

    (StatusCode::BAD_REQUEST, Json(CsvImportResponse {
        message: error_message.to_string(),
        rows_imported: 0,
        errors: vec![error_message.to_string()],
    })).into_response()
}

// Modification de la fonction import_csv pour utiliser la nouvelle gestion d'erreurs
async fn import_csv(
    query: Query<ImportParams>,
    multipart: Multipart,
) -> impl IntoResponse {
    info!("Début de l'importation CSV");
    let database_url = std::env::var("PG_DATABASE_URL").expect("PG_DATABASE_URL must be set");
    
    match import_csv_internal(database_url, multipart, query.project_id.clone(), query.schema_name.clone(), query.source.clone(), query.analysis.clone()).await {
        Ok(response) => {
            info!("Importation CSV réussie");
            response
        },
        Err(e) => {
            error!("Erreur lors de l'importation CSV: {}", e);
            handle_error(e)
        }
    }
}

async fn import_csv_internal(
    database_url: String,
    mut multipart: Multipart,
    project_id: Option<String>,
    schema_name: Option<String>,
    source: Option<String>,
    analysis: Option<String>,
) -> Result<Response, Box<dyn Error>> {
    debug!("Récupération du contenu des fichiers CSV");
    let mut files_content = Vec::new();
    let mut import_mode = String::from("single");

    while let Some(field) = multipart.next_field().await? {
        match field.name() {
            Some("files") => {
                let content = String::from_utf8(field.bytes().await?.to_vec())?;
                files_content.push(content);
            },
            Some("mode") => {
                import_mode = String::from_utf8(field.bytes().await?.to_vec())?;
            },
            Some("analysis") => {
                continue;
            },
            _ => {}
        }
    }

    if files_content.is_empty() {
        warn!("Aucun fichier CSV n'a été fourni");
        return Err("Aucun fichier CSV fourni".into());
    }

    // Utiliser le schéma data_latest pour simplifier la gestion
    let schema_name = schema_name.unwrap_or_else(|| "data_latest".to_string());
    info!("Utilisation du schéma: {}", schema_name);
    
    // Créer la connexion à la base de données
    let pool = PgPool::connect(&database_url).await?;
    
    // Vérifier si le schéma existe déjà
    let schema_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM pg_namespace WHERE nspname = $1)"
    )
    .bind(&schema_name)
    .fetch_one(&pool)
    .await?;

    if schema_exists {
        // Supprimer le schéma existant
        sqlx::query(&format!(
            "DROP SCHEMA IF EXISTS {} CASCADE",
            schema_name
        ))
        .execute(&pool)
        .await?;
    }
    
    // Créer le schéma
    sqlx::query(&format!(
        "CREATE SCHEMA IF NOT EXISTS {}",
        schema_name
    ))
    .execute(&pool)
    .await?;

    // Créer les tables
    create_tables(&pool, &schema_name).await?;

    let mut total_rows_imported = 0;
    let mut total_errors = 0;
    let mut error_messages = Vec::new();

    if import_mode == "single" {
        // Mode fichier unique
        if files_content.len() > 1 {
            return Err("Mode fichier unique sélectionné mais plusieurs fichiers fournis".into());
        }
        let (rows_imported, errors, errors_list) = process_single_file(&pool, &schema_name, &files_content[0], &source).await?;
        total_rows_imported += rows_imported;
        total_errors += errors;
        error_messages.extend(errors_list);
    } else {
        // Mode fichiers multiples
        for content in files_content {
            let (rows_imported, errors, errors_list) = process_multiple_files(&pool, &schema_name, &content, &source).await?;
            total_rows_imported += rows_imported;
            total_errors += errors;
            error_messages.extend(errors_list);
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
                errors: error_messages,
            })).into_response());
        }
    }

    Ok((StatusCode::OK, Json(CsvImportResponse {
        message: format!("Import réussi avec {} erreurs", total_errors),
        rows_imported: total_rows_imported,
        errors: error_messages,
    })).into_response())
}

async fn create_tables(pool: &PgPool, schema_name: &str) -> Result<(), Box<dyn Error>> {
    // Utiliser la même fonction de création de tables que pour la collecte
    // pour assurer la cohérence de la structure
    crate::routes::collect::database::create_collection_tables(pool, schema_name)
        .await
        .map_err(|e| Box::new(e) as Box<dyn Error>)
}

// Modification de la fonction process_batch pour utiliser la nouvelle signature
async fn process_batch(
    pool: &PgPool,
    schema_name: &str,
    batch: Vec<Vec<String>>,
    source: &Option<String>,
) -> Result<(usize, usize, Vec<String>), Box<dyn Error>> {
    let mut rows_imported = 0;
    let mut errors = 0;
    let mut error_messages = Vec::new();
    let mut transaction = pool.begin().await?;

    for fields in batch {
        let str_fields: Vec<&str> = fields.iter().map(|s| s.as_str()).collect();
        match process_tweet_record(&mut transaction, schema_name, &str_fields, source).await {
            Ok(_) => rows_imported += 1,
            Err(e) => {
                errors += 1;
                error_messages.push(e.to_string());
            }
        }
    }

    transaction.commit().await?;
    Ok((rows_imported, errors, error_messages))
}

// Modification de la fonction process_single_file pour utiliser le traitement par lots
async fn process_single_file(pool: &PgPool, schema_name: &str, content: &str, source: &Option<String>) -> Result<(usize, usize, Vec<String>), Box<dyn Error>> {
    let mut rows_imported = 0;
    let mut errors = 0;
    let mut error_messages = Vec::new();
    let mut first_line = true;
    let mut _headers = Vec::new();
    let mut current_batch = Vec::new();
    const BATCH_SIZE: usize = 1000;
    
    // Déterminer la source
    let _source_type = source.as_deref().unwrap_or("twitter");
    
    for line in content.lines() {
        if first_line {
            debug!("En-tête CSV: {}", line);
            _headers = line.split(',').map(|h| h.trim_matches('"').to_string()).collect();
            first_line = false;
            continue;
        }

        let mut rdr = csv::ReaderBuilder::new()
            .has_headers(false)
            .from_reader(line.as_bytes());
        
        if let Some(result) = rdr.records().next() {
            match result {
                Ok(record) => {
                    let fields: Vec<String> = record.iter().map(|s| s.to_string()).collect();
                    current_batch.push(fields);

                    // Traiter le lot quand il atteint la taille maximale
                    if current_batch.len() >= BATCH_SIZE {
                        let (batch_rows, batch_errors, batch_messages) = 
                            process_batch(pool, schema_name, current_batch, source).await?;
                        rows_imported += batch_rows;
                        errors += batch_errors;
                        error_messages.extend(batch_messages);
                        current_batch = Vec::new();
                    }
                },
                Err(e) => {
                    error!("Erreur lors de la lecture de la ligne: {}", e);
                    errors += 1;
                    error_messages.push(format!("Erreur lors de la lecture de la ligne: {}", e));
                }
            }
        }
    }

    // Traiter le dernier lot s'il reste des données
    if !current_batch.is_empty() {
        let (batch_rows, batch_errors, batch_messages) = 
            process_batch(pool, schema_name, current_batch, source).await?;
        rows_imported += batch_rows;
        errors += batch_errors;
        error_messages.extend(batch_messages);
    }

    Ok((rows_imported, errors, error_messages))
}

async fn process_multiple_files(pool: &PgPool, schema_name: &str, content: &str, source: &Option<String>) -> Result<(usize, usize, Vec<String>), Box<dyn Error>> {
    let mut rows_imported = 0;
    let mut errors = 0;
    let mut first_line = true;
    let mut table_name = String::new();
    let mut error_messages = Vec::new();
    
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
                            error_messages.push(format!("Type de table inconnu: {}", table_name));
                        }
                    }
                },
                Err(e) => {
                    error!("Erreur lors de la lecture de la ligne: {}", e);
                    errors += 1;
                    error_messages.push(format!("Erreur lors de la lecture de la ligne: {}", e));
                }
            }
        }
    }

    // Créer une transaction pour toutes les opérations
    let mut transaction = pool.begin().await?;

    // Insérer les données dans l'ordre correct
    // 1. D'abord les tweets
    for fields in tweets_to_insert {
        let str_fields: Vec<&str> = fields.iter().map(|s| s.as_str()).collect();
        match process_tweet_record(&mut transaction, schema_name, &str_fields, source).await {
            Ok(_) => rows_imported += 1,
            Err(e) => {
                error!("Erreur lors du traitement du tweet: {}", e);
                errors += 1;
                error_messages.push(e.to_string());
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
            .execute(&mut *transaction)
            .await {
                Ok(_) => rows_imported += 1,
                Err(e) => {
                    error!("Erreur lors de l'insertion du hashtag: {}", e);
                    errors += 1;
                    error_messages.push(e.to_string());
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
            .execute(&mut *transaction)
            .await {
                Ok(_) => rows_imported += 1,
                Err(e) => {
                    error!("Erreur lors de l'insertion de l'URL: {}", e);
                    errors += 1;
                    error_messages.push(e.to_string());
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
            .execute(&mut *transaction)
            .await {
                Ok(_) => rows_imported += 1,
                Err(e) => {
                    error!("Erreur lors de l'insertion du retweet: {}", e);
                    errors += 1;
                    error_messages.push(e.to_string());
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
            .execute(&mut *transaction)
            .await {
                Ok(_) => rows_imported += 1,
                Err(e) => {
                    error!("Erreur lors de l'insertion de la réponse: {}", e);
                    errors += 1;
                    error_messages.push(e.to_string());
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
            .execute(&mut *transaction)
            .await {
                Ok(_) => rows_imported += 1,
                Err(e) => {
                    error!("Erreur lors de l'insertion de la citation: {}", e);
                    errors += 1;
                    error_messages.push(e.to_string());
                }
            }
        }
    }

    // Valider la transaction
    transaction.commit().await?;

    Ok((rows_imported, errors, error_messages))
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

// Ajout de la fonction de validation des données
fn validate_and_sanitize_data(field: &str) -> Result<String, String> {
    // Vérification des caractères dangereux
    if field.contains(";") || field.contains("--") || field.contains("/*") || field.contains("*/") {
        return Err("Caractères non autorisés détectés".to_string());
    }

    // Nettoyage des caractères spéciaux
    let sanitized = field
        .replace("'", "''") // Échapper les apostrophes
        .replace("\"", "\\\"") // Échapper les guillemets
        .trim()
        .to_string();

    // Vérification de la longueur
    if sanitized.len() > 1000 {
        return Err("Champ trop long".to_string());
    }

    Ok(sanitized)
}

// Modification de la fonction process_tweet_record pour utiliser la validation
async fn process_tweet_record(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    schema_name: &str,
    fields: &[&str],
    source: &Option<String>
) -> Result<(), Box<dyn Error>> {
    // Validation des champs obligatoires avec valeurs par défaut
    let tweet_id = fields.get(0).map(|&s| validate_and_sanitize_data(s)).transpose()?
        .unwrap_or_else(|| format!("tweet_{}", chrono::Utc::now().timestamp_millis()));
    
    let created_at = fields.get(1)
        .and_then(|&s| chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S").ok())
        .unwrap_or_else(|| chrono::Utc::now().naive_utc());
    
    let published_time = created_at.timestamp_millis();
    
    let text = fields.get(4).map(|&s| validate_and_sanitize_data(s)).transpose()?
        .unwrap_or_else(|| String::from(""));
    
    let user_id = fields.get(1).map(|&s| validate_and_sanitize_data(s)).transpose()?
        .unwrap_or_else(|| String::from("unknown_user"));
    
    let user_name = fields.get(2).map(|&s| validate_and_sanitize_data(s)).transpose()?
        .unwrap_or_else(|| String::from("Unknown User"));
    
    let user_screen_name = fields.get(3).map(|&s| validate_and_sanitize_data(s)).transpose()?
        .unwrap_or_else(|| String::from("unknown"));
    
    let language = fields.get(8).map(|&s| validate_and_sanitize_data(s)).transpose()?
        .unwrap_or_else(|| String::from("fr"));

    // Validation des coordonnées avec valeurs par défaut
    let coordinates_longitude = fields.get(9)
        .and_then(|&s| s.parse::<f64>().ok())
        .map(|v| v.to_string());
    
    let coordinates_latitude = fields.get(10)
        .and_then(|&s| s.parse::<f64>().ok())
        .map(|v| v.to_string());

    // Validation des booléens avec valeur par défaut
    let possibly_sensitive = fields.get(11)
        .map(|&s| s.to_lowercase() == "true")
        .unwrap_or(false);

    // Validation des nombres avec valeurs par défaut
    let retweet_count = fields.get(12)
        .and_then(|&s| s.parse::<i32>().ok())
        .unwrap_or(0);
    
    let reply_count = fields.get(13)
        .and_then(|&s| s.parse::<i32>().ok())
        .unwrap_or(0);
    
    let quote_count = fields.get(14)
        .and_then(|&s| s.parse::<i32>().ok())
        .unwrap_or(0);

    // Insertion du tweet
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
    .bind(&tweet_id)
    .bind(created_at)
    .bind(published_time)
    .bind(&text)
    .bind(&user_id)
    .bind(&user_name)
    .bind(&user_screen_name)
    .bind(source.as_ref().map(|s| s.as_str()))
    .bind(&language)
    .bind(&coordinates_longitude)
    .bind(&coordinates_latitude)
    .bind(possibly_sensitive)
    .bind(retweet_count)
    .bind(reply_count)
    .bind(quote_count)
    .execute(&mut **transaction)
    .await?;

    // Traitement des hashtags et URLs par lots
    if fields.len() > 15 {
        let hashtags: Vec<String> = fields[15].split(',')
            .filter(|s| !s.is_empty() && s.starts_with("#"))
            .map(|s| validate_and_sanitize_data(s).unwrap_or_default())
            .collect();

        if !hashtags.is_empty() {
            let mut hashtag_values = Vec::new();
            for hashtag in hashtags {
                hashtag_values.push(format!("('{}', '{}')", tweet_id, hashtag));
            }
            
            if !hashtag_values.is_empty() {
                sqlx::query(&format!(
                    "INSERT INTO {}.tweet_hashtag (tweet_id, hashtag) VALUES {}",
                    schema_name,
                    hashtag_values.join(",")
                ))
                .execute(&mut **transaction)
                .await?;
            }
        }
    }

    // Validation du type de tweet
    let is_retweet = text.contains("RT @");
    let is_reply = text.starts_with("@");
    let is_quote = text.contains("https://twitter.com/");

    // Insertion des relations
    if is_retweet {
        sqlx::query(&format!(
            "INSERT INTO {}.retweet (retweeted_tweet_id) VALUES ($1)",
            schema_name
        ))
        .bind(&tweet_id)
        .execute(&mut **transaction)
        .await?;
    }

    if is_reply {
        sqlx::query(&format!(
            "INSERT INTO {}.reply (in_reply_to_tweet_id) VALUES ($1)",
            schema_name
        ))
        .bind(&tweet_id)
        .execute(&mut **transaction)
        .await?;
    }

    if is_quote {
        sqlx::query(&format!(
            "INSERT INTO {}.quote (quoted_tweet_id) VALUES ($1)",
            schema_name
        ))
        .bind(&tweet_id)
        .execute(&mut **transaction)
        .await?;
    }

  
    Ok(())
} 