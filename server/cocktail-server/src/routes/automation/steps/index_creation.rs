use std::process::Command;
use std::fs;
use tokio::time::timeout;
use tracing::{info, error, debug};

use crate::routes::automation::{
    AutomationContext,
    config::{INDEX_CREATION_TIMEOUT, CARGO_CMD},
    error::{IndexCreationError, AutomationError},
};

/// Exécute l'étape de création de l'index Tantivy
pub async fn run_index_creation(context: &AutomationContext) -> Result<(), AutomationError> {
    debug!("Début de la création de l'index Tantivy");
    
    timeout(INDEX_CREATION_TIMEOUT, async {
        // Vérifier que le chemin est correct
        let expected_path = format!("tantivy-data/{}", context.schema_name);
        if context.tantivy_dir.to_str().unwrap() != expected_path {
            error!("Le chemin de l'index Tantivy est incorrect. Attendu: {}, Reçu: {}", 
                expected_path, context.tantivy_dir.display());
            return Err(IndexCreationError::FileOperationError(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Chemin de l'index Tantivy incorrect"
            )));
        }

        // 1. Forcer la suppression de l'index Tantivy existant s'il existe
        if context.tantivy_dir.exists() {
            debug!("Suppression forcée de l'ancien index Tantivy: {}", context.tantivy_dir.display());
            match fs::remove_dir_all(&context.tantivy_dir) {
                Ok(_) => {
                    debug!("Suppression réussie du dossier: {}", context.tantivy_dir.display());
                },
                Err(e) => {
                    error!("Erreur lors de la suppression de l'ancien index: {} (code: {:?})", e, e.kind());
                    return Err(IndexCreationError::FileOperationError(e));
                }
            }
            // Attendre un court instant pour s'assurer que le système de fichiers a libéré le dossier
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }

        // 2. Vérifier que le dossier a bien été supprimé
        if context.tantivy_dir.exists() {
            error!("Le dossier n'a pas pu être supprimé: {}", context.tantivy_dir.display());
            return Err(IndexCreationError::FileOperationError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Le dossier n'a pas pu être supprimé: {}", context.tantivy_dir.display())
            )));
        }

        // 3. Créer le nouveau dossier pour l'index
        debug!("Création du nouveau dossier pour l'index: {}", context.tantivy_dir.display());
        match fs::create_dir_all(&context.tantivy_dir) {
            Ok(_) => {
                debug!("Création réussie du dossier: {}", context.tantivy_dir.display());
            },
            Err(e) => {
                error!("Erreur lors de la création du dossier de l'index: {} (code: {:?})", e, e.kind());
                return Err(IndexCreationError::FileOperationError(e));
            }
        }

        // 4. Exécuter la commande de création d'index
        debug!("Création du nouvel index dans: {}", context.tantivy_dir.display());
        let output = Command::new(CARGO_CMD)
            .current_dir(&context.workspace_dir)
            .arg("run")
            .arg("--bin")
            .arg("cocktail")
            .arg("index")
            .arg("create")
            .arg("--directory-path")
            .arg(context.tantivy_dir.to_str().unwrap())
            .output()?;

        if !output.status.success() {
            let err_msg = String::from_utf8_lossy(&output.stderr);
            error!("Échec de la création de l'index: {}", err_msg);
            return Err(IndexCreationError::CommandFailed(err_msg.to_string()));
        }

        info!("Création de l'index Tantivy terminée avec succès");
        Ok(())
    }).await??;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_run_index_creation() {
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

        // Créer les dossiers nécessaires
        fs::create_dir_all(&context.project_dir).unwrap();
        fs::create_dir_all(&context.tantivy_dir).unwrap();

        // Le test échouera car les commandes ne sont pas disponibles dans l'environnement de test
        let result = run_index_creation(&context).await;
        assert!(result.is_err());

        // Vérifier que les dossiers temporaires ont été nettoyés
        assert!(!context.project_dir.join(TEMP_INDEX_DIR).exists());
    }

    #[tokio::test]
    async fn test_run_index_creation_with_existing_dirs() {
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

        // Créer les dossiers avec du contenu
        fs::create_dir_all(&context.project_dir).unwrap();
        fs::create_dir_all(&context.tantivy_dir).unwrap();
        fs::write(context.tantivy_dir.join("test.txt"), "test").unwrap();
        fs::create_dir_all(context.project_dir.join(TEMP_INDEX_DIR)).unwrap();
        fs::write(context.project_dir.join(TEMP_INDEX_DIR).join("test.txt"), "test").unwrap();

        // Le test échouera car les commandes ne sont pas disponibles dans l'environnement de test
        let result = run_index_creation(&context).await;
        assert!(result.is_err());

        // Vérifier que les dossiers temporaires ont été nettoyés
        assert!(!context.project_dir.join(TEMP_INDEX_DIR).exists());
    }
} 