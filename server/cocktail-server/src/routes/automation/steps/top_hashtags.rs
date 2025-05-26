use std::process::Command;
use std::fs;
use tokio::time::timeout;
use tracing::{info, error, debug};
use serde_json;
use tantivy::Index;

use crate::routes::automation::{
    AutomationContext,
    config::{TOPK_TIMEOUT, SQLITE3_CMD, TOPK_DB_FILE, HASHTAG_TABLE},
    error::{TopHashtagsError, AutomationError},
};

/// Exécute l'étape de génération des top hashtags
pub async fn run_top_hashtags(context: &AutomationContext) -> Result<(), AutomationError> {
    debug!("Début de la génération des top hashtags");
    
    timeout(TOPK_TIMEOUT, async {
        // Vérification du répertoire de travail
        if !context.workspace_dir.exists() {
            fs::create_dir_all(&context.workspace_dir)?;
        }

        // Vérification de l'existence du binaire topk
        let topk_path = context.workspace_dir.join("target/debug/topk");
        if !topk_path.exists() {
            error!("Le binaire topk n'existe pas dans {}", topk_path.display());
            return Err(TopHashtagsError::CommandFailed("Binaire topk non trouvé".to_string()));
        }
        debug!("Binaire topk trouvé dans {}", topk_path.display());

        // Vérification de l'index Tantivy
        let index = Index::open_in_dir(&context.tantivy_dir)
            .map_err(|e| TopHashtagsError::CommandFailed(format!("Erreur d'ouverture de l'index: {}", e)))?;
        
        let reader = index.reader()
            .map_err(|e| TopHashtagsError::CommandFailed(format!("Erreur de lecture de l'index: {}", e)))?;
        
        let num_docs = reader.searcher().num_docs();
        debug!("Nombre de documents dans l'index: {}", num_docs);
        
        if num_docs == 0 {
            error!("L'index Tantivy est vide");
            return Err(TopHashtagsError::CommandFailed("L'index Tantivy est vide".to_string()));
        }

        // Supprimer l'ancien fichier topk.db s'il existe
        let db_path = context.workspace_dir.join("topk.db");
        if db_path.exists() {
            fs::remove_file(&db_path)?;
        }

        // Création de la table hashtag
        debug!("Création de la table hashtag");
        let create_table_cmd = format!(
            "{} {} 'CREATE TABLE IF NOT EXISTS {} (key TEXT PRIMARY KEY, doc_count INTEGER NOT NULL);'",
            SQLITE3_CMD, TOPK_DB_FILE, HASHTAG_TABLE
        );

        let output = Command::new("sh")
            .arg("-c")
            .arg(&create_table_cmd)
            .current_dir(&context.workspace_dir)
            .output()?;

        if !output.status.success() {
            let err_msg = String::from_utf8_lossy(&output.stderr);
            error!("Échec de la création de la table hashtag: {}", err_msg);
            return Err(TopHashtagsError::DatabaseError(err_msg.to_string()));
        }

        // Exécuter topk et récupérer sa sortie
        let output = Command::new("sh")
            .arg("-c")
            .arg(format!(
                "cd {} && rm -f topk.db && ./target/debug/topk --directory-path {} --query '*' | docker run -i --rm --user \"$(id -u):$(id -g)\" -v \"$PWD\":/usr/src/myapp -w /usr/src/myapp ghcr.io/williamjacksn/sqlite-utils:latest insert --not-null key --not-null doc_count topk.db hashtag -",
                context.workspace_dir.display(),
                context.tantivy_dir.display()
            ))
            .output()?;

        if !output.status.success() {
            let err_msg = String::from_utf8_lossy(&output.stderr);
            error!("Échec de la commande topk: {}", err_msg);
            return Err(TopHashtagsError::CommandFailed(err_msg.to_string()));
        }

        let topk_stdout = String::from_utf8_lossy(&output.stdout);
        debug!("Sortie de topk: {}", topk_stdout);

        info!("Génération des top hashtags terminée avec succès");
        Ok(())
    }).await??;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_run_top_hashtags() {
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
        let result = run_top_hashtags(&context).await;
        assert!(result.is_err());
    }
} 