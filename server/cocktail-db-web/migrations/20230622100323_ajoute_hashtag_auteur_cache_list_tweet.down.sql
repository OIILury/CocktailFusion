-- Add down migration script here
ALTER TABLE project DROP COLUMN hidden_hashtag_tweets_list;
ALTER TABLE project DROP COLUMN hidden_author_tweets_list;
