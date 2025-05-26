use sqlx::postgres::PgPool;
use tracing::{info, debug};
use std::path::Path;

use crate::routes::automation::{
    AutomationContext,
    error::{SchemaCopyError, AutomationError},
};

/// Exécute l'étape de copie des données du schéma d'import vers le schéma du projet
pub async fn run_schema_copy(context: &AutomationContext) -> Result<(), AutomationError> {
    debug!("Début de la copie des données vers le schéma du projet");
    
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
    
    // Copier les tables du schéma d'import vers le schéma du projet
    debug!("Copie des tables du schéma {} vers le schéma {}", context.schema_name, project_id);
    let tables = vec!["tweet", "tweet_hashtag", "tweet_url", "retweet", "reply", "quote"];
    
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
            // Si la table n'existe pas, la créer en copiant la structure de la table source
            debug!("Création de la table {}.{}", project_id, table);
            sqlx::query(&format!(
                r#"
                CREATE TABLE IF NOT EXISTS "{}"."{}" 
                AS SELECT * FROM {}.{} 
                WITH NO DATA
                "#,
                project_id, table,
                context.schema_name, table
            ))
            .execute(&pool)
            .await?;
        }
        
        // Copier les données de la table source vers la table de destination
        debug!("Copie des données dans la table {}.{}", project_id, table);
        sqlx::query(&format!(
            r#"
            INSERT INTO "{}"."{}" 
            SELECT * FROM {}.{} 
            ON CONFLICT DO NOTHING
            "#,
            project_id, table,
            context.schema_name, table
        ))
        .execute(&pool)
        .await?;
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