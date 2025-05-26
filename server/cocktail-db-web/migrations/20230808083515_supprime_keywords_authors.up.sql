-- Add up migration script here
-- Supprime les mots clefs et les auteurs du projets suite a l'ajout du requeteurs premium
ALTER TABLE project DROP COLUMN included_exact_keywords;
ALTER TABLE project DROP COLUMN included_exact_group_keywords;
ALTER TABLE project DROP COLUMN included_any_keywords;
ALTER TABLE project DROP COLUMN excluded_any_keywords;

ALTER TABLE project DROP COLUMN included_accounts;
ALTER TABLE project DROP COLUMN excluded_accounts;
