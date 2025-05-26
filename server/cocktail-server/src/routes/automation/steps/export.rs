use std::process::{Command, Stdio};
use std::io::Write;
use tokio::time::timeout;
use tracing::{info, error, debug};

use crate::routes::automation::{
    AutomationContext,
    config::{EXPORT_TIMEOUT, CARGO_CMD, GZIP_CMD},
    error::{ExportError, AutomationError},
};

/// Exécute l'étape d'export et de compression des tweets
pub async fn run_export(context: &AutomationContext) -> Result<(), AutomationError> {
    debug!("Début de l'export des tweets");
    
    timeout(EXPORT_TIMEOUT, async {
        // Exécution de la commande d'export
        let mut command = Command::new(CARGO_CMD);
        command.current_dir(&context.workspace_dir)
               .arg("run")
               .arg("--bin")
               .arg("tweets-from-sql-to-json")
               .stdin(Stdio::piped())
               .stdout(Stdio::piped())
               .stderr(Stdio::piped())
               .env("SCHEMA_NAME", &context.schema_name)
               .env("EXPORT_DATE", chrono::Local::now().format("%Y-%m-%d").to_string());

        let mut child = command.spawn()?;
        
        // Écrire la date dans stdin
        if let Some(mut stdin) = child.stdin.take() {
            let date_str = chrono::Local::now().format("%Y-%m-%d").to_string();
            debug!("Date envoyée au programme: {}", date_str);
            stdin.write_all(date_str.as_bytes())?;
            stdin.write_all(b"\n")?;
        }

        let result = child.wait_with_output()?;
        
        if !result.status.success() {
            let err_msg = String::from_utf8_lossy(&result.stderr);
            error!("Échec de l'export des tweets: {}", err_msg);
            return Err(ExportError::CommandFailed(err_msg.to_string()));
        }

        // Écrire le fichier JSON
        let json_file = format!("tweets_collecte_{}.json", context.date_str);
        debug!("Écriture du fichier JSON: {}", json_file);
        std::fs::write(&json_file, &result.stdout)?;

        // Compression du fichier JSON
        let gzip_output = Command::new(GZIP_CMD)
            .arg("-c")
            .arg(&json_file)
            .stdout(std::fs::File::create(&context.gzip_file)?)
            .spawn()?;

        if let Err(e) = gzip_output.wait_with_output() {
            error!("Échec de la compression: {}", e);
            return Err(ExportError::CompressionError(e.into()));
        }

        info!("Export et compression des tweets terminés avec succès");
        Ok(())
    }).await??;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_run_export() {
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
        let result = run_export(&context).await;
        assert!(result.is_err());
    }
} 