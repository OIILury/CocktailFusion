-- Add down migration script here
ALTER TABLE project ADD included_exact_keywords TEXT DEFAULT '' NOT NULL;
ALTER TABLE project ADD included_exact_group_keywords TEXT DEFAULT '' NOT NULL;
ALTER TABLE project ADD included_any_keywords TEXT DEFAULT '' NOT NULL;
ALTER TABLE project ADD excluded_any_keywords TEXT DEFAULT '' NOT NULL;

ALTER TABLE project ADD included_accounts TEXT DEFAULT '' NOT NULL;
ALTER TABLE project ADD excluded_accounts TEXT DEFAULT '' NOT NULL;
