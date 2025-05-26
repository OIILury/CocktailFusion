use std::process::Command;
use tokio::time::timeout;
use tracing::{info, error, debug};

use crate::routes::automation::{
    AutomationContext,
    config::{COOCCURRENCE_TIMEOUT, CARGO_CMD, SQLITE3_CMD, COOCCURRENCE_TABLE, TOPK_DB_FILE},
    error::{CooccurrenceError, AutomationError},
};

/// Exécute l'étape de calcul des cooccurrences de hashtags
pub async fn run_cooccurrence(context: &AutomationContext) -> Result<(), AutomationError> {
    debug!("Début du calcul des cooccurrences");
    
    timeout(COOCCURRENCE_TIMEOUT, async {
        // Création de la table hashtag_cooccurence
        debug!("Création de la table hashtag_cooccurence");
        let create_table_cmd = format!(
            "{} {} 'CREATE TABLE IF NOT EXISTS {} (hashtag1 TEXT NOT NULL, hashtag2 TEXT NOT NULL, count INTEGER NOT NULL, PRIMARY KEY (hashtag1, hashtag2));'",
            SQLITE3_CMD, TOPK_DB_FILE, COOCCURRENCE_TABLE
        );

        let output = Command::new("sh")
            .arg("-c")
            .arg(&create_table_cmd)
            .current_dir(&context.workspace_dir)
            .output()?;

        if !output.status.success() {
            let err_msg = String::from_utf8_lossy(&output.stderr);
            error!("Échec de la création de la table hashtag_cooccurence: {}", err_msg);
            return Err(CooccurrenceError::DatabaseError(err_msg.to_string()));
        }

        // Calcul des cooccurrences
        debug!("Calcul des cooccurrences");
        let cmd = format!(
            "{} run --bin topk_cooccurence -- --pg-database-url {} --schema {} | \
             docker run -i --rm --user \"$(id -u):$(id -g)\" -v \"$PWD\":/usr/src/myapp -w /usr/src/myapp ghcr.io/williamjacksn/sqlite-utils:latest insert --not-null hashtag1 --not-null hashtag2 --not-null count {} {} -",
            CARGO_CMD,
            context.database_url,
            context.schema_name,
            TOPK_DB_FILE,
            COOCCURRENCE_TABLE
        );

        let output = Command::new("sh")
            .arg("-c")
            .arg(&cmd)
            .current_dir(&context.workspace_dir)
            .output()?;

        if !output.status.success() {
            let err_msg = String::from_utf8_lossy(&output.stderr);
            error!("Échec du calcul des cooccurrences: {}", err_msg);
            return Err(CooccurrenceError::CommandFailed(err_msg.to_string()));
        }

        info!("Calcul des cooccurrences terminé avec succès");
        Ok(())
    }).await??;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_run_cooccurrence() {
        // Créer un contexte temporaire pour les tests
        let temp_dir = tempdir().unwrap();
        let context = AutomationContext {
            schema_name: "test_schema".to_string(),
            workspace_dir: temp_dir.path().to_path_buf(),
            project_dir: temp_dir.path().join("project-data/test_schema"),
            tantivy_dir: temp_dir.path().join("project-data/test_schema/tantivy-data"),
            database_url: "postgres://test:test@localhost:5432/test".to_string(),
            date_str: chrono::Local::now().format("%Y_%m_%d").to_string(),
            gzip_file: format!("tweets_collecte_{}.json.gz", chrono::Local::now().format("%Y_%m_%d")),
        };

        // Le test échouera car les commandes ne sont pas disponibles dans l'environnement de test
        let result = run_cooccurrence(&context).await;
        assert!(result.is_err());
    }
} 