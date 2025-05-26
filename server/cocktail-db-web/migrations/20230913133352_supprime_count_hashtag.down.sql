-- Add down migration script here
ALTER TABLE project ADD COLUMN hashtag_count INTEGER NOT NULL DEFAULT(0) ;
