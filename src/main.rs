mod cli;
mod models;
mod crawler;
mod parser;
mod discovery;

use clap::Parser;
use cli::Cli;
use crawler::Crawler;
use std::str::FromStr;
use url::Url;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    
    let base_url = Url::from_str(&cli.url)?;
    
    let mut crawler = Crawler::new(cli, base_url)?;
    crawler.run().await?;

    Ok(())
}
