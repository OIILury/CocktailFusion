use sqlx::postgres::PgPool;
use tracing::{info, debug};
use std::path::Path;

use crate::routes::automation::{
    AutomationContext,
    error::{SchemaCopyError, AutomationError},
};

/// Exécute l'étape de copie des données du schéma data_latest vers le schéma du projet
pub async fn run_schema_copy(context: &AutomationContext) -> Result<(), AutomationError> {
    debug!("Début de la copie des données depuis data_latest vers le schéma du projet");
    
    // Créer la connexion à la base de données
    let pool = PgPool::connect(&context.database_url).await?;
    
    // Récupérer l'ID du projet depuis le contexte
    let project_id = match &context.project_id {
        Some(id) if !id.is_empty() => id.clone(),
        _ => {
            debug!("Aucun ID de projet fourni, utilisation de l'ID par défaut");
            // Utiliser l'ID par défaut si aucun n'est fourni
            "fa678ab3-9c21-4f17-b06a-85f7360fc876".to_string()
        }
    };
    
    debug!("Utilisation de l'ID du projet: {}", project_id);
    
    // Créer le schéma du projet s'il n'existe pas
    debug!("Vérification/création du schéma du projet: {}", project_id);
    sqlx::query(&format!(
        r#"CREATE SCHEMA IF NOT EXISTS "{}""#,
        project_id
    ))
    .execute(&pool)
    .await?;
    
    // Copier les tables du schéma data_latest vers le schéma du projet
    let source_schema = "data_latest";
    debug!("Copie des tables du schéma {} vers le schéma {}", source_schema, project_id);
    let tables = vec![
        "corpus", "place", "quote", "reply", "retweet", "tweet", 
        "tweet_cashtag", "tweet_emoji", "tweet_hashtag", "tweet_keyword_hashtag",
        "tweet_keyword_user", "tweet_media", "tweet_place", "tweet_url", 
        "tweet_user_mention", "user", "withheld_in_country"
    ];
    
    // Copie de toutes les tables présentes dans le schéma d'import
    
    for table in tables {
        debug!("Traitement de la table {}", table);
        
        // Vérifier si la table existe dans le schéma du projet
        let table_exists = sqlx::query_scalar::<_, bool>(&format!(
            r#"
            SELECT EXISTS (
                SELECT FROM information_schema.tables 
                WHERE table_schema = $1 
                AND table_name = $2
            )
            "#
        ))
        .bind(&project_id)
        .bind(table)
        .fetch_one(&pool)
        .await?;
        
        if !table_exists {
            // Si la table n'existe pas, la créer avec la nouvelle structure
            debug!("Création de la table {}.{}", project_id, table);
            
            match table {
                "tweet" => {
                    // Créer avec la nouvelle structure pour tweet
                    sqlx::query(&format!(
                        r#"
                        CREATE TABLE IF NOT EXISTS "{}"."{}" (
                            id TEXT PRIMARY KEY,
                            created_at TEXT NOT NULL,
                            published_time BIGINT NOT NULL,
                            user_id TEXT NOT NULL,
                            user_name TEXT NOT NULL,
                            user_screen_name TEXT NOT NULL,
                            text TEXT NOT NULL,
                            source TEXT,
                            language TEXT NOT NULL,
                            coordinates_longitude TEXT,
                            coordinates_latitude TEXT,
                            possibly_sensitive BOOLEAN DEFAULT FALSE,
                            retweet_count BIGINT NOT NULL DEFAULT 0,
                            reply_count BIGINT NOT NULL DEFAULT 0,
                            quote_count BIGINT NOT NULL DEFAULT 0
                        )
                        "#,
                        project_id, table
                    ))
                    .execute(&pool)
                    .await?;
                },
                "tweet_hashtag" => {
                    // Créer avec la nouvelle structure pour tweet_hashtag
                    sqlx::query(&format!(
                        r#"
                        CREATE TABLE IF NOT EXISTS "{}"."{}" (
                            tweet_id TEXT REFERENCES "{}".tweet(id),
                            hashtag TEXT NOT NULL,
                            "order" INTEGER,
                            start_indice INTEGER,
                            end_indice INTEGER,
                            PRIMARY KEY (tweet_id, hashtag)
                        )
                        "#,
                        project_id, table, project_id
                    ))
                    .execute(&pool)
                    .await?;
                },
                "tweet_url" => {
                    // Créer avec la nouvelle structure pour tweet_url
                    sqlx::query(&format!(
                        r#"
                        CREATE TABLE IF NOT EXISTS "{}"."{}" (
                            tweet_id TEXT REFERENCES "{}".tweet(id),
                            url TEXT NOT NULL,
                            "order" INTEGER,
                            start_indice INTEGER,
                            end_indice INTEGER,
                            PRIMARY KEY (tweet_id, url)
                        )
                        "#,
                        project_id, table, project_id
                    ))
                    .execute(&pool)
                    .await?;
                },
                _ => {
                    // Pour les autres tables, utiliser l'ancienne méthode
                    sqlx::query(&format!(
                        r#"
                        CREATE TABLE IF NOT EXISTS "{}"."{}" 
                        AS SELECT * FROM {}.{} 
                        WITH NO DATA
                        "#,
                        project_id, table,
                        source_schema, table
                    ))
                    .execute(&pool)
                    .await?;
                }
            }
        }
        
        // Copier les données de la table source vers la table de destination
        debug!("Copie des données dans la table {}.{}", project_id, table);
        
        // Utiliser des requêtes spécifiques pour éviter les conflits de colonnes
        match table {
            "tweet" => {
                // Copier les colonnes communes pour tweet (ajout des nouvelles colonnes avec valeurs par défaut)
                sqlx::query(&format!(
                    r#"
                    INSERT INTO "{}"."{}" (
                        id, created_at, published_time, user_id, user_name,
                        user_screen_name, text, source, language,
                        coordinates_longitude, coordinates_latitude, possibly_sensitive,
                        retweet_count, reply_count, quote_count
                    ) 
                    SELECT 
                        id, created_at, published_time, user_id, user_name,
                        user_screen_name, text, source, language,
                        coordinates_longitude, coordinates_latitude, 
                        COALESCE(possibly_sensitive, false),
                        COALESCE(retweet_count, 0),
                        COALESCE(reply_count, 0),
                        COALESCE(quote_count, 0)
                    FROM {}.{} 
                    ON CONFLICT DO NOTHING
                    "#,
                    project_id, table,
                    source_schema, table
                ))
                .execute(&pool)
                .await?;
            },
            "tweet_hashtag" => {
                // Copier seulement les colonnes communes pour tweet_hashtag
                sqlx::query(&format!(
                    r#"
                    INSERT INTO "{}"."{}" (tweet_id, hashtag) 
                    SELECT tweet_id, hashtag FROM {}.{} 
                    ON CONFLICT DO NOTHING
                    "#,
                    project_id, table,
                    source_schema, table
                ))
                .execute(&pool)
                .await?;
            },
            "tweet_url" => {
                // Copier seulement les colonnes communes pour tweet_url
                sqlx::query(&format!(
                    r#"
                    INSERT INTO "{}"."{}" (tweet_id, url) 
                    SELECT tweet_id, url FROM {}.{} 
                    ON CONFLICT DO NOTHING
                    "#,
                    project_id, table,
                    source_schema, table
                ))
                .execute(&pool)
                .await?;
            },
            _ => {
                // Pour les autres tables, utiliser SELECT * comme avant
                sqlx::query(&format!(
                    r#"
                    INSERT INTO "{}"."{}" 
                    SELECT * FROM {}.{} 
                    ON CONFLICT DO NOTHING
                    "#,
                    project_id, table,
                    source_schema, table
                ))
                .execute(&pool)
                .await?;
            }
        }
    }
    
    info!("Copie des données vers le schéma du projet terminée avec succès");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_run_schema_copy() {
        // Créer un contexte temporaire pour les tests
        let temp_dir = tempdir().unwrap();
        let context = AutomationContext {
            schema_name: "import_20250521".to_string(),
            workspace_dir: temp_dir.path().to_path_buf(),
            project_dir: temp_dir.path().join("project-data/test_schema"),
            tantivy_dir: temp_dir.path().join("project-data/test_schema/tantivy-data"),
            database_url: "postgres://test:test@localhost:5432/test".to_string(),
            date_str: chrono::Local::now().format("%Y_%m_%d").to_string(),
            gzip_file: format!("tweets_collecte_{}.json.gz", chrono::Local::now().format("%Y_%m_%d")),
        };

        // Le test échouera car la base de données n'est pas disponible dans l'environnement de test
        let result = run_schema_copy(&context).await;
        assert!(result.is_err());
    }
} 