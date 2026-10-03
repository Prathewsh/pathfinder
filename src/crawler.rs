use crate::cli::Cli;
use crate::models::{CrawlResult, Task};
use crate::parser::extract_urls;
use anyhow::{ensure, Result};
use futures::{stream::FuturesUnordered, StreamExt};
use reqwest::Client;
use std::collections::{HashSet, VecDeque};
use std::time::Instant;
use url::Url;

pub struct Crawler {
    cli: Cli,
    base_url: Url,
    client: Client,
    visited: HashSet<String>,
}

impl Crawler {
    pub fn new(cli: Cli, mut base_url: Url) -> Result<Self> {
        ensure!(cli.concurrency > 0, "Concurrency must be greater than zero");
        ensure!(
            matches!(base_url.scheme(), "http" | "https") && base_url.host_str().is_some(),
            "Target must be an HTTP or HTTPS URL with a host"
        );
        base_url.set_fragment(None);
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::limited(10))
            .pool_max_idle_per_host(20)
            .build()?;

        Ok(Self {
            cli,
            base_url,
            client,
            visited: HashSet::new(),
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        let mut pending = VecDeque::new();
        self.visited.clear();
        self.visited.insert(self.base_url.to_string());
        pending.push_back(Task {
            url: self.base_url.clone(),
            depth: 0,
        });

        if self.cli.depth > 0 {
            for path in ["/robots.txt", "/sitemap.xml"]
                .into_iter()
                .chain(crate::discovery::common_paths().iter().copied())
            {
                let url = self.base_url.join(path)?;
                if self.visited.insert(url.to_string()) {
                    pending.push_back(Task { url, depth: 1 });
                }
            }
        }

        println!("Pathfinder v0.1\n");
        println!("Target: {}", self.cli.url);
        println!("Depth:  {}", self.cli.depth);
        println!("Concurrency: {}\n", self.cli.concurrency);

        let mut results: Vec<CrawlResult> = Vec::new();

        let mut active = FuturesUnordered::new();
        loop {
            while active.len() < self.cli.concurrency {
                let Some(task) = pending.pop_front() else {
                    break;
                };
                active.push(Self::process_task(
                    task,
                    self.client.clone(),
                    self.cli.clone(),
                    self.base_url.clone(),
                ));
            }

            let Some((result, discovered)) = active.next().await else {
                break;
            };
            for url in discovered {
                if self.visited.insert(url.to_string()) {
                    pending.push_back(Task {
                        url,
                        depth: result.depth + 1,
                    });
                }
            }
            if self.cli.status.is_none() || self.cli.status == Some(result.status) {
                println!("[{}]  {}", result.status, result.url);
            }
            results.push(result);
        }

        // Print summary
        let total = results.len();
        let ok = results.iter().filter(|r| r.status == 200).count();
        let redirects = results
            .iter()
            .filter(|r| (300..400).contains(&r.status))
            .count();
        let errors = results
            .iter()
            .filter(|r| r.status == 0 || r.status >= 400)
            .count();

        println!("\nDiscovered: {}", total);
        println!("200 OK:     {}", ok);
        println!("Redirects:  {}", redirects);
        println!("Errors:     {}", errors);

        // Write output files
        if let Some(ref path) = self.cli.output {
            let lines: Vec<String> = results
                .iter()
                .map(|r| format!("[{}] {}", r.status, r.url))
                .collect();
            std::fs::write(path, lines.join("\n"))?;
            println!("\nResults saved to {}", path);
        }

        if let Some(ref path) = self.cli.json {
            let json = serde_json::to_string_pretty(&results)?;
            std::fs::write(path, json)?;
            println!("JSON results saved to {}", path);
        }

        Ok(())
    }

    async fn process_task(
        task: Task,
        client: Client,
        cli: Cli,
        base_url: Url,
    ) -> (CrawlResult, Vec<Url>) {
        let start = Instant::now();
        let res = client.get(task.url.clone()).send().await;
        let elapsed = start.elapsed().as_millis();

        match res {
            Ok(response) => {
                let status = response.status().as_u16();
                let content_type = response
                    .headers()
                    .get(reqwest::header::CONTENT_TYPE)
                    .and_then(|v| v.to_str().ok())
                    .map(|s| s.to_string());
                let size = response.content_length().unwrap_or(0);

                let response_url = response.url().clone();
                let body = response.text().await.unwrap_or_default();
                let size = if size == 0 { body.len() as u64 } else { size };

                let mut discovered = Vec::new();
                if let Some(ct) = &content_type {
                    if task.depth < cli.depth
                        && (ct.contains("text/html") || (cli.js && ct.contains("javascript")))
                    {
                        discovered =
                            extract_urls(&body, &response_url, &base_url, cli.subdomains, cli.js);
                    }
                }

                (
                    CrawlResult {
                        url: task.url.to_string(),
                        status,
                        content_type,
                        size,
                        depth: task.depth,
                        response_time_ms: elapsed,
                    },
                    discovered,
                )
            }
            Err(_e) => (
                CrawlResult {
                    url: task.url.to_string(),
                    status: 0,
                    content_type: None,
                    size: 0,
                    depth: task.depth,
                    response_time_ms: elapsed,
                },
                Vec::new(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use tokio::time::{timeout, Duration};

    #[test]
    fn rejects_invalid_configuration() {
        let cli = Cli::parse_from(["pathfinder", "-u", "https://example.com", "-c", "0"]);
        assert!(Crawler::new(cli, Url::parse("https://example.com").unwrap()).is_err());
        let cli = Cli::parse_from(["pathfinder", "-u", "file:///tmp/page"]);
        assert!(Crawler::new(cli, Url::parse("file:///tmp/page").unwrap()).is_err());
    }

    async fn crawl_at_depth(depth: u32) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}/", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            loop {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut request = Vec::new();
                let mut buf = [0; 1024];
                while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                    let n = socket.read(&mut buf).await.unwrap();
                    if n == 0 {
                        break;
                    }
                    request.extend_from_slice(&buf[..n]);
                }
                let body = "<a href='/unique-child#one'>child</a><a href='/unique-child#two'>duplicate</a>";
                let response = format!("HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body);
                socket.write_all(response.as_bytes()).await.unwrap();
            }
        });
        let cli = Cli::parse_from([
            "pathfinder",
            "-u",
            &base,
            "-c",
            "1",
            "-d",
            &depth.to_string(),
        ]);
        let mut crawler = Crawler::new(cli, Url::parse(&base).unwrap()).unwrap();
        // Ignore machine-specific HTTP proxy settings in this loopback test.
        crawler.client = Client::builder().no_proxy().build().unwrap();
        let outcome = timeout(Duration::from_secs(15), crawler.run()).await;
        server.abort();
        outcome
            .expect("crawler must finish even when seeds exceed concurrency")
            .unwrap();
        if depth == 0 {
            assert_eq!(crawler.visited.len(), 1);
        } else {
            assert!(crawler.visited.contains(&format!("{base}unique-child")));
            assert!(crawler.visited.contains(&format!("{base}robots.txt")));
            assert!(crawler.visited.len() > 200);
            assert!(crawler.visited.iter().all(|url| !url.contains('#')));
        }
    }

    #[tokio::test]
    async fn completes_large_seed_queue_with_one_worker() {
        crawl_at_depth(1).await;
    }

    #[tokio::test]
    async fn depth_zero_only_requests_target() {
        crawl_at_depth(0).await;
    }
}
