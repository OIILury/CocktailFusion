use std::time::Duration;

/// Timeouts pour chaque étape de l'automatisation
pub const EXPORT_TIMEOUT: Duration = Duration::from_secs(300); // 5 minutes
pub const INDEX_CREATION_TIMEOUT: Duration = Duration::from_secs(300); // 5 minutes
pub const INGESTION_TIMEOUT: Duration = Duration::from_secs(300); // 5 minutes
pub const TOPK_TIMEOUT: Duration = Duration::from_secs(300); // 5 minutes
pub const COOCCURRENCE_TIMEOUT: Duration = Duration::from_secs(300); // 5 minutes

/// Configuration des chemins de fichiers
#[allow(dead_code)]
pub const PROJECT_DATA_DIR: &str = "project-data";

#[allow(dead_code)]
pub const TANTIVY_DATA_DIR: &str = "tantivy-data";

#[allow(dead_code)]
pub const FULL_TEXT_DATA_DIR: &str = "full-text-data";

#[allow(dead_code)]
pub const TEMP_INDEX_DIR: &str = "temp-index";

/// Configuration des noms de fichiers
pub const TOPK_DB_FILE: &str = "topk.db";
pub const HASHTAG_TABLE: &str = "hashtag";
pub const COOCCURRENCE_TABLE: &str = "hashtag_cooccurence";

/// Configuration des commandes externes
pub const CARGO_CMD: &str = "cargo";
pub const GZIP_CMD: &str = "gzip";
pub const SQLITE3_CMD: &str = "sqlite3";

/// Configuration des images Docker
#[allow(dead_code)]
pub const SQLITE_UTILS_IMAGE: &str = "ghcr.io/williamjacksn/sqlite-utils:latest";

/// Configuration des variables d'environnement requises
#[allow(dead_code)]
pub const REQUIRED_ENV_VARS: &[&str] = &["PG_DATABASE_URL"];

/// Structure de configuration pour l'automatisation
#[derive(Debug, Clone)]
pub struct AutomationConfig {
    pub export_timeout: Duration,
    pub index_creation_timeout: Duration,
    pub ingestion_timeout: Duration,
    pub topk_timeout: Duration,
    pub cooccurrence_timeout: Duration,
}

impl Default for AutomationConfig {
    fn default() -> Self {
        Self {
            export_timeout: EXPORT_TIMEOUT,
            index_creation_timeout: INDEX_CREATION_TIMEOUT,
            ingestion_timeout: INGESTION_TIMEOUT,
            topk_timeout: TOPK_TIMEOUT,
            cooccurrence_timeout: COOCCURRENCE_TIMEOUT,
        }
    }
}

/// Vérifie que toutes les variables d'environnement requises sont présentes
pub fn check_required_env_vars() -> Result<(), String> {
    for var in REQUIRED_ENV_VARS {
        if std::env::var(var).is_err() {
            return Err(format!("La variable d'environnement {} n'est pas définie", var));
        }
    }
    Ok(())
} 