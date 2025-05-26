-- Add up migration script here
ALTER TABLE project ADD COLUMN hidden_hashtag_tweets_list TEXT DEFAULT '[]' NOT NULL;
ALTER TABLE project ADD COLUMN hidden_author_tweets_list TEXT DEFAULT '[]' NOT NULL;

