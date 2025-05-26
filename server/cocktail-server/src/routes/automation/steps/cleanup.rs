use std::fs;
use tracing::{info, error, debug};

use crate::routes::automation::{
    AutomationContext,
    error::{CleanupError, AutomationError},
};

/// Exécute l'étape de nettoyage des anciens index
pub async fn run_cleanup(context: &AutomationContext) -> Result<(), AutomationError> {
    debug!("Début du nettoyage des anciens index");

    // Supprimer les anciens dossiers
    let full_text_path = context.workspace_dir.join(format!("full-text-data/{}", context.schema_name));
    let tantivy_path = context.workspace_dir.join(format!("tantivy-data/{}", context.schema_name));

    // Supprimer le dossier full-text-data s'il existe
    if full_text_path.exists() {
        debug!("Suppression du dossier full-text-data");
        if let Err(e) = fs::remove_dir_all(&full_text_path) {
            error!("Erreur lors de la suppression du dossier full-text-data: {}", e);
            return Err(CleanupError::DirectoryRemovalError(e).into());
        }
    }

    // Supprimer le dossier tantivy-data s'il existe
    if tantivy_path.exists() {
        debug!("Suppression du dossier tantivy-data");
        if let Err(e) = fs::remove_dir_all(&tantivy_path) {
            error!("Erreur lors de la suppression du dossier tantivy-data: {}", e);
            return Err(CleanupError::DirectoryRemovalError(e).into());
        }
    }

    // Créer le dossier du projet s'il n'existe pas
    debug!("Création du dossier du projet");
    if let Err(e) = fs::create_dir_all(&context.project_dir) {
        error!("Erreur lors de la création du dossier du projet: {}", e);
        return Err(CleanupError::DirectoryCreationError(e).into());
    }

    // Créer le dossier tantivy-data
    debug!("Création du dossier tantivy-data");
    if let Err(e) = fs::create_dir_all(&context.tantivy_dir) {
        error!("Erreur lors de la création du dossier tantivy-data: {}", e);
        return Err(CleanupError::DirectoryCreationError(e).into());
    }

    info!("Nettoyage des anciens index terminé avec succès");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_run_cleanup() {
        // Créer un contexte temporaire pour les tests
        let temp_dir = tempdir().unwrap();
        let context = AutomationContext {
            schema_name: "test_schema".to_string(),
            workspace_dir: temp_dir.path().to_path_buf(),
            project_dir: temp_dir.path().join("tantivy-data/test_schema"),
            tantivy_dir: std::path::PathBuf::from("tantivy-data/test_schema"),
            database_url: "postgres://test:test@localhost:5432/test".to_string(),
            date_str: chrono::Local::now().format("%Y_%m_%d").to_string(),
            gzip_file: format!("tweets_collecte_{}.json.gz", chrono::Local::now().format("%Y_%m_%d")),
        };

        // Créer des dossiers de test
        fs::create_dir_all(&context.project_dir).unwrap();
        fs::create_dir_all(&context.tantivy_dir).unwrap();
        fs::create_dir_all(context.workspace_dir.join(format!("full-text-data/{}", context.schema_name))).unwrap();

        // Exécuter le nettoyage
        let result = run_cleanup(&context).await;
        assert!(result.is_ok());

        // Vérifier que les dossiers ont été recréés
        assert!(context.project_dir.exists());
        assert!(context.tantivy_dir.exists());
        assert!(!context.workspace_dir.join(format!("full-text-data/{}", context.schema_name)).exists());
    }
} 