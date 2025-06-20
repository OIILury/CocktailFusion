#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_detect_delimiter() {
        let content = "id,name,age\n1,John,30\n2,Jane,25".to_string();
        let mut analyzer = CsvAnalyzer::new(content);
        analyzer.detect_delimiter().unwrap();
        assert_eq!(analyzer.delimiter, ',');

        let content = "id;name;age\n1;John;30\n2;Jane;25".to_string();
        let mut analyzer = CsvAnalyzer::new(content);
        analyzer.detect_delimiter().unwrap();
        assert_eq!(analyzer.delimiter, ';');
    }

    #[test]
    fn test_detect_encoding() {
        let content = "id,name,age\n1,John,30\n2,Jane,25".to_string();
        let mut analyzer = CsvAnalyzer::new(content);
        analyzer.detect_encoding().unwrap();
        assert_eq!(analyzer.encoding, "UTF-8");
    }

    #[test]
    fn test_analyze_basic_csv() {
        let content = "id,name,age,created_at\n1,John,30,2024-01-01 12:00:00\n2,Jane,25,2024-01-02 13:00:00".to_string();
        let mut analyzer = CsvAnalyzer::new(content);
        let analysis = analyzer.analyze().unwrap();

        assert_eq!(analysis.total_rows, 2);
        assert_eq!(analysis.total_columns, 4);
        assert_eq!(analysis.headers, vec!["id", "name", "age", "created_at"]);
        assert_eq!(analysis.preview.len(), 2);
        assert_eq!(analysis.potential_issues.len(), 0);
    }

    #[test]
    fn test_analyze_csv_with_issues() {
        let content = "id,name,age,created_at\n1,,30,invalid_date\n2,Jane,invalid_number,2024-01-02 13:00:00".to_string();
        let mut analyzer = CsvAnalyzer::new(content);
        let analysis = analyzer.analyze().unwrap();

        assert_eq!(analysis.total_rows, 2);
        assert_eq!(analysis.total_columns, 4);
        assert!(analysis.potential_issues.len() > 0);

        let missing_value_issues: Vec<&CsvIssue> = analysis.potential_issues
            .iter()
            .filter(|issue| matches!(issue.issue_type, IssueType::MissingValue))
            .collect();
        assert!(!missing_value_issues.is_empty());

        let invalid_date_issues: Vec<&CsvIssue> = analysis.potential_issues
            .iter()
            .filter(|issue| matches!(issue.issue_type, IssueType::InvalidDateFormat))
            .collect();
        assert!(!invalid_date_issues.is_empty());

        let invalid_number_issues: Vec<&CsvIssue> = analysis.potential_issues
            .iter()
            .filter(|issue| matches!(issue.issue_type, IssueType::InvalidNumber))
            .collect();
        assert!(!invalid_number_issues.is_empty());
    }

    #[test]
    fn test_detect_data_types() {
        let content = "text,number,date,boolean\nHello,123,2024-01-01 12:00:00,true\nWorld,456,2024-01-02 13:00:00,false".to_string();
        let mut analyzer = CsvAnalyzer::new(content);
        let analysis = analyzer.analyze().unwrap();

        assert_eq!(analysis.data_types.get("text"), Some(&DataType::Text));
        assert_eq!(analysis.data_types.get("number"), Some(&DataType::Number));
        assert_eq!(analysis.data_types.get("date"), Some(&DataType::Date));
        assert_eq!(analysis.data_types.get("boolean"), Some(&DataType::Boolean));
    }

    #[test]
    fn test_preview_limit() {
        let content = "id,name\n1,John\n2,Jane\n3,Bob\n4,Alice\n5,Charlie\n6,David".to_string();
        let mut analyzer = CsvAnalyzer::new(content);
        let analysis = analyzer.analyze().unwrap();

        assert_eq!(analysis.total_rows, 6);
        assert_eq!(analysis.preview.len(), 5); // Seulement les 5 premières lignes
    }
} 