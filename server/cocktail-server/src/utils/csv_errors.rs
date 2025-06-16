use thiserror::Error;

#[derive(Error, Debug)]
pub enum CsvAnalyzerError {
    #[error("Erreur de lecture du fichier CSV: {0}")]
    CsvReadError(#[from] csv::Error),

    #[error("Erreur d'encodage: {0}")]
    EncodingError(String),

    #[error("Format de fichier invalide: {0}")]
    InvalidFormat(String),

    #[error("Erreur de détection du délimiteur: {0}")]
    DelimiterDetectionError(String),

    #[error("Erreur de validation des données: {0}")]
    ValidationError(String),

    #[error("Erreur de conversion de type: {0}")]
    TypeConversionError(String),

    #[error("Erreur de format de date: {0}")]
    DateFormatError(String),

    #[error("Erreur de format de nombre: {0}")]
    NumberFormatError(String),

    #[error("Erreur inattendue: {0}")]
    UnexpectedError(String),
}

impl From<std::io::Error> for CsvAnalyzerError {
    fn from(error: std::io::Error) -> Self {
        CsvAnalyzerError::UnexpectedError(error.to_string())
    }
}

impl From<std::str::Utf8Error> for CsvAnalyzerError {
    fn from(error: std::str::Utf8Error) -> Self {
        CsvAnalyzerError::EncodingError(error.to_string())
    }
}

impl From<chrono::ParseError> for CsvAnalyzerError {
    fn from(error: chrono::ParseError) -> Self {
        CsvAnalyzerError::DateFormatError(error.to_string())
    }
}

impl From<std::num::ParseFloatError> for CsvAnalyzerError {
    fn from(error: std::num::ParseFloatError) -> Self {
        CsvAnalyzerError::NumberFormatError(error.to_string())
    }
} 