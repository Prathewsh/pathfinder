use clap::Parser;

#[derive(Parser, Debug, Clone)]
#[command(name = "Pathfinder")]
#[command(version = "0.1")]
#[command(about = "Fast web path discovery and crawling CLI tool")]
pub struct Cli {
    /// Target URL
    #[arg(short, long)]
    pub url: String,

    /// Maximum crawl depth
    #[arg(short, long, default_value_t = 3)]
    pub depth: u32,

    /// Maximum concurrent requests
    #[arg(short, long, default_value_t = 100)]
    pub concurrency: usize,

    /// Show only responses with this HTTP status code
    #[arg(long)]
    pub status: Option<u16>,

    /// Optionally inspect JavaScript files for URLs
    #[arg(long, default_value_t = false)]
    pub js: bool,

    /// Allow subdomains when crawling
    #[arg(long, default_value_t = false)]
    pub subdomains: bool,

    /// Optional request rate limit (requests per second)
    #[arg(long)]
    pub rate: Option<u32>,

    /// Output results to a text file
    #[arg(short, long)]
    pub output: Option<String>,

    /// Output results to a JSON file
    #[arg(long)]
    pub json: Option<String>,

    /// Wordlist to supplement crawling
    #[arg(short, long)]
    pub wordlist: Option<String>,
}
