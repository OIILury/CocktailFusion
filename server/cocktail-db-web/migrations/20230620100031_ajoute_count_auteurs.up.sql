-- Add up migration script here
ALTER TABLE project ADD COLUMN authors_count INTEGER NOT NULL DEFAULT(0) ;
