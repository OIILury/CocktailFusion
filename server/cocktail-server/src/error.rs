use std::{convert::Infallible, env::VarError};

use axum::{
  http::StatusCode,
  response::{Html, IntoResponse},
};
use cocktail_db_twitter::DbTwitterError;
use tantivy::TantivyError;
use thiserror::Error;
use uuid::Uuid;

#[derive(Error, Debug)]
pub enum WebError {
  // #[error("impossible de se connecter à redis.")]
  // RedisConnectionError { source: redis::RedisError },
  // #[error("impossible de créer un pool de connection redis.")]
  // RedisPoolCreationError { source: r2d2::Error },
  #[error("impossible de créer un pool de connection redis.")]
  LaunchServerError { source: hyper::Error },
  #[error("Erreur d'ajout du corpus {corpus_id} au projet {project_id}: {source}")]
  SQLAddCorpusError {
    corpus_id: Uuid,
    project_id: Uuid,
    source: sqlx::Error,
  },
  #[error("Erreur : donnée(s) introuvable(s).")]
  NotFound,
  #[error("WTF: {0}")]
  WTFError(String),
  #[error("Communautés : {0}")]
  Community(String),
  #[error("Export impossible : {0}")]
  ExportImpossible(String),
  #[error("Accès interdit : {0}")]
  Forbidden(String),
  #[error("Requête invalide : {0}")]
  BadRequest(String),
  #[error("Erreur de base de données : {0}")]
  DatabaseError(String),
  #[error("Erreur serveur interne : {0}")]
  InternalServerError(String),
}

impl IntoResponse for WebError {
  fn into_response(self) -> axum::response::Response {
    match self {
      WebError::NotFound => {
        tracing::error!("{self}");
        (StatusCode::NOT_FOUND, Html("<h1>NOT FOUND</h1>")).into_response()
      }
      WebError::Community(e) => {
        tracing::error!("{e}");
        (
          StatusCode::NOT_FOUND,
          Html(format!("<h1>NOT FOUND</h1><p>{e}</p>")),
        )
          .into_response()
      }
      _ => {
        tracing::error!("{}", self);
        (
          StatusCode::INTERNAL_SERVER_ERROR,
          Html("<p>INTERNAL SERVER ERROR</p>"),
        )
          .into_response()
      }
    }
  }
}

impl From<hyper::Error> for WebError {
  fn from(source: hyper::Error) -> Self {
    WebError::LaunchServerError { source }
  }
}

impl From<fts::SearchError> for WebError {
  fn from(e: fts::SearchError) -> Self {
    WebError::WTFError(e.to_string())
  }
}

impl From<sqlx::Error> for WebError {
  fn from(e: sqlx::Error) -> Self {
    match e {
      sqlx::Error::RowNotFound => WebError::NotFound,
      _ => WebError::WTFError(e.to_string()),
    }
  }
}

impl From<DbTwitterError> for WebError {
  fn from(e: DbTwitterError) -> Self {
    match e {
      DbTwitterError::SQL { source: _ } => WebError::NotFound,
    }
  }
}

impl From<cocktail_graph_utils::error::GraphError> for WebError {
  fn from(e: cocktail_graph_utils::error::GraphError) -> Self {
    match e {
      cocktail_graph_utils::error::GraphError::Searcher(e) => WebError::Community(e),
      cocktail_graph_utils::error::GraphError::Database(e) => WebError::WTFError(e.to_string()),
      cocktail_graph_utils::error::GraphError::IO(e) => WebError::WTFError(e.to_string()),
      cocktail_graph_utils::error::GraphError::Script(e) => WebError::WTFError(e),
      cocktail_graph_utils::error::GraphError::Infallible(e) => WebError::WTFError(e.to_string()),
    }
  }
}

impl From<serde_json::Error> for WebError {
  fn from(e: serde_json::Error) -> Self {
    WebError::WTFError(e.to_string())
  }
}

impl From<VarError> for WebError {
  fn from(_source: VarError) -> Self {
    WebError::NotFound
  }
}

impl From<TantivyError> for WebError {
  fn from(_source: TantivyError) -> Self {
    WebError::NotFound
  }
}

impl From<Infallible> for WebError {
  fn from(e: Infallible) -> Self {
    WebError::WTFError(e.to_string())
  }
}
