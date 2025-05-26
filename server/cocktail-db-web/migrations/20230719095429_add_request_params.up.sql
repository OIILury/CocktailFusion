-- Add up migration script here
ALTER TABLE project ADD COLUMN request_params TEXT DEFAULT '[[{"data":[],"link":""}],[{"data":[],"link":"ET"}]]' NOT NULL;