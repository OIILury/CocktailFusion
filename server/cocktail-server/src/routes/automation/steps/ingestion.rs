use std::process::Command;
use tokio::time::timeout;
use tracing::{info, error, debug};

use crate::routes::automation::{
    AutomationContext,
    config::{INGESTION_TIMEOUT, CARGO_CMD},
    error::{IngestionError, AutomationError},
};

/// Exécute l'étape d'ingestion des tweets dans l'index Tantivy
pub async fn run_ingestion(context: &AutomationContext) -> Result<(), AutomationError> {
    debug!("Début de l'ingestion des tweets");
    
    timeout(INGESTION_TIMEOUT, async {
        // Construction de la commande d'ingestion
        let cmd = format!(
            "gunzip -c {} | {} run --bin cocktail index ingest --directory-path {}",
            context.gzip_file,
            CARGO_CMD,
            context.tantivy_dir.display()
        );

        debug!("Exécution de la commande d'ingestion: {}", cmd);
        let output = Command::new("sh")
            .arg("-c")
            .arg(&cmd)
            .current_dir(&context.workspace_dir)
            .output()?;

        if !output.status.success() {
            let err_msg = String::from_utf8_lossy(&output.stderr);
            error!("Échec de l'ingestion des tweets: {}", err_msg);
            return Err(IngestionError::CommandFailed(err_msg.to_string()));
        }

        info!("Ingestion des tweets terminée avec succès");
        Ok(())
    }).await??;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_run_ingestion() {
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
        let result = run_ingestion(&context).await;
        assert!(result.is_err());
    }
} 