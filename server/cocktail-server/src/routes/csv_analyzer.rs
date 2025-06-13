use std::error::Error;
use csv::{Reader, ReaderBuilder};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::NaiveDateTime;

#[derive(Debug, Serialize, Deserialize)]
pub struct CsvAnalysis {
    pub total_rows: usize,
    pub total_columns: usize,
    pub headers: Vec<String>,
    pub separator: char,
    pub data_types: HashMap<String, DataType>,
    pub issues: Vec<Issue>,
    pub preview: Vec<HashMap<String, String>>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub enum DataType {
    Text,
    Number,
    Date,
    Boolean,
    Unknown,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Issue {
    pub row: Option<usize>,
    pub column: String,
    pub issue_type: IssueType,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum IssueType {
    MissingValue,
    InvalidDate,
    InvalidNumber,
    SpecialCharacters,
    InconsistentDataType,
}

pub struct CsvAnalyzer {
    content: String,
}

impl CsvAnalyzer {
    pub fn new(content: String) -> Self {
        Self { content }
    }

    pub fn analyze(&self) -> Result<CsvAnalysis, Box<dyn Error>> {
        // 1. Détecter le séparateur
        let separator = self.detect_separator()?;
        
        // 2. Créer le reader CSV
        let mut reader = ReaderBuilder::new()
            .delimiter(separator as u8)
            .has_headers(true)
            .from_reader(self.content.as_bytes());

        // 3. Analyser les en-têtes
        let headers = reader.headers()?.iter()
            .map(|s| s.to_string())
            .collect();

        // 4. Analyser les données
        let mut analysis = CsvAnalysis {
            total_rows: 0,
            total_columns: headers.len(),
            headers,
            separator,
            data_types: HashMap::new(),
            issues: Vec::new(),
            preview: Vec::new(),
        };

        // 5. Analyser les 5 premières lignes pour la prévisualisation
        let mut records = reader.records();
        for _ in 0..5 {
            if let Some(record) = records.next() {
                let record = record?;
                let mut row_data = HashMap::new();
                for (header, value) in analysis.headers.iter().zip(record.iter()) {
                    row_data.insert(header.clone(), value.to_string());
                    self.analyze_value(header, value, &mut analysis);
                }
                analysis.preview.push(row_data);
            }
        }

        // 6. Compter le nombre total de lignes
        analysis.total_rows = self.content.lines().count() - 1; // -1 pour l'en-tête

        Ok(analysis)
    }

    fn detect_separator(&self) -> Result<char, Box<dyn Error>> {
        let first_line = self.content.lines().next()
            .ok_or("Fichier vide")?;
        
        let separators = [',', ';', '\t'];
        let mut max_count = 0;
        let mut best_separator = ',';

        for &sep in &separators {
            let count = first_line.matches(sep).count();
            if count > max_count {
                max_count = count;
                best_separator = sep;
            }
        }

        Ok(best_separator)
    }

    fn analyze_value(&self, column: &str, value: &str, analysis: &mut CsvAnalysis) {
        // Déterminer le type de données
        let data_type = self.detect_data_type(value);
        
        // Mettre à jour le type de données pour la colonne
        analysis.data_types.entry(column.to_string())
            .and_modify(|existing_type| {
                if *existing_type != data_type {
                    analysis.issues.push(Issue {
                        row: None,
                        column: column.to_string(),
                        issue_type: IssueType::InconsistentDataType,
                        message: format!("Type de données incohérent: {} vs {}", 
                            format!("{:?}", existing_type),
                            format!("{:?}", data_type)),
                    });
                }
            })
            .or_insert(data_type);

        // Vérifier les problèmes potentiels
        if value.trim().is_empty() {
            analysis.issues.push(Issue {
                row: None,
                column: column.to_string(),
                issue_type: IssueType::MissingValue,
                message: "Valeur manquante".to_string(),
            });
        }

        // Vérifier les caractères spéciaux problématiques
        if value.contains(|c: char| c.is_control() && c != '\n' && c != '\r' && c != '\t') {
            analysis.issues.push(Issue {
                row: None,
                column: column.to_string(),
                issue_type: IssueType::SpecialCharacters,
                message: "Caractères spéciaux détectés".to_string(),
            });
        }
    }

    fn detect_data_type(&self, value: &str) -> DataType {
        if value.trim().is_empty() {
            return DataType::Unknown;
        }

        // Essayer de parser comme une date
        if NaiveDateTime::parse_from_str(value, "%Y-%m-%d %H:%M:%S").is_ok() {
            return DataType::Date;
        }

        // Essayer de parser comme un nombre
        if value.parse::<f64>().is_ok() {
            return DataType::Number;
        }

        // Vérifier si c'est un booléen
        if value.eq_ignore_ascii_case("true") || value.eq_ignore_ascii_case("false") {
            return DataType::Boolean;
        }

        // Par défaut, considérer comme du texte
        DataType::Text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_separator() {
        let content = "col1,col2,col3\n1,2,3\n4,5,6".to_string();
        let analyzer = CsvAnalyzer::new(content);
        assert_eq!(analyzer.detect_separator().unwrap(), ',');

        let content = "col1;col2;col3\n1;2;3\n4;5;6".to_string();
        let analyzer = CsvAnalyzer::new(content);
        assert_eq!(analyzer.detect_separator().unwrap(), ';');

        let content = "col1\tcol2\tcol3\n1\t2\t3\n4\t5\t6".to_string();
        let analyzer = CsvAnalyzer::new(content);
        assert_eq!(analyzer.detect_separator().unwrap(), '\t');
    }

    #[test]
    fn test_detect_data_type() {
        let analyzer = CsvAnalyzer::new("".to_string());

        assert_eq!(analyzer.detect_data_type("2023-01-01 12:00:00"), DataType::Date);
        assert_eq!(analyzer.detect_data_type("123.45"), DataType::Number);
        assert_eq!(analyzer.detect_data_type("true"), DataType::Boolean);
        assert_eq!(analyzer.detect_data_type("false"), DataType::Boolean);
        assert_eq!(analyzer.detect_data_type("Hello World"), DataType::Text);
        assert_eq!(analyzer.detect_data_type(""), DataType::Unknown);
    }

    #[test]
    fn test_analyze() {
        let content = r#"id,created_at,text,user_id,user_name,user_screen_name,source,language,coordinates_longitude,coordinates_latitude,possibly_sensitive,retweet_count,reply_count,quote_count,hashtags,urls
1,2023-01-01 12:00:00,Hello World,123,John Doe,@johndoe,Twitter,en,1.23,4.56,false,0,0,0,#hello #world,https://example.com
2,2023-01-01 12:01:00,RT @user Hello,456,Jane Doe,@janedoe,Twitter,en,,,false,1,0,0,#retweet,https://twitter.com
3,2023-01-01 12:02:00,Test message,789,Bob Smith,@bobsmith,Twitter,en,7.89,0.12,false,0,1,0,#test,https://test.com"#.to_string();

        let analyzer = CsvAnalyzer::new(content);
        let analysis = analyzer.analyze().unwrap();

        assert_eq!(analysis.total_rows, 3);
        assert_eq!(analysis.total_columns, 16);
        assert_eq!(analysis.separator, ',');
        assert_eq!(analysis.headers.len(), 16);
        assert_eq!(analysis.preview.len(), 3);

        // Vérifier les types de données
        assert_eq!(analysis.data_types.get("id").unwrap(), &DataType::Number);
        assert_eq!(analysis.data_types.get("created_at").unwrap(), &DataType::Date);
        assert_eq!(analysis.data_types.get("text").unwrap(), &DataType::Text);
        assert_eq!(analysis.data_types.get("possibly_sensitive").unwrap(), &DataType::Boolean);

        // Vérifier les problèmes potentiels
        let missing_values = analysis.issues.iter()
            .filter(|issue| matches!(issue.issue_type, IssueType::MissingValue))
            .count();
        assert_eq!(missing_values, 2); // Les coordonnées manquantes dans la ligne 2
    }
} 