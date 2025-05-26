use std::{
  io::{stdin, BufRead, BufReader},
  path::Path,
};

use serde::Deserialize;
use tantivy::{doc, Index, TantivyError};

pub fn ingest<P: AsRef<Path>>(directory_path: P) -> Result<(), TantivyError> {
  let index = Index::open_in_dir(directory_path)?;
  let schema = index.schema();
  let id = schema.get_field("id").unwrap();
  let published_time = schema.get_field("published_time").unwrap();
  let published_time_ms = schema.get_field("published_time_ms").unwrap();
  let user_id = schema.get_field("user_id").unwrap();
  let user_name = schema.get_field("user_name").unwrap();
  let user_screen_name = schema.get_field("user_screen_name").unwrap();
  let text = schema.get_field("text").unwrap();
  let urls = schema.get_field("urls").unwrap();
  let hashtags = schema.get_field("hashtags").unwrap();
  let retweet_count = schema.get_field("retweet_count").unwrap();
  let reply_count = schema.get_field("reply_count").unwrap();
  let quote_count = schema.get_field("quote_count").unwrap();

  let rdr = BufReader::new(stdin());
  let mut index_writer = index.writer(125_000_000)?;
  let mut cpt = 0u64;
  for json_line in rdr.lines() {
    let tweet: Tweet = serde_json::from_str(&json_line?)?;
    let mut document = doc!(
        id => tweet.id,
        user_id => tweet.user_id,
        user_name => tweet.user_name,
        user_screen_name => tweet.user_screen_name,
        text => tweet.text,
        published_time => tantivy::DateTime::from_unix_timestamp(tweet.published_time / 1_000),
        published_time_ms => tweet.published_time as u64,
        retweet_count => tweet.retweet_count,
        reply_count => tweet.reply_count,
        quote_count => tweet.quote_count
    );
    tweet
      .hashtags
      .unwrap_or_default()
      .iter()
      .for_each(|f| document.add_text(hashtags, f));

    tweet
      .urls
      .unwrap_or_default()
      .iter()
      .for_each(|f| document.add_text(urls, f));
    let _ = index_writer.add_document(document);
    cpt += 1;
    if cpt % 100_000 == 0 {
      index_writer.commit()?;
    }
  }
  index_writer.commit()?;

  Ok(())
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct Tweet {
  id: String,
  #[serde(skip)]
  created_at: String,
  published_time: i64,
  user_id: String,
  user_name: String,
  user_screen_name: String,
  text: String,
  #[serde(skip)]
  source: Option<String>,
  #[serde(skip)]
  language: String,
  #[serde(skip)]
  coordinates_longitude: Option<String>,
  #[serde(skip)]
  coordinates_latitude: Option<String>,
  #[serde(skip)]
  possibly_sensitive: Option<bool>,
  hashtags: Option<Vec<String>>,
  urls: Option<Vec<String>>,
  retweet_count: u64,
  #[serde(default)]
  reply_count: u64,
  #[serde(default)]
  quote_count: u64,
}
