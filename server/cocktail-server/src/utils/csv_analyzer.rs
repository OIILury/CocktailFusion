use std::collections::HashMap;
use std::error::Error;
use std::io::Cursor;
use csv::{Reader, ReaderBuilder};
use serde::{Serialize, Deserialize};
use encoding_rs::{Encoding, UTF_8, WINDOWS_1252};
use chrono::NaiveDateTime;

#[derive(Debug, Serialize, Deserialize)]
pub struct CsvAnalysis {
    pub total_rows: usize,
    pub total_columns: usize,
    pub headers: Vec<String>,
    pub preview: Vec<HashMap<String, String>>,
    pub encoding: String,
    pub delimiter: char,
    pub potential_issues: Vec<CsvIssue>,
    pub data_types: HashMap<String, DataType>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CsvIssue {
    pub issue_type: IssueType,
    pub message: String,
    pub affected_rows: Vec<usize>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum IssueType {
    MissingValue,
    InvalidDateFormat,
    InvalidNumber,
    SpecialCharacters,
    InconsistentDataType,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub enum DataType {
    Text,
    Number,
    Date,
    Boolean,
    Unknown,
}

pub struct CsvAnalyzer {
    content: String,
    encoding: String,
    delimiter: char,
}

impl CsvAnalyzer {
    pub fn new(content: String) -> Self {
        Self {
            content,
            encoding: String::new(),
            delimiter: ',',
        }
    }

    pub fn analyze(&mut self) -> Result<CsvAnalysis, Box<dyn Error>> {
        // Détecter l'encodage
        self.detect_encoding()?;
        
        // Détecter le délimiteur
        self.detect_delimiter()?;

        // Créer un curseur pour le contenu
        let cursor = Cursor::new(self.content.as_bytes());

        // Créer le lecteur CSV avec un buffer plus petit
        let mut reader = ReaderBuilder::new()
            .delimiter(self.delimiter as u8)
            .buffer_capacity(8192) // Buffer de 8KB
            .from_reader(cursor);

        // Lire les en-têtes
        let headers = reader.headers()?.iter()
            .map(|h| h.to_string())
            .collect::<Vec<String>>();

        // Analyser les données
        let mut analysis = CsvAnalysis {
            total_rows: 0,
            total_columns: headers.len(),
            headers: headers.clone(),
            preview: Vec::with_capacity(5), // Pré-allouer pour 5 lignes
            encoding: self.encoding.clone(),
            delimiter: self.delimiter,
            potential_issues: Vec::new(),
            data_types: HashMap::with_capacity(headers.len()),
        };

        // Analyser les 5 premières lignes pour la prévisualisation
        let mut record = csv::StringRecord::new();
        let mut row_count = 0;
        let mut preview_count = 0;

        // Lire les enregistrements par lots
        while reader.read_record(&mut record)? {
            row_count += 1;
            
            if preview_count < 5 {
                let mut row_data = HashMap::with_capacity(headers.len());
                for (i, field) in record.iter().enumerate() {
                    if i < headers.len() {
                        row_data.insert(headers[i].clone(), field.to_string());
                    }
                }
                analysis.preview.push(row_data);
                preview_count += 1;
            }

            // Analyser les types de données et détecter les problèmes
            self.analyze_row(&record, &headers, &mut analysis, row_count);

            // Limiter l'analyse à 1000 lignes pour les gros fichiers
            if row_count >= 1000 {
                analysis.potential_issues.push(CsvIssue {
                    issue_type: IssueType::SpecialCharacters,
                    message: "L'analyse a été limitée aux 1000 premières lignes pour des raisons de performance".to_string(),
                    affected_rows: vec![],
                });
                break;
            }
        }

        analysis.total_rows = row_count;
        Ok(analysis)
    }

    fn detect_encoding(&mut self) -> Result<(), Box<dyn Error>> {
        // Vérifier d'abord UTF-8
        if self.content.is_valid_utf8() {
            self.encoding = "UTF-8".to_string();
            return Ok(());
        }

        // Essayer Windows-1252
        let (cow, _, had_errors) = WINDOWS_1252.decode(&self.content.as_bytes());
        if !had_errors {
            self.encoding = "Windows-1252".to_string();
            return Ok(());
        }

        // Par défaut, utiliser UTF-8
        self.encoding = "UTF-8".to_string();
        Ok(())
    }

    fn detect_delimiter(&mut self) -> Result<(), Box<dyn Error>> {
        let delimiters = [',', ';', '\t', '|'];
        let mut max_fields = 0;
        let mut best_delimiter = ',';

        for delimiter in delimiters.iter() {
            let mut reader = ReaderBuilder::new()
                .delimiter(*delimiter as u8)
                .from_reader(self.content.as_bytes());

            if let Ok(headers) = reader.headers() {
                let field_count = headers.len();
                if field_count > max_fields {
                    max_fields = field_count;
                    best_delimiter = *delimiter;
                }
            }
        }

        self.delimiter = best_delimiter;
        Ok(())
    }

    fn analyze_row(
        &self,
        record: &csv::StringRecord,
        headers: &[String],
        analysis: &mut CsvAnalysis,
        row_number: usize,
    ) {
        for (i, field) in record.iter().enumerate() {
            if i >= headers.len() {
                continue;
            }

            let header = &headers[i];
            
            // Vérifier les valeurs manquantes
            if field.trim().is_empty() {
                analysis.potential_issues.push(CsvIssue {
                    issue_type: IssueType::MissingValue,
                    message: format!("Valeur manquante dans la colonne '{}'", header),
                    affected_rows: vec![row_number],
                });
                continue;
            }

            // Détecter le type de données
            let data_type = self.detect_data_type(field);
            
            // Mettre à jour le type de données dominant pour cette colonne
            analysis.data_types.entry(header.clone())
                .and_modify(|current_type| {
                    if *current_type == DataType::Unknown {
                        *current_type = data_type.clone();
                    }
                })
                .or_insert(data_type.clone());

            // Vérifier les problèmes spécifiques au type de données
            match data_type {
                DataType::Date => {
                    if let Err(_) = NaiveDateTime::parse_from_str(field, "%Y-%m-%d %H:%M:%S") {
                        analysis.potential_issues.push(CsvIssue {
                            issue_type: IssueType::InvalidDateFormat,
                            message: format!("Format de date invalide dans la colonne '{}'", header),
                            affected_rows: vec![row_number],
                        });
                    }
                },
                DataType::Number => {
                    if let Err(_) = field.parse::<f64>() {
                        analysis.potential_issues.push(CsvIssue {
                            issue_type: IssueType::InvalidNumber,
                            message: format!("Nombre invalide dans la colonne '{}'", header),
                            affected_rows: vec![row_number],
                        });
                    }
                },
                _ => {}
            }
        }
    }

    fn detect_data_type(&self, value: &str) -> DataType {
        // Essayer de parser comme une date
        if NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S").is_ok() {
            return DataType::Date;
        }

        // Essayer de parser comme un nombre
        if value.parse::<f64>().is_ok() {
            return DataType::Number;
        }

        // Vérifier si c'est un booléen
        if value.to_lowercase() == "true" || value.to_lowercase() == "false" {
            return DataType::Boolean;
        }

        // Par défaut, considérer comme du texte
        DataType::Text
    }
}

trait ValidUtf8 {
    fn is_valid_utf8(&self) -> bool;
}

impl ValidUtf8 for String {
    fn is_valid_utf8(&self) -> bool {
        std::str::from_utf8(self.as_bytes()).is_ok()
    }
} 