use std::{fmt, path::Path, vec, collections::HashMap};

use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use serde::Serializer;
use serde::{Deserialize, Deserializer, Serialize};
use tantivy::aggregation::agg_result::BucketEntry;
use tantivy::aggregation::bucket::HistogramBounds;
use tantivy::query::PhraseQuery;
use tantivy::schema::Schema;
use tantivy::LeasedItem;
use tantivy::Searcher;
use tantivy::{
  aggregation::{
    agg_req::{Aggregation, Aggregations, BucketAggregation, BucketAggregationType},
    bucket::{CustomOrder, HistogramAggregation, Order, TermsAggregation},
    AggregationCollector,
  },
  collector::{Count, TopDocs},
  query::{
    AllQuery, BooleanQuery, FuzzyTermQuery, Occur, Query, QueryParser, QueryParserError, TermQuery,
  },
  schema::{Field, IndexRecordOption},
  Document, TantivyError, Term,
};

pub use copy_index_data::*;
pub use create_index_config::*;
pub use ingest::*;
pub use tantivy::{DocAddress, Index};

pub mod copy_index_data;
pub mod create_index_config;
pub mod ingest;

use sqlx::Decode;

#[derive(Debug, Clone, Decode, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Bloc {
  pub data: Vec<String>,
  pub link: String,
}

#[derive(Debug, thiserror::Error)]
pub enum SearchError {
  #[error("La recherche ne retourne rien")]
  NotFound,
  #[error("Impossible de parser la query")]
  ParseError,
}

impl From<TantivyError> for SearchError {
  fn from(source: TantivyError) -> Self {
    tracing::error!("{source:?}");
    SearchError::NotFound
  }
}

impl From<QueryParserError> for SearchError {
  fn from(_: QueryParserError) -> Self {
    SearchError::NotFound
  }
}

impl From<serde_json::Error> for SearchError {
  fn from(_: serde_json::Error) -> Self {
    SearchError::ParseError
  }
}

pub const MAX: u32 = 200_000_000;

#[derive(Debug, Deserialize)]
pub enum OrderBy {
  PublishedTime,
  RetweetCount,
  ReplyCount,
  QuoteCount,
  EngagementCount,
}

impl fmt::Display for OrderBy {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      OrderBy::PublishedTime => write!(f, "PublishedTime"),
      OrderBy::RetweetCount => write!(f, "RetweetCount"),
      OrderBy::ReplyCount => write!(f, "ReplyCount"),
      OrderBy::QuoteCount => write!(f, "QuoteCount"),
      OrderBy::EngagementCount => write!(f, "EngagementCount"),
    }
  }
}

impl From<&str> for OrderBy {
  fn from(key: &str) -> Self {
    match key {
      "reponses" => OrderBy::ReplyCount,
      "citations" => OrderBy::QuoteCount,
      "retweets" => OrderBy::RetweetCount,
      "engageants" => OrderBy::EngagementCount,
      _ => OrderBy::PublishedTime,
    }
  }
}

pub fn retrieve_index<P: AsRef<Path>>(dir: P) -> Result<Index, SearchError> {
  let index = Index::open_in_dir(dir)?;

  Ok(index)
}

pub fn search_for_communities(
  index: &Index,
) -> Result<(Vec<(f32, DocAddress)>, usize), SearchError> {
  let searcher = index.reader()?.searcher();

  let query = AllQuery;

  let top_docs: (Vec<_>, usize) = {
    let top_docs_collector = TopDocs::with_limit(10_000_000);
    searcher.search(&query, &(top_docs_collector, Count))?
  };

  Ok(top_docs)
}

pub fn hashtag_search<P: AsRef<str>>(
  index: &Index,
  query: P,
  limit: usize,
) -> Result<(Vec<(f32, DocAddress)>, usize), SearchError> {
  let schema = index.schema();
  let searcher = index.reader()?.searcher();
  let hashtags = schema.get_field("hashtags").unwrap(); // unwrap car **on sait** que text est un champ dans l'index
  let term = Term::from_field_text(hashtags, query.as_ref());
  let query = FuzzyTermQuery::new(term, 1, true);
  let top_docs: (Vec<_>, usize) = {
    let top_docs_collector = TopDocs::with_limit(limit);
    searcher.search(&query, &(top_docs_collector, Count))?
  };

  let mut result: Vec<_> = top_docs
    .0
    .into_iter()
    .map(|(_score, doc_address)| {
      let doc = searcher.doc(doc_address);
      doc
        .map(|d| {
          let doc = &d;
          let field = &hashtags;
          doc
            .get_all(*field)
            .map(extract_string)
            .collect::<Vec<_>>()
            .first()
            .cloned()
            .unwrap()
        })
        .unwrap_or_default()
    })
    .collect();

  result.sort_unstable();
  result.dedup();

  // Ok(top_docs)
  Ok((vec![], 0))
}

pub fn doc_count(index: &Index, query: Option<String>) -> Result<usize, SearchError> {
  let searcher = index.reader()?.searcher();
  let query_parser = QueryParser::for_index(index, vec![]);

  if let Some(query) = query {
    let q = query_parser.parse_query(query.as_ref())?;
    let count = searcher.search(&q, &Count)?;
    Ok(count)
  } else {
    let count = searcher.search(&AllQuery, &Count)?;
    Ok(count)
  }
}

pub fn topk<T>(index: &Index, query: T, size: u32) -> Result<String, SearchError>
where
  T: AsRef<str>,
{
  let schema = index.schema();
  let searcher = index.reader()?.searcher();
  let hashtags = schema.get_field("hashtags").unwrap();
  let query_parser = QueryParser::for_index(index, vec![hashtags]);
  let q = query_parser.parse_query(query.as_ref())?;
  let agg_req: Aggregations = vec![(
    "hashtags".to_string(),
    Aggregation::Bucket(BucketAggregation {
      bucket_agg: BucketAggregationType::Terms(TermsAggregation {
        field: "hashtags".to_string(),
        order: Some(CustomOrder {
          target: tantivy::aggregation::bucket::OrderTarget::Count,
          order: Order::Desc,
        }),
        size: Some(size),
        ..Default::default()
      }),
      sub_aggregation: Default::default(),
    }),
  )]
  .into_iter()
  .collect();

  let collector = AggregationCollector::from_aggs(agg_req);
  let agg_res = searcher.search(&q, &collector)?;
  
  let buckets = if let tantivy::aggregation::agg_result::AggregationResult::BucketResult(br) =
    agg_res.0.get("hashtags").unwrap()
  {
    match br {
      tantivy::aggregation::agg_result::BucketResult::Terms {
        buckets,
        sum_other_doc_count: _,
        doc_count_error_upper_bound: _,
      } => buckets.clone(),
      _ => Vec::new(),
    }
  } else {
    Vec::new()
  };

  // Transformer les buckets en format compatible avec sqlite-utils
  #[derive(serde::Serialize)]
  struct HashtagEntry {
    key: String,
    doc_count: u64,
  }

  let formatted_buckets: Vec<HashtagEntry> = buckets
    .into_iter()
    .map(|bucket| HashtagEntry {
      key: bucket.key.to_string(),
      doc_count: bucket.doc_count,
    })
    .collect();

  let s = serde_json::to_string(&formatted_buckets)?;
  Ok(s)
}

pub fn search_tweets(
  index: &Index,
  query: &str,
  order_by: &Option<OrderBy>,
) -> Result<Vec<Tweet>, SearchError> {
  let schema = index.schema();
  let searcher = index.reader()?.searcher();
  let id = schema.get_field("id").unwrap();
  let text = schema.get_field("text").unwrap();
  let user_id = schema.get_field("user_id").unwrap();
  let user_screen_name = schema.get_field("user_screen_name").unwrap();
  let user_name = schema.get_field("user_name").unwrap();
  let published_time = schema.get_field("published_time").unwrap();
  let published_time_ms = schema.get_field("published_time_ms").unwrap();
  let retweet_count = schema.get_field("retweet_count").unwrap();
  let reply_count = schema.get_field("reply_count").unwrap();
  let quote_count = schema.get_field("quote_count").unwrap();
  let hashtags = schema.get_field("hashtags").unwrap();
  let urls = schema.get_field("urls").unwrap();

  let query_parser = QueryParser::for_index(index, vec![text]);
  let query = query_parser.parse_query(query)?;
  let search_results: Vec<_> = if let Some(order_by) = order_by {
    searcher
      .search(
        &query,
        &TopDocs::with_limit(10).order_by_fast_field::<u64>(match order_by {
          OrderBy::RetweetCount => retweet_count,
          OrderBy::ReplyCount => reply_count,
          OrderBy::QuoteCount => quote_count,
          _ => retweet_count,
        }),
      )?
      .into_iter()
      .map(|(_score, doc_address)| doc_address)
      .collect()
  } else {
    searcher
      .search(&query, &TopDocs::with_limit(10))?
      .into_iter()
      .map(|(_score, doc_address)| doc_address)
      .collect()
  };
  let tweets: Vec<_> = search_results
    .iter()
    .filter_map(|doc_address| searcher.doc(*doc_address).ok())
    .map(|doc| Tweet {
      id: extract(&doc, &id, extract_string),
      user_id: extract(&doc, &user_id, extract_string),
      user_name: extract(&doc, &user_name, extract_string),
      user_screen_name: extract(&doc, &user_screen_name, extract_string),
      text: extract(&doc, &text, extract_string),
      published_time: extract(&doc, &published_time, extract_date),
      published_time_ms: extract(&doc, &published_time_ms, extract_u64),
      retweet_count: extract(&doc, &retweet_count, extract_u64),
      reply_count: extract(&doc, &reply_count, extract_u64),
      quote_count: extract(&doc, &quote_count, extract_u64),
      hashtags: extract_vec(&doc, &hashtags, extract_string),
      urls: extract_vec(&doc, &urls, extract_string),
    })
    .collect();

  Ok(tweets)
}

pub fn search_tweets_for_analysis(
  index: &Index,
  start_date: &NaiveDate,
  end_date: &NaiveDate,
  hashtag_list: &Vec<String>,
  exclude_hashtag_list: &Vec<String>,
  request_params: &Vec<Vec<Bloc>>,
) -> Result<Vec<Tweet>, SearchError> {
  let schema = index.schema();
  let searcher = index.reader()?.searcher();
  let id = schema.get_field("id").unwrap();
  let text = schema.get_field("text").unwrap();
  let user_id = schema.get_field("user_id").unwrap();
  let user_screen_name = schema.get_field("user_screen_name").unwrap();
  let user_name = schema.get_field("user_name").unwrap();
  let published_time = schema.get_field("published_time").unwrap();
  let published_time_ms = schema.get_field("published_time_ms").unwrap();
  let retweet_count = schema.get_field("retweet_count").unwrap();
  let reply_count = schema.get_field("reply_count").unwrap();
  let quote_count = schema.get_field("quote_count").unwrap();
  let hashtags = schema.get_field("hashtags").unwrap();
  let urls = schema.get_field("urls").unwrap();

  let query = get_query(
    index,
    start_date,
    end_date,
    hashtag_list,
    exclude_hashtag_list,
    request_params,
  );

  let search_results: Vec<_> = searcher
    .search(&query, &TopDocs::with_limit(MAX as usize))?
    .into_iter()
    .map(|(_score, doc_address)| doc_address)
    .collect();

  let tweets: Vec<_> = search_results
    .iter()
    .filter_map(|doc_address| searcher.doc(*doc_address).ok())
    .map(|doc| Tweet {
      id: extract(&doc, &id, extract_string),
      user_id: extract(&doc, &user_id, extract_string),
      user_name: extract(&doc, &user_name, extract_string),
      user_screen_name: extract(&doc, &user_screen_name, extract_string),
      text: extract(&doc, &text, extract_string),
      published_time: extract(&doc, &published_time, extract_date),
      published_time_ms: extract(&doc, &published_time_ms, extract_u64),
      retweet_count: extract(&doc, &retweet_count, extract_u64),
      reply_count: extract(&doc, &reply_count, extract_u64),
      quote_count: extract(&doc, &quote_count, extract_u64),
      hashtags: extract_vec(&doc, &hashtags, extract_string),
      urls: extract_vec(&doc, &urls, extract_string),
    })
    .collect();

  Ok(tweets)
}

pub fn search_tweets_for_preview(
  index: &Index,
  start_date: &NaiveDate,
  end_date: &NaiveDate,
  hashtag_list: &Vec<String>,
  exclude_hashtag_list: &Vec<String>,
  request_params: &Vec<Vec<Bloc>>,
) -> Result<PreviewTweets, SearchError> {
  let schema = index.schema();
  let searcher = index.reader()?.searcher();
  let id = schema.get_field("id").unwrap();
  let text = schema.get_field("text").unwrap();
  let user_id = schema.get_field("user_id").unwrap();
  let user_screen_name = schema.get_field("user_screen_name").unwrap();
  let user_name = schema.get_field("user_name").unwrap();
  let published_time = schema.get_field("published_time").unwrap();
  let published_time_ms = schema.get_field("published_time_ms").unwrap();
  let retweet_count = schema.get_field("retweet_count").unwrap();
  let reply_count = schema.get_field("reply_count").unwrap();
  let quote_count = schema.get_field("quote_count").unwrap();
  let hashtags = schema.get_field("hashtags").unwrap();
  let urls = schema.get_field("urls").unwrap();

  let sort_field = schema
    .get_field("engagement_count")
    .unwrap_or(retweet_count);

  let query = get_query(
    index,
    start_date,
    end_date,
    hashtag_list,
    exclude_hashtag_list,
    request_params,
  );

  let mut search_results: Vec<_> = searcher
    .search(
      &query,
      &TopDocs::with_limit(MAX as usize).order_by_fast_field::<u64>(sort_field),
    )?
    .into_iter()
    .map(|(_score, doc_address)| doc_address)
    .collect();
  let count = search_results.len();
  search_results.truncate(10);

  let tweets: Vec<_> = search_results
    .iter()
    .filter_map(|doc_address| searcher.doc(*doc_address).ok())
    .map(|doc| Tweet {
      id: extract(&doc, &id, extract_string),
      user_id: extract(&doc, &user_id, extract_string),
      user_name: extract(&doc, &user_name, extract_string),
      user_screen_name: extract(&doc, &user_screen_name, extract_string),
      text: extract(&doc, &text, extract_string),
      published_time: extract(&doc, &published_time, extract_date),
      published_time_ms: extract(&doc, &published_time_ms, extract_u64),
      retweet_count: extract(&doc, &retweet_count, extract_u64),
      reply_count: extract(&doc, &reply_count, extract_u64),
      quote_count: extract(&doc, &quote_count, extract_u64),
      hashtags: extract_vec(&doc, &hashtags, extract_string),
      urls: extract_vec(&doc, &urls, extract_string),
    })
    .collect();

  Ok(PreviewTweets {
    count: count as i64,
    tweets,
  })
}

pub fn search_tweets_for_result(
  index: &Index,
  included_user_names: &Vec<String>,
  hidden_hashtags: &Vec<String>,
  hidden_authors: &Vec<String>,
  exclude_retweets: bool,
  order_by: OrderBy,
  order: &String,
  date: &Option<NaiveDate>,
  hashtag: &Option<String>,
  page: u32,
) -> Result<Vec<Tweet>, SearchError> {
  let schema = index.schema();
  let searcher = index.reader()?.searcher();
  let id = schema.get_field("id").unwrap();
  let text = schema.get_field("text").unwrap();
  let user_id = schema.get_field("user_id").unwrap();
  let user_screen_name = schema.get_field("user_screen_name").unwrap();
  let user_name = schema.get_field("user_name").unwrap();
  let published_time = schema.get_field("published_time").unwrap();
  let published_time_ms = schema.get_field("published_time_ms").unwrap();
  let retweet_count = schema.get_field("retweet_count").unwrap();
  let reply_count = schema.get_field("reply_count").unwrap();
  let quote_count = schema.get_field("quote_count").unwrap();
  let urls = schema.get_field("urls").unwrap();

  let sort_field = match order_by {
    OrderBy::PublishedTime => schema
      .get_field(match order.as_str() {
        "croissant" => "asc_published_time_ms",
        _ => "published_time_ms",
      })
      .unwrap(),
    OrderBy::RetweetCount => schema
      .get_field(match order.as_str() {
        "croissant" => "asc_retweet_count",
        _ => "retweet_count",
      })
      .unwrap(),
    OrderBy::ReplyCount => schema
      .get_field(match order.as_str() {
        "croissant" => "asc_reply_count",
        _ => "reply_count",
      })
      .unwrap(),
    OrderBy::QuoteCount => schema
      .get_field(match order.as_str() {
        "croissant" => "asc_quote_count",
        _ => "quote_count",
      })
      .unwrap(),
    OrderBy::EngagementCount => schema
      .get_field(match order.as_str() {
        "croissant" => "asc_engagement_count",
        _ => "engagement_count",
      })
      .unwrap(),
  };
  let hashtags = schema.get_field("hashtags").unwrap();

  let query = get_results_query(
    index,
    included_user_names,
    hidden_hashtags,
    hidden_authors,
    exclude_retweets,
    date,
    hashtag,
  );

  let offset: usize = match page {
    0 => 0,
    _ => ((page - 1) * 10).try_into().unwrap_or(0),
  };

  let search_results: Vec<_> = searcher
    .search(
      &query,
      &TopDocs::with_limit(10)
        .and_offset(offset)
        .order_by_fast_field::<u64>(sort_field),
    )?
    .into_iter()
    .map(|(_score, doc_address)| doc_address)
    .collect();

  let tweets: Vec<_> = search_results
    .iter()
    .filter_map(|doc_address| searcher.doc(*doc_address).ok())
    .map(|doc| Tweet {
      id: extract(&doc, &id, extract_string),
      user_id: extract(&doc, &user_id, extract_string),
      user_name: extract(&doc, &user_name, extract_string),
      user_screen_name: extract(&doc, &user_screen_name, extract_string),
      text: extract(&doc, &text, extract_string),
      published_time: extract(&doc, &published_time, extract_date),
      published_time_ms: extract(&doc, &published_time_ms, extract_u64),
      retweet_count: extract(&doc, &retweet_count, extract_u64),
      reply_count: extract(&doc, &reply_count, extract_u64),
      quote_count: extract(&doc, &quote_count, extract_u64),
      hashtags: extract_vec(&doc, &hashtags, extract_string),
      urls: extract_vec(&doc, &urls, extract_string),
    })
    .collect();

  Ok(tweets)
}

pub fn get_all_tweets(index: &Index) -> Result<Vec<Tweet>, SearchError> {
  let schema = index.schema();
  let searcher = index.reader()?.searcher();
  let id = schema.get_field("id").unwrap();
  let text = schema.get_field("text").unwrap();
  let user_id = schema.get_field("user_id").unwrap();
  let user_screen_name = schema.get_field("user_screen_name").unwrap();
  let user_name = schema.get_field("user_name").unwrap();
  let published_time = schema.get_field("published_time").unwrap();
  let published_time_ms = schema.get_field("published_time_ms").unwrap();
  let retweet_count = schema.get_field("retweet_count").unwrap();
  let reply_count = schema.get_field("reply_count").unwrap();
  let quote_count = schema.get_field("quote_count").unwrap();
  let hashtags = schema.get_field("hashtags").unwrap();
  let urls = schema.get_field("urls").unwrap();

  let query = AllQuery;

  let search_results: Vec<_> = searcher
    .search(&query, &TopDocs::with_limit(5000))?
    .into_iter()
    .map(|(_score, doc_address)| doc_address)
    .collect();

  let tweets: Vec<_> = search_results
    .iter()
    .filter_map(|doc_address| searcher.doc(*doc_address).ok())
    .map(|doc| Tweet {
      id: extract(&doc, &id, extract_string),
      user_id: extract(&doc, &user_id, extract_string),
      user_name: extract(&doc, &user_name, extract_string),
      user_screen_name: extract(&doc, &user_screen_name, extract_string),
      text: extract(&doc, &text, extract_string),
      published_time: extract(&doc, &published_time, extract_date),
      published_time_ms: extract(&doc, &published_time_ms, extract_u64),
      retweet_count: extract(&doc, &retweet_count, extract_u64),
      reply_count: extract(&doc, &reply_count, extract_u64),
      quote_count: extract(&doc, &quote_count, extract_u64),
      hashtags: extract_vec(&doc, &hashtags, extract_string),
      urls: extract_vec(&doc, &urls, extract_string),
    })
    .collect();

  Ok(tweets)
}

pub fn search_tweets_count_per_day(
  index: &Index,
  included_user_names: &Vec<String>,
  hidden_hashtags: &Vec<String>,
  start_date: &NaiveDate,
  end_date: &NaiveDate,
  tab: &String,
) -> Result<Vec<FrequenceByDate>, SearchError> {
  let schema = index.schema();

  let searcher = index.reader()?.searcher();
  let query = get_results_query(
    index,
    included_user_names,
    hidden_hashtags,
    &vec![],
    false,
    &None,
    &None,
  );

  let sub_agg: Aggregations = vec![(
    "tweets_id".to_string(),
    Aggregation::Bucket(BucketAggregation {
      bucket_agg: BucketAggregationType::Terms(TermsAggregation {
        field: "id".to_string(),
        size: Some(MAX),
        ..Default::default()
      }),
      sub_aggregation: Default::default(),
    }),
  )]
  .into_iter()
  .collect();

  let agg_req: Aggregations = vec![(
    "par_jour".to_string(),
    Aggregation::Bucket(BucketAggregation {
      bucket_agg: BucketAggregationType::Histogram(HistogramAggregation {
        field: "published_time_ms".to_string(),
        interval: 86_400_000f64,
        extended_bounds: Some(HistogramBounds {
          min: start_date.and_hms(00, 00, 00).timestamp_millis() as f64,
          max: end_date.and_hms(00, 00, 00).timestamp_millis() as f64,
        }),
        ..Default::default()
      }),
      sub_aggregation: sub_agg,
    }),
  )]
  .into_iter()
  .collect();

  let aggregation_collector = AggregationCollector::from_aggs(agg_req);

  let aggregation_result = searcher.search(&query, &aggregation_collector)?;
  let v = serde_json::to_value(&aggregation_result)?;
  let result: ByDay = serde_json::from_value(v)?;

  Ok(get_frequence_by_date(&schema, &searcher, result, tab))
}

pub fn aggregate_authors(
  index: &Index,
  tab: &String,
  page: u32,
) -> Result<Vec<AuthorCount>, SearchError> {
  let schema = index.schema();
  let searcher = index.reader()?.searcher();

  let query = AllQuery;

  let sub_agg: Aggregations = vec![(
    "tweets_id".to_string(),
    Aggregation::Bucket(BucketAggregation {
      bucket_agg: BucketAggregationType::Terms(TermsAggregation {
        field: "id".to_string(),
        order: Some(CustomOrder {
          target: tantivy::aggregation::bucket::OrderTarget::Count,
          order: Order::Desc,
        }),
        size: Some(MAX),
        ..Default::default()
      }),
      sub_aggregation: Default::default(),
    }),
  )]
  .into_iter()
  .collect();

  let agg_req: Aggregations = vec![(
    "authors".to_string(),
    Aggregation::Bucket(BucketAggregation {
      bucket_agg: BucketAggregationType::Terms(TermsAggregation {
        field: "user_id".to_string(),
        size: Some(match page {
          0 => 50,
          _ => page * 10,
        }),
        ..Default::default()
      }),
      sub_aggregation: sub_agg,
    }),
  )]
  .into_iter()
  .collect();

  let collector = AggregationCollector::from_aggs(agg_req);
  let aggregation_result = searcher.search(&query, &collector)?;

  let v = serde_json::to_value(&aggregation_result)?;
  let result: Authors = serde_json::from_value(v)?;
  let mut res: Vec<AuthorCount> = result
    .authors
    .buckets
    .into_iter()
    .enumerate()
    .filter_map(|(index, bucket)| {
      if page != 0
        && (index < ((page - 1) * 10).try_into().unwrap_or(0)
          || index >= (page * 10).try_into().unwrap_or(10))
      {
        return None;
      }

      let tweets_buckets = if tab.as_str() == "total" {
        let first = TweetsBucket {
          key: bucket.tweets_id.buckets.first()?.key.clone(),
        };
        vec![first]
      } else {
        bucket.tweets_id.buckets
      };
      let tweets_stats = get_tweets_stats(&schema, &searcher, tweets_buckets);
      let mut author: Option<Author> = None;
      let mut count = 0;

      for stat in tweets_stats {
        if author.is_none() {
          author = Some(Author {
            user_name: stat.user_name,
            user_screen_name: stat.user_screen_name,
          });
        }

        match tab.as_str() {
          "retweets" => {
            count += stat.retweet_count;
          }
          "citations" => {
            count += stat.quote_count;
          }
          "repondus" => {
            count += stat.reply_count;
          }
          _ => {
            return Some(AuthorCount {
              author: author.unwrap(),
              count: bucket.doc_count as u64,
            });
          }
        }
      }
      if author.is_some() && count > 0 {
        return Some(AuthorCount {
          author: author.unwrap(),
          count,
        });
      }
      None
    })
    .collect();
  res.sort_by(|a, b| b.count.cmp(&a.count));

  Ok(res)
}

pub fn get_frequence_for_hashtag(
  schema: &Schema,
  searcher: &LeasedItem<Searcher>,
  hashtags: Field,
  start_date: &NaiveDate,
  end_date: &NaiveDate,
  tab: &String,
  hashtag: &String,
) -> Result<Frequence, SearchError> {
  let query = TermQuery::new(
    Term::from_field_text(hashtags, hashtag),
    IndexRecordOption::Basic,
  );

  let sub_agg: Aggregations = vec![(
    "tweets_id".to_string(),
    Aggregation::Bucket(BucketAggregation {
      bucket_agg: BucketAggregationType::Terms(TermsAggregation {
        field: "id".to_string(),
        size: Some(MAX),
        ..Default::default()
      }),
      sub_aggregation: Default::default(),
    }),
  )]
  .into_iter()
  .collect();

  let agg_req: Aggregations = vec![(
    "par_jour".to_string(),
    Aggregation::Bucket(BucketAggregation {
      bucket_agg: BucketAggregationType::Histogram(HistogramAggregation {
        field: "published_time_ms".to_string(),
        interval: 86_400_000f64,
        extended_bounds: Some(HistogramBounds {
          min: start_date.and_hms(00, 00, 00).timestamp_millis() as f64,
          max: end_date.and_hms(00, 00, 00).timestamp_millis() as f64,
        }),
        ..Default::default()
      }),
      sub_aggregation: sub_agg,
    }),
  )]
  .into_iter()
  .collect();

  let aggregation_collector = AggregationCollector::from_aggs(agg_req);

  let aggregation_result = searcher.search(&query, &aggregation_collector)?;
  let v = serde_json::to_value(&aggregation_result)?;
  let result: ByDay = serde_json::from_value(v)?;

  Ok(Frequence {
    hashtag: hashtag.clone(),
    hidden: false,
    data: get_frequence_by_date(&schema, &searcher, result, tab),
  })
}

pub fn get_frequence_for_hashtag_cooccurence(
  schema: &Schema,
  searcher: &LeasedItem<Searcher>,
  hashtags: Field,
  start_date: &NaiveDate,
  end_date: &NaiveDate,
  tab: &String,
  cooccurence: &HashtagCooccurence,
) -> Result<FrequenceCooccurence, SearchError> {
  let mut query = Vec::new();

  let term_query1: Box<dyn Query> = Box::new(TermQuery::new(
    Term::from_field_text(hashtags, &cooccurence.hashtag1),
    IndexRecordOption::Basic,
  ));
  let term_query2: Box<dyn Query> = Box::new(TermQuery::new(
    Term::from_field_text(hashtags, &cooccurence.hashtag2),
    IndexRecordOption::Basic,
  ));
  query.push((Occur::Must, term_query1));
  query.push((Occur::Must, term_query2));

  let sub_agg: Aggregations = vec![(
    "tweets_id".to_string(),
    Aggregation::Bucket(BucketAggregation {
      bucket_agg: BucketAggregationType::Terms(TermsAggregation {
        field: "id".to_string(),
        size: Some(MAX),
        ..Default::default()
      }),
      sub_aggregation: Default::default(),
    }),
  )]
  .into_iter()
  .collect();

  let agg_req: Aggregations = vec![(
    "par_jour".to_string(),
    Aggregation::Bucket(BucketAggregation {
      bucket_agg: BucketAggregationType::Histogram(HistogramAggregation {
        field: "published_time_ms".to_string(),
        interval: 86_400_000f64,
        extended_bounds: Some(HistogramBounds {
          min: start_date.and_hms(00, 00, 00).timestamp_millis() as f64,
          max: end_date.and_hms(00, 00, 00).timestamp_millis() as f64,
        }),
        ..Default::default()
      }),
      sub_aggregation: sub_agg,
    }),
  )]
  .into_iter()
  .collect();

  let aggregation_collector = AggregationCollector::from_aggs(agg_req);

  let aggregation_result = searcher.search(&BooleanQuery::new(query), &aggregation_collector)?;
  let v = serde_json::to_value(&aggregation_result)?;
  let result: ByDay = serde_json::from_value(v)?;
  let mut label = cooccurence.hashtag1.clone();
  label.push_str("-");
  label.push_str(cooccurence.hashtag2.as_str());

  Ok(FrequenceCooccurence {
    label,
    hidden: false,
    data: get_frequence_by_date(&schema, &searcher, result, tab),
  })
}

pub fn search_study_hashtags_count_per_day(
  index: &Index,
  start_date: &NaiveDate,
  end_date: &NaiveDate,
  hashtag_list: &Vec<String>,
  tab: &String,
) -> Result<Vec<Frequence>, SearchError> {
  let searcher = index.reader()?.searcher();
  let schema = index.schema();
  let hashtags = schema.get_field("hashtags").unwrap();

  let mut v_result = vec![];

  for hashtag in hashtag_list {
    v_result.push(
      get_frequence_for_hashtag(
        &schema, &searcher, hashtags, start_date, end_date, tab, &hashtag,
      )
      .unwrap(),
    );
  }

  v_result.sort_by(|h1, h2| h1.hashtag.cmp(&h2.hashtag));

  Ok(v_result)
}

pub fn search_top_hashtags_count_per_day(
  index: &Index,
  start_date: &NaiveDate,
  end_date: &NaiveDate,
  tab: &String,
) -> Result<Vec<Frequence>, SearchError> {
  let searcher = index.reader()?.searcher();
  let schema = index.schema();
  let hashtags = schema.get_field("hashtags").unwrap();

  let mut v_result_topk = vec![];

  let topk_hastags_json = topk(index, "*", 10)?;
  let topk_hashtags = serde_json::from_str::<Vec<BucketEntry>>(topk_hastags_json.as_str())?;

  for hashtag in topk_hashtags {
    v_result_topk.push(
      get_frequence_for_hashtag(
        &schema,
        &searcher,
        hashtags,
        start_date,
        end_date,
        tab,
        &hashtag.key.to_string(),
      )
      .unwrap(),
    );
  }

  v_result_topk.sort_by(|h1, h2| h1.hashtag.cmp(&h2.hashtag));

  Ok(v_result_topk)
}

pub fn search_top_hashtags_cooccurence_count_per_day(
  index: &Index,
  start_date: &NaiveDate,
  end_date: &NaiveDate,
  tab: &String,
  cooccurences: &Vec<HashtagCooccurence>,
) -> Result<Vec<FrequenceCooccurence>, SearchError> {
  let searcher = index.reader()?.searcher();
  let schema = index.schema();
  let hashtags = schema.get_field("hashtags").unwrap();

  let mut v_result = vec![];

  for cooccurence in cooccurences {
    v_result.push(
      get_frequence_for_hashtag_cooccurence(
        &schema,
        &searcher,
        hashtags,
        start_date,
        end_date,
        tab,
        cooccurence,
      )
      .unwrap(),
    );
  }

  v_result.sort_by(|h1, h2| h1.label.cmp(&h2.label));

  Ok(v_result)
}

fn extract_string(un: &tantivy::schema::Value) -> String {
  if let tantivy::schema::Value::Str(s) = un {
    s.to_owned()
  } else {
    "".to_string()
  }
}

fn extract_date(un: &tantivy::schema::Value) -> DateTime<Utc> {
  if let tantivy::schema::Value::Date(d) = un {
    Utc.timestamp_millis(d.into_unix_timestamp() * 1000)
  } else {
    Utc::now()
  }
}

fn extract_u64(un: &tantivy::schema::Value) -> u64 {
  if let tantivy::schema::Value::U64(u) = un {
    *u
  } else {
    u64::default()
  }
}

fn extract<F, T>(doc: &Document, field: &Field, extract_fn: F) -> T
where
  F: Fn(&tantivy::schema::Value) -> T,
  T: Clone,
{
  doc
    .get_all(*field)
    .map(extract_fn)
    .collect::<Vec<_>>()
    .first()
    .cloned()
    .unwrap()
}

fn extract_vec<F, T>(doc: &Document, field: &Field, extract_fn: F) -> Vec<T>
where
  F: Fn(&tantivy::schema::Value) -> T,
  T: Clone,
{
  doc.get_all(*field).map(extract_fn).collect::<Vec<T>>()
}

fn get_query(
  index: &Index,
  start_date: &NaiveDate,
  end_date: &NaiveDate,
  hashtag_list: &Vec<String>,
  exclude_hashtag_list: &Vec<String>,
  request_params: &Vec<Vec<Bloc>>,
) -> BooleanQuery {
  let schema = index.schema();
  let text: Field = schema.get_field("text").unwrap();
  let user_screen_name = schema.get_field("user_screen_name").unwrap();
  let hashtags = schema.get_field("hashtags").unwrap();
  let query_parser = QueryParser::for_index(&index, vec![text]);
  let mut query = Vec::new();

  let mut sub_query: Vec<(Occur, Box<dyn Query>)> = Vec::new();

  if request_params[0].len() == 0 && hashtag_list.len() > 0 {
    for hashtag in hashtag_list {
      let term_query: Box<dyn Query> = Box::new(TermQuery::new(
        Term::from_field_text(hashtags, &hashtag),
        IndexRecordOption::Basic,
      ));
      sub_query.push((Occur::Should, term_query));
    }
    query.push((
      Occur::Must,
      Box::new(BooleanQuery::new(sub_query)) as Box<dyn Query>,
    ));
  }

  for hashtag in exclude_hashtag_list {
    let term_query: Box<dyn Query> = Box::new(TermQuery::new(
      Term::from_field_text(hashtags, &hashtag),
      IndexRecordOption::Basic,
    ));
    query.push((Occur::MustNot, term_query));
  }

  let date_query: Box<dyn Query> = Box::new(
    query_parser
      .parse_query(&format!(
        "published_time_ms:[{start_date_timestamp} TO {end_date_timestamp}]",
        start_date_timestamp = start_date.and_hms(0, 0, 0).timestamp_millis(),
        end_date_timestamp = end_date.and_hms(23, 59, 59).timestamp_millis(),
      ))
      .unwrap(),
  );

  query.push((Occur::Must, date_query));

  let link = request_params[1][0].link.clone();
  sub_query = Vec::new();

  if request_params[1][0].data.len() > 0 {
    let query_exclude = create_query_from_bloc(
      request_params[1][0].data.clone(),
      text,
      user_screen_name,
      hashtags,
      Occur::Should,
    );

    if link == "ET" {
      query.push((
        Occur::MustNot,
        Box::new(BooleanQuery::new(query_exclude)) as Box<dyn Query>,
      ));
    } else {
      sub_query.push((
        Occur::Should,
        Box::new(BooleanQuery::new(vec![
          (Occur::Must, Box::new(AllQuery) as Box<dyn Query>),
          (
            Occur::MustNot,
            Box::new(BooleanQuery::new(query_exclude)) as Box<dyn Query>,
          ),
        ])) as Box<dyn Query>,
      ));
    }
  }

  let mut query_requeteur = vec![];
  let mut query_bloc = vec![];

  let mut bloc_subquery = vec![];
  if request_params[0].iter().any(|r| r.data.len() > 0) {
    for bloc in &request_params[0] {
      if bloc.link == "OU" || bloc.link == "" {
        bloc_subquery.push((
          Occur::Should,
          Box::new(BooleanQuery::new(create_query_from_bloc(
            bloc.data.clone(),
            text,
            user_screen_name,
            hashtags,
            Occur::Must,
          ))) as Box<dyn Query>,
        ));
      } else {
        query_bloc.push((
          Occur::Must,
          Box::new(BooleanQuery::new(create_query_from_bloc(
            bloc.data.clone(),
            text,
            user_screen_name,
            hashtags,
            Occur::Must,
          ))) as Box<dyn Query>,
        ));

        if !bloc_subquery.is_empty() {
          query_bloc.push((
            Occur::Must,
            Box::new(BooleanQuery::new(bloc_subquery)) as Box<dyn Query>,
          ));
        }
        query_requeteur.push((
          Occur::Should,
          Box::new(BooleanQuery::new(query_bloc)) as Box<dyn Query>,
        ));
        query_bloc = vec![];
        bloc_subquery = vec![];
      }
    }

    if !bloc_subquery.is_empty() {
      query_requeteur.push((
        Occur::Should,
        Box::new(BooleanQuery::new(bloc_subquery)) as Box<dyn Query>,
      ));
    }

    query.push((
      Occur::Must,
      Box::new(BooleanQuery::new(query_requeteur)) as Box<dyn Query>,
    ));
  } else {
    //Si aucune condition, on prend tout
    query.push((Occur::Must, Box::new(AllQuery) as Box<dyn Query>))
  }
  BooleanQuery::new(query)
}

// Créé une query à partir d'un Vec de String
fn create_query_from_bloc(
  data: Vec<String>,
  text: Field,
  user_screen_name: Field,
  hashtags: Field,
  occur_type: Occur,
) -> Vec<(Occur, Box<dyn Query>)> {
  let mut query_bloc: Vec<(Occur, Box<dyn Query>)> = Vec::new();

  for element in data {
    let mut field = text;
    let value: String;

    if element.starts_with("@") {
      field = user_screen_name;
      value = element[1..].to_string();
    } else if element.starts_with("#") {
      field = hashtags;
      value = element[1..].to_string();
    } else {
      value = element.to_ascii_lowercase().clone()
    }

    if value.contains(' ') {
      //Groupe de mots
      let phrase_query: Box<dyn Query> = Box::new(PhraseQuery::new(
        value
          .split(' ')
          .map(|e| Term::from_field_text(text, e))
          .collect(),
      ));
      query_bloc.push((occur_type, phrase_query));
    } else {
      let term_query: Box<dyn Query> = Box::new(TermQuery::new(
        Term::from_field_text(field, &value),
        IndexRecordOption::Basic,
      ));
      query_bloc.push((occur_type, term_query));
    }
  }

  return query_bloc;
}

fn get_results_query(
  index: &Index,
  included_user_names: &Vec<String>,
  hidden_hashtags: &Vec<String>,
  hidden_authors: &Vec<String>,
  exclude_retweets: bool,
  date: &Option<NaiveDate>,
  hashtag: &Option<String>,
) -> BooleanQuery {
  let schema = index.schema();
  let mut query = Vec::new();

  if included_user_names.len() > 0 {
    let user_screen_name = schema.get_field("user_screen_name").unwrap();
    let mut sub_query: Vec<(Occur, Box<dyn Query>)> = Vec::new();

    for screen_name in included_user_names {
      let term_query: Box<dyn Query> = Box::new(TermQuery::new(
        Term::from_field_text(user_screen_name, &screen_name),
        IndexRecordOption::Basic,
      ));
      sub_query.push((Occur::Should, term_query));
    }

    query.push((
      Occur::Must,
      Box::new(BooleanQuery::new(sub_query)) as Box<dyn Query>,
    ));
  } else {
    query.push((Occur::Must, Box::new(AllQuery) as Box<dyn Query>));
  }

  if hidden_hashtags.len() > 0 {
    let hashtags = schema.get_field("hashtags").unwrap();

    for hashtag in hidden_hashtags {
      //Coocurence
      if hashtag.contains('-') {
        let coocurence = hashtag.split("-").collect::<Vec<&str>>();

        let hashtag1 = coocurence[0];
        let hashtag2 = coocurence[1];
        let mut sub_query: Vec<(Occur, Box<dyn Query>)> = Vec::new();

        let term_query_hashtag1: Box<dyn Query> = Box::new(TermQuery::new(
          Term::from_field_text(hashtags, &hashtag1),
          IndexRecordOption::Basic,
        ));
        let term_query_hashtag2: Box<dyn Query> = Box::new(TermQuery::new(
          Term::from_field_text(hashtags, &hashtag2),
          IndexRecordOption::Basic,
        ));

        sub_query.push((Occur::Must, term_query_hashtag1));
        sub_query.push((Occur::Must, term_query_hashtag2));

        query.push((
          Occur::MustNot,
          Box::new(BooleanQuery::new(sub_query)) as Box<dyn Query>,
        ));
      } else {
        let term_query: Box<dyn Query> = Box::new(TermQuery::new(
          Term::from_field_text(hashtags, &hashtag),
          IndexRecordOption::Basic,
        ));
        query.push((Occur::MustNot, term_query));
      }
    }
  }

  if hidden_authors.len() > 0 {
    let user_screen_name = schema.get_field("user_screen_name").unwrap();

    for author in hidden_authors {
      let term_query: Box<dyn Query> = Box::new(TermQuery::new(
        Term::from_field_text(user_screen_name, &author),
        IndexRecordOption::Basic,
      ));
      query.push((Occur::MustNot, term_query));
    }
  }

  if exclude_retweets {
    let text = schema.get_field("text").unwrap();
    let query_parser = QueryParser::for_index(&index, vec![text]);

    let rt_query: Box<dyn Query> =
      Box::new(query_parser.parse_query(&format!("text:RT @")).unwrap());

    query.push((Occur::MustNot, rt_query));
  }

  if date.is_some() {
    let published_time_ms = schema.get_field("published_time_ms").unwrap();
    let query_parser = QueryParser::for_index(&index, vec![published_time_ms]);
    let date = date.unwrap();

    let date_query: Box<dyn Query> = Box::new(
      query_parser
        .parse_query(&format!(
          "published_time_ms:[{start_date_timestamp} TO {end_date_timestamp}]",
          start_date_timestamp = date.and_hms(0, 0, 0).timestamp_millis(),
          end_date_timestamp = date.and_hms(23, 59, 59).timestamp_millis(),
        ))
        .unwrap(),
    );

    query.push((Occur::Must, date_query));
  }

  if hashtag.is_some() {
    let hashtags = schema.get_field("hashtags").unwrap();

    let term_query: Box<dyn Query> = Box::new(TermQuery::new(
      Term::from_field_text(hashtags, &hashtag.as_ref().unwrap()),
      IndexRecordOption::Basic,
    ));

    query.push((Occur::Must, term_query));
  }

  BooleanQuery::new(query)
}

fn get_tweets_stats(
  schema: &Schema,
  searcher: &LeasedItem<Searcher>,
  buckets: Vec<TweetsBucket>,
) -> Vec<TweetStats> {
  let id = schema.get_field("id").unwrap();
  let user_screen_name = schema.get_field("user_screen_name").unwrap();
  let user_name = schema.get_field("user_name").unwrap();
  let retweet_count = schema.get_field("retweet_count").unwrap();
  let reply_count = schema.get_field("reply_count").unwrap();
  let quote_count = schema.get_field("quote_count").unwrap();

  let mut query = Vec::new();

  let mut sub_query: Vec<(Occur, Box<dyn Query>)> = Vec::new();
  if buckets.len() > 0 {
    for bucket in buckets {
      let term_query: Box<dyn Query> = Box::new(TermQuery::new(
        Term::from_field_text(id, &bucket.key),
        IndexRecordOption::Basic,
      ));
      sub_query.push((Occur::Should, term_query));
    }

    query.push((
      Occur::Must,
      Box::new(BooleanQuery::new(sub_query)) as Box<dyn Query>,
    ));
  }

  let search_results: Vec<_> = searcher
    .search(
      &BooleanQuery::new(query),
      &TopDocs::with_limit(MAX as usize),
    )
    .unwrap_or_default()
    .into_iter()
    .map(|(_score, doc_address)| doc_address)
    .collect();
  search_results
    .iter()
    .filter_map(|doc_address| searcher.doc(*doc_address).ok())
    .map(|doc| TweetStats {
      user_name: extract(&doc, &user_name, extract_string),
      user_screen_name: extract(&doc, &user_screen_name, extract_string),
      retweet_count: extract(&doc, &retweet_count, extract_u64),
      reply_count: extract(&doc, &reply_count, extract_u64),
      quote_count: extract(&doc, &quote_count, extract_u64),
    })
    .collect()
}

fn get_frequence_by_date(
  schema: &Schema,
  searcher: &LeasedItem<Searcher>,
  result: ByDay,
  tab: &String,
) -> Vec<FrequenceByDate> {
  let mut data = vec![];
  for bucket in result.par_jour.buckets {
    data.push(match tab.as_str() {
      "retweets" => {
        let mut retweets = 0;

        let tweets_stats = get_tweets_stats(&schema, &searcher, bucket.tweets_id.buckets);

        for stat in tweets_stats {
          retweets += stat.retweet_count;
        }
        FrequenceByDate {
          date: bucket.key.date().naive_utc(),
          frequence: retweets,
        }
      }
      "citations" => {
        let mut quotes = 0;

        let tweets_stats = get_tweets_stats(&schema, &searcher, bucket.tweets_id.buckets);

        for stat in tweets_stats {
          quotes += stat.quote_count;
        }
        FrequenceByDate {
          date: bucket.key.date().naive_utc(),
          frequence: quotes,
        }
      }
      "repondus" => {
        let mut replies = 0;

        let tweets_stats = get_tweets_stats(&schema, &searcher, bucket.tweets_id.buckets);

        for stat in tweets_stats {
          replies += stat.reply_count;
        }
        FrequenceByDate {
          date: bucket.key.date().naive_utc(),
          frequence: replies,
        }
      }
      _ => FrequenceByDate {
        date: bucket.key.date().naive_utc(),
        frequence: bucket.doc_count,
      },
    });
  }

  data
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Tweet {
  pub id: String,
  pub user_id: String,
  pub user_name: String,
  pub user_screen_name: String,
  pub text: String,
  pub published_time: DateTime<Utc>,
  pub published_time_ms: u64,
  pub retweet_count: u64,
  pub reply_count: u64,
  pub quote_count: u64,
  pub hashtags: Vec<String>,
  pub urls: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PreviewTweets {
  pub count: i64,
  pub tweets: Vec<Tweet>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AuthorCount {
  pub author: Author,
  pub count: u64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Author {
  pub user_name: String,
  pub user_screen_name: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ByDay {
  pub par_jour: ByDayBuckets,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Authors {
  pub authors: AuthorBuckets,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ByDayBuckets {
  pub buckets: Vec<ByDayBucket>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AuthorBuckets {
  pub buckets: Vec<AuthorBucket>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ByDayBucket {
  #[serde(deserialize_with = "f64_date_de")]
  pub key: DateTime<Utc>,
  pub doc_count: u64,
  pub tweets_id: TweetsBuckets,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AuthorBucket {
  pub key: String,
  pub doc_count: i64,
  pub tweets_id: TweetsBuckets,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TweetsBuckets {
  pub buckets: Vec<TweetsBucket>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TweetsBucket {
  pub key: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TweetStats {
  pub user_name: String,
  pub user_screen_name: String,
  pub retweet_count: u64,
  pub reply_count: u64,
  pub quote_count: u64,
}

fn f64_date_de<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
where
  D: Deserializer<'de>,
{
  let n: f64 = <f64 as serde::Deserialize>::deserialize(deserializer)?;
  Ok(Utc.timestamp_millis(n as i64))
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq, Deserialize)]
pub struct Frequence {
  #[serde(rename = "label")]
  pub hashtag: String,
  pub hidden: bool,
  pub data: Vec<FrequenceByDate>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq, Deserialize)]
pub struct FrequenceCooccurence {
  pub label: String,
  pub hidden: bool,
  pub data: Vec<FrequenceByDate>,
}

#[derive(Debug, Default)]
pub struct HashtagCooccurence {
  pub hashtag1: String,
  pub hashtag2: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq, Deserialize)]
pub struct FrequenceByDate {
  #[serde(rename = "x", serialize_with = "date_fr")]
  pub date: NaiveDate,
  #[serde(rename = "y")]
  pub frequence: u64,
}

fn date_fr<S: Serializer>(d: &NaiveDate, serializer: S) -> Result<S::Ok, S::Error> {
  serializer.serialize_str(d.format("%Y-%m-%d").to_string().as_str())
}
