use std::path::Path;

use tantivy::{doc, Index, TantivyError};

use crate::Tweet;

pub fn copy_index_data(directory_path: &Path, tweets: Vec<Tweet>) -> Result<(), TantivyError> {
  let index = Index::open_in_dir(directory_path)?;
  let schema = index.schema();
  let id = schema.get_field("id").unwrap();
  let published_time = schema.get_field("published_time").unwrap();
  let published_time_ms = schema.get_field("published_time_ms").unwrap();
  let asc_published_time_ms = schema.get_field("asc_published_time_ms").unwrap();
  let user_id = schema.get_field("user_id").unwrap();
  let user_name = schema.get_field("user_name").unwrap();
  let user_screen_name = schema.get_field("user_screen_name").unwrap();
  let text = schema.get_field("text").unwrap();
  let urls = schema.get_field("urls").unwrap();
  let hashtags = schema.get_field("hashtags").unwrap();
  let retweet_count = schema.get_field("retweet_count").unwrap();
  let reply_count = schema.get_field("reply_count").unwrap();
  let quote_count = schema.get_field("quote_count").unwrap();
  let engagement_count = schema.get_field("engagement_count").unwrap();
  let asc_retweet_count = schema.get_field("asc_retweet_count").unwrap();
  let asc_reply_count = schema.get_field("asc_reply_count").unwrap();
  let asc_quote_count = schema.get_field("asc_quote_count").unwrap();
  let asc_engagement_count = schema.get_field("asc_engagement_count").unwrap();

  let mut index_writer = index.writer(125_000_000)?;
  let mut cpt = 0u64;
  for tweet in tweets {
    let mut document = doc!(
        id => tweet.id,
        user_id => tweet.user_id,
        user_name => tweet.user_name,
        user_screen_name => tweet.user_screen_name,
        text => tweet.text,
        published_time => tantivy::DateTime::from_unix_timestamp((tweet.published_time_ms / 1_000).try_into().unwrap()),
        published_time_ms => tweet.published_time_ms,
        asc_published_time_ms => u64::MAX - tweet.published_time_ms,
        retweet_count => tweet.retweet_count,
        reply_count => tweet.reply_count,
        quote_count => tweet.quote_count,
        engagement_count =>  tweet.retweet_count + tweet.reply_count + tweet.quote_count,
        asc_retweet_count => u64::MAX - tweet.retweet_count,
        asc_reply_count => u64::MAX - tweet.reply_count,
        asc_quote_count => u64::MAX - tweet.quote_count,
        asc_engagement_count => u64::MAX - tweet.retweet_count - tweet.reply_count - tweet.quote_count,
    );
    tweet
      .hashtags
      .iter()
      .for_each(|f| document.add_text(hashtags, f));

    tweet.urls.iter().for_each(|f| document.add_text(urls, f));

    let _ = index_writer.add_document(document);
    cpt += 1;
    if cpt % 100_000 == 0 {
      index_writer.commit()?;
    }
  }
  index_writer.commit()?;

  Ok(())
}
