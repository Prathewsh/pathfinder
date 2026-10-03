use serde::Serialize;
use url::Url;

#[derive(Debug, Clone, Serialize)]
pub struct CrawlResult {
    pub url: String,
    pub status: u16,
    pub content_type: Option<String>,
    pub size: u64,
    pub depth: u32,
    pub response_time_ms: u128,
}

#[derive(Debug, Clone)]
pub struct Task {
    pub url: Url,
    pub depth: u32,
}
