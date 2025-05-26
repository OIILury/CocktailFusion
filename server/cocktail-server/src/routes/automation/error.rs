use std::fmt;
use std::io;
use std::env::VarError;
use tokio::time::error::Elapsed;
use thiserror::Error;
use sqlx;

/// Erreurs spécifiques à l'étape d'export
#[derive(Debug)]
pub enum ExportError {
    CommandFailed(String),
    FileWriteError(io::Error),
    CompressionError(io::Error),
    TimeoutError,
}

impl From<io::Error> for ExportError {
    fn from(error: io::Error) -> Self {
        ExportError::FileWriteError(error)
    }
}

/// Erreurs spécifiques à l'étape de nettoyage
#[derive(Debug)]
pub enum CleanupError {
    DirectoryRemovalError(io::Error),
    DirectoryCreationError(io::Error),
}

/// Erreurs spécifiques à l'étape de création d'index
#[derive(Debug)]
pub enum IndexCreationError {
    CommandFailed(String),
    FileOperationError(io::Error),
    TimeoutError,
}

impl From<io::Error> for IndexCreationError {
    fn from(error: io::Error) -> Self {
        IndexCreationError::FileOperationError(error)
    }
}

/// Erreurs spécifiques à l'étape d'ingestion
#[derive(Debug)]
pub enum IngestionError {
    CommandFailed(String),
    TimeoutError,
}

impl From<io::Error> for IngestionError {
    fn from(error: io::Error) -> Self {
        IngestionError::CommandFailed(error.to_string())
    }
}

/// Erreurs spécifiques à l'étape des top hashtags
#[derive(Debug)]
pub enum TopHashtagsError {
    CommandFailed(String),
    DatabaseError(String),
    TimeoutError,
    InvalidJson,
}

impl From<io::Error> for TopHashtagsError {
    fn from(error: io::Error) -> Self {
        TopHashtagsError::CommandFailed(error.to_string())
    }
}

/// Erreurs spécifiques à l'étape des cooccurrences
#[derive(Debug)]
pub enum CooccurrenceError {
    CommandFailed(String),
    DatabaseError(String),
    TimeoutError,
}

impl From<io::Error> for CooccurrenceError {
    fn from(error: io::Error) -> Self {
        CooccurrenceError::CommandFailed(error.to_string())
    }
}

/// Erreurs spécifiques à l'étape de copie de schémas
#[derive(Debug, thiserror::Error)]
pub enum SchemaCopyError {
    #[error("Erreur de connexion à la base de données: {0}")]
    DatabaseError(#[from] sqlx::Error),
    
    #[error("Format de schéma invalide: {0}")]
    InvalidSchemaName(String),
    
    #[error("Impossible de déterminer l'ID du projet")]
    ProjectIdNotFound,
}

/// Erreur principale de l'automatisation
#[derive(Debug)]
pub enum AutomationError {
    ExportError(ExportError),
    CleanupError(CleanupError),
    IndexCreationError(IndexCreationError),
    IngestionError(IngestionError),
    TopHashtagsError(TopHashtagsError),
    CooccurrenceError(CooccurrenceError),
    ConfigurationError(String),
    EnvironmentError(String),
    IoError(io::Error),
    TimeoutError,
    SchemaCopyError(SchemaCopyError),
}

impl From<VarError> for AutomationError {
    fn from(error: VarError) -> Self {
        AutomationError::EnvironmentError(error.to_string())
    }
}

impl From<Elapsed> for AutomationError {
    fn from(_: Elapsed) -> Self {
        AutomationError::TimeoutError
    }
}

impl fmt::Display for AutomationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AutomationError::ExportError(e) => write!(f, "Erreur d'export : {:?}", e),
            AutomationError::CleanupError(e) => write!(f, "Erreur de nettoyage : {:?}", e),
            AutomationError::IndexCreationError(e) => write!(f, "Erreur de création d'index : {:?}", e),
            AutomationError::IngestionError(e) => write!(f, "Erreur d'ingestion : {:?}", e),
            AutomationError::TopHashtagsError(e) => write!(f, "Erreur de génération des top hashtags : {:?}", e),
            AutomationError::CooccurrenceError(e) => write!(f, "Erreur de calcul des cooccurrences : {:?}", e),
            AutomationError::ConfigurationError(e) => write!(f, "Erreur de configuration : {}", e),
            AutomationError::EnvironmentError(e) => write!(f, "Erreur d'environnement : {}", e),
            AutomationError::IoError(e) => write!(f, "Erreur d'entrée/sortie : {}", e),
            AutomationError::TimeoutError => write!(f, "Erreur de timeout"),
            AutomationError::SchemaCopyError(e) => write!(f, "Erreur de copie de schéma : {:?}", e),
        }
    }
}

impl std::error::Error for AutomationError {}

impl From<io::Error> for AutomationError {
    fn from(error: io::Error) -> Self {
        AutomationError::IoError(error)
    }
}

impl From<ExportError> for AutomationError {
    fn from(error: ExportError) -> Self {
        AutomationError::ExportError(error)
    }
}

impl From<CleanupError> for AutomationError {
    fn from(error: CleanupError) -> Self {
        AutomationError::CleanupError(error)
    }
}

impl From<IndexCreationError> for AutomationError {
    fn from(error: IndexCreationError) -> Self {
        AutomationError::IndexCreationError(error)
    }
}

impl From<IngestionError> for AutomationError {
    fn from(error: IngestionError) -> Self {
        AutomationError::IngestionError(error)
    }
}

impl From<TopHashtagsError> for AutomationError {
    fn from(error: TopHashtagsError) -> Self {
        AutomationError::TopHashtagsError(error)
    }
}

impl From<CooccurrenceError> for AutomationError {
    fn from(error: CooccurrenceError) -> Self {
        AutomationError::CooccurrenceError(error)
    }
}

impl From<SchemaCopyError> for AutomationError {
    fn from(error: SchemaCopyError) -> Self {
        AutomationError::SchemaCopyError(error)
    }
}

impl From<sqlx::Error> for AutomationError {
    fn from(error: sqlx::Error) -> Self {
        AutomationError::SchemaCopyError(SchemaCopyError::DatabaseError(error))
    }
} 