use std::{fs, path::Path};

use tantivy::{
  schema::{Schema, FAST, INDEXED, STORED, STRING, TEXT},
  Index, TantivyError,
};

pub fn create_index_config<P: AsRef<Path>>(directory_path: P) -> Result<(), TantivyError> {
  fs::create_dir(&directory_path)?;
  let mut schema_builder = Schema::builder();

  schema_builder.add_text_field("id", STRING | FAST | STORED);
  schema_builder.add_date_field("published_time", INDEXED | FAST | STORED);
  schema_builder.add_u64_field("published_time_ms", INDEXED | FAST | STORED);
  schema_builder.add_u64_field("asc_published_time_ms", INDEXED | FAST | STORED);
  schema_builder.add_text_field("user_id", STRING | FAST | STORED);
  schema_builder.add_text_field("user_name", STRING | STORED);
  schema_builder.add_text_field("user_screen_name", STRING | STORED);
  schema_builder.add_text_field("text", TEXT | STORED);
  schema_builder.add_text_field("urls", TEXT | STORED);
  schema_builder.add_text_field("hashtags", STRING | FAST | STORED);
  schema_builder.add_u64_field("retweet_count", INDEXED | FAST | STORED);
  schema_builder.add_u64_field("reply_count", INDEXED | FAST | STORED);
  schema_builder.add_u64_field("quote_count", INDEXED | FAST | STORED);
  schema_builder.add_u64_field("engagement_count", INDEXED | FAST | STORED);
  schema_builder.add_u64_field("asc_retweet_count", INDEXED | FAST | STORED);
  schema_builder.add_u64_field("asc_reply_count", INDEXED | FAST | STORED);
  schema_builder.add_u64_field("asc_quote_count", INDEXED | FAST | STORED);
  schema_builder.add_u64_field("asc_engagement_count", INDEXED | FAST | STORED);

  let schema = schema_builder.build();

  Index::create_in_dir(directory_path, schema)?;

  Ok(())
}
