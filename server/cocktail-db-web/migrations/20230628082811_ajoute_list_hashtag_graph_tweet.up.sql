-- Add up migration script here
ALTER TABLE project ADD COLUMN hidden_hashtag_tweets_graph_list TEXT DEFAULT '[]' NOT NULL;
