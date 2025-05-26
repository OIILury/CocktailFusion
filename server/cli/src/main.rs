use std::{net::SocketAddr, path::PathBuf};

use chrono::NaiveDate;
use clap::{Parser, Subcommand};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Registry};

#[derive(Debug, Parser)]
/// Programme de création de cocktails :
///
/// service, création, consommation
#[clap(name = "cocktail", about)]
struct Cli {
  #[clap(subcommand)]
  commmand: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
  Serve {
    #[clap(long, env)]
    database_path: PathBuf,
    // TODO mouais c'est pas génial
    #[clap(long, env = "PG_DATABASE_URL")]
    pg_database_url: String,
    #[clap(long, env)]
    r_script: PathBuf,
    #[clap(long, env)]
    python_script: PathBuf,
    // jusqu'ici
    #[clap(long, env)]
    topk_database_path: PathBuf,
    #[clap(long, env)]
    kratos_browser_url: String,
    #[clap(long, env = "DIRECTORY_PATH")]
    directory_path: PathBuf,
    #[clap(long, env, default_value = "0.0.0.0:3000")]
    listen_to: SocketAddr,
    #[clap(long, env, default_value = "http://127.0.0.1:4433")]
    kratos_base_path: String,
  },
  #[clap(subcommand)]
  Index(Index),

  /// alimente redis depuis l'entrée standard
  #[clap(subcommand, rename_all = "lower")]
  TopK(TopK),
  #[clap(subcommand)]
  Study(Study),
}

#[derive(Debug, Subcommand)]
enum Index {
  Create {
    #[clap(long, env = "DIRECTORY_PATH")]
    directory_path: PathBuf,
  },
  Ingest {
    #[clap(long, env = "DIRECTORY_PATH")]
    directory_path: PathBuf,
  },
  HashtagSearch {
    #[clap(long, env = "DIRECTORY_PATH")]
    directory_path: PathBuf,
    #[clap(long, env = "QUERY")]
    query: String,
    #[clap(long, env = "LIMIT", default_value_t = 4_294_967_295)]
    limit: usize,
  },
}

#[derive(Debug, Subcommand)]
enum TopK {
  Ingest {
    // #[clap(long, env)]
    // connection_uri: ConnectionInfo,
    #[clap(long, env)]
    start: NaiveDate,
    #[clap(long, env)]
    end: NaiveDate,
  },
  Drop,
}

#[derive(Debug, Subcommand)]
enum Study {
  Clear {
    #[clap(long, env)]
    database_path: PathBuf,
    #[clap(long, env = "PG_DATABASE_URL")]
    pg_database_url: String,
  },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
  Registry::default()
    .with(tracing_subscriber::fmt::layer())
    .with(EnvFilter::from_default_env())
    .init();
  let args = Cli::parse();
  match args.commmand {
    Commands::Serve {
      database_path,
      topk_database_path,
      directory_path,
      listen_to,
      kratos_base_path,
      r_script,
      python_script,
      pg_database_url,
      kratos_browser_url,
    } => {
      tracing::info!("serve");
      let databases = cocktail_server::Databases {
        web_database_path: database_path,
        topk_database_path,
        pg_uri: pg_database_url,
      };
      let kratos = cocktail_server::Kratos {
        kratos_base_path,
        kratos_browser_url,
      };
      let scripts = cocktail_server::Scripts {
        r_script,
        python_script,
      };
      cocktail_server::run(directory_path, listen_to, databases, kratos, scripts).await?
    }
    Commands::Index(command) => match command {
      Index::Create { directory_path } => fts::create_index_config(&directory_path)?,
      Index::Ingest { directory_path } => fts::ingest(directory_path)?,
      Index::HashtagSearch {
        directory_path,
        query,
        limit,
      } => {
        let index = fts::Index::open_in_dir(directory_path)?;
        let results = fts::hashtag_search(&index, query, limit)?;
        dbg!(results);
      }
    },
    Commands::TopK(command) => match command {
      TopK::Ingest { start: _, end: _ } => {
        // cocktail_twitter_data::ingest(connection_uri)?;
        cocktail_twitter_data::ingest_sqlite().await?;
        // let _ = cocktail_twitter_data::create_databases(start, 12).await;
      }
      TopK::Drop => {
        cocktail_twitter_data::drop_databases().await;
      }
    },
    Commands::Study(command) => match command {
      Study::Clear {
        database_path,
        pg_database_url,
      } => {
        let databases = cocktail_server::Databases {
          web_database_path: database_path,
          topk_database_path: PathBuf::new(),
          pg_uri: pg_database_url,
        };

        cocktail_server::clear_anynomous_studies(databases).await?
      }
    },
  };

  Ok(())
}
