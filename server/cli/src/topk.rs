use std::path::PathBuf;

use clap::Parser;
use fts::MAX;

#[derive(Debug, Parser)]
struct Args {
  #[clap(long, short)]
  directory_path: PathBuf,
  #[clap(long, short)]
  query: String,
}

fn main() -> anyhow::Result<()> {
  let args = Args::parse();

  let index = fts::retrieve_index(args.directory_path.clone()).map_err(|e| {
    anyhow::anyhow!("Erreur lors de la récupération de l'index: {:?}", e)
  })?;

  let topk = fts::topk(&index, &args.query, MAX).map_err(|e| {
    anyhow::anyhow!("Erreur lors de la génération des top hashtags: {:?}", e)
  })?;
  
  println!("{}", topk);

  Ok(())
}
