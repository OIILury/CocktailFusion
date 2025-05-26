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

  let index = fts::retrieve_index(args.directory_path).map_err(|_e| anyhow::anyhow!("oups"))?;
  let topk = fts::topk(&index, &args.query, MAX).map_err(|e| {
    dbg!(e);
    anyhow::anyhow!("topk oups")
  })?;
  println!("{topk}");

  Ok(())
}
