-- Add up migration script here
ALTER TABLE project ADD user_id TEXT DEFAULT '' NOT NULL;
