use std::{convert::Infallible, io};

use fts::SearchError;

#[derive(Debug, thiserror::Error)]
pub enum GraphError {
  #[error("Searcher: {0}")]
  Searcher(String),
  #[error("Database: {0}")]
  Database(#[source] sqlx::Error),
  #[error("IO error: {0}")]
  IO(#[source] io::Error),
  #[error("Script: {0}")]
  Script(String),
  #[error("Infallible: {0}")]
  Infallible(#[source] Infallible),
}

impl std::convert::From<sqlx::Error> for GraphError {
  fn from(e: sqlx::Error) -> Self {
    GraphError::Database(e)
  }
}

impl std::convert::From<io::Error> for GraphError {
  fn from(e: io::Error) -> Self {
    GraphError::IO(e)
  }
}

impl std::convert::From<SearchError> for GraphError {
  fn from(_: SearchError) -> Self {
    GraphError::Searcher("Search error".to_string())
  }
}

impl std::convert::From<Infallible> for GraphError {
  fn from(e: Infallible) -> Self {
    GraphError::Infallible(e)
  }
}
