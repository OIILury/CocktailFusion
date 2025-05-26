use std::path::PathBuf;

use clap::Parser;

#[derive(Debug, Parser)]
struct Args {
  #[clap(long)]
  directory_path: PathBuf,
}
fn main() -> anyhow::Result<()> {
  let args = Args::parse();

  let index = fts::retrieve_index(args.directory_path).map_err(|_e| anyhow::anyhow!("oups"))?;
  let count = fts::doc_count(
    &index,
    None, // Some("text:vegan AND published_time_ms:[1590969600000 TO 1593561600000]".to_string()),
  )
  .map_err(|_e| anyhow::anyhow!("oups"))?;
  println!("doc count: {count}");
  Ok(())
}
