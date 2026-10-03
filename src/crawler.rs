use crate::cli::Cli;
use crate::models::{CrawlResult, Task};
use crate::parser::extract_urls;
use anyhow::Result;
use dashmap::DashSet;
use reqwest::Client;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{mpsc, Semaphore};
use url::Url;

pub struct Crawler {
    cli: Cli,
    base_url: Url,
    client: Client,
    visited: Arc<DashSet<String>>,
}

impl Crawler {
    pub fn new(cli: Cli, base_url: Url) -> Result<Self> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::limited(10))
            .pool_max_idle_per_host(20)
            .build()?;

        Ok(Self {
            cli,
            base_url,
            client,
            visited: Arc::new(DashSet::new()),
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        let (task_tx, mut task_rx) = mpsc::channel::<Task>(self.cli.concurrency * 2);
        let (result_tx, mut result_rx) = mpsc::unbounded_channel::<CrawlResult>();
        let semaphore = Arc::new(Semaphore::new(self.cli.concurrency));
        let in_flight = Arc::new(AtomicUsize::new(0));

        // Seed initial URLs
        let initial_url = self.base_url.clone();
        self.visited.insert(initial_url.to_string());
        task_tx.send(Task { url: initial_url, depth: 0 }).await?;

        let robots_url = self.base_url.join("/robots.txt")?;
        if self.visited.insert(robots_url.to_string()) {
            task_tx.send(Task { url: robots_url, depth: 1 }).await?;
        }
        let sitemap_url = self.base_url.join("/sitemap.xml")?;
        if self.visited.insert(sitemap_url.to_string()) {
            task_tx.send(Task { url: sitemap_url, depth: 1 }).await?;
        }

        // Seed built-in common paths for automatic discovery
        for path in crate::discovery::common_paths() {
            if let Ok(url) = self.base_url.join(path) {
                if self.visited.insert(url.to_string()) {
                    task_tx.send(Task { url, depth: 1 }).await?;
                }
            }
        }

        println!("Pathfinder v0.1\n");
        println!("Target: {}", self.cli.url);
        println!("Depth:  {}", self.cli.depth);
        println!("Concurrency: {}\n", self.cli.concurrency);

        let mut results: Vec<CrawlResult> = Vec::new();

        loop {
            tokio::select! {
                biased;

                // Collect completed results first
                Some(result) = result_rx.recv() => {
                    in_flight.fetch_sub(1, Ordering::SeqCst);

                    if self.cli.status.is_none() || self.cli.status == Some(result.status) {
                        println!("[{}]  {}", result.status, result.url);
                    }
                    results.push(result);
                }

                // Dispatch new tasks
                Some(task) = task_rx.recv() => {
                    in_flight.fetch_add(1, Ordering::SeqCst);

                    let client = self.client.clone();
                    let task_tx = task_tx.clone();
                    let result_tx = result_tx.clone();
                    let visited = self.visited.clone();
                    let sem = semaphore.clone();
                    let cli = self.cli.clone();
                    let base_url = self.base_url.clone();

                    tokio::spawn(async move {
                        let _permit = sem.acquire().await.unwrap();
                        Self::process_task(task, client, task_tx, result_tx, visited, cli, base_url).await;
                    });
                }

                else => break,
            }

            // If nothing is in-flight and the task channel is empty, we're done
            if in_flight.load(Ordering::SeqCst) == 0 && task_rx.is_empty() {
                // Drain any remaining results
                while let Ok(result) = result_rx.try_recv() {
                    if self.cli.status.is_none() || self.cli.status == Some(result.status) {
                        println!("[{}]  {}", result.status, result.url);
                    }
                    results.push(result);
                }
                break;
            }
        }

        // Print summary
        let total = results.len();
        let ok = results.iter().filter(|r| r.status == 200).count();
        let redirects = results.iter().filter(|r| (300..400).contains(&r.status)).count();
        let errors = results.iter().filter(|r| r.status >= 400).count();

        println!("\nDiscovered: {}", total);
        println!("200 OK:     {}", ok);
        println!("Redirects:  {}", redirects);
        println!("Errors:     {}", errors);

        // Write output files
        if let Some(ref path) = self.cli.output {
            let lines: Vec<String> = results.iter().map(|r| format!("[{}] {}", r.status, r.url)).collect();
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
        task_tx: mpsc::Sender<Task>,
        result_tx: mpsc::UnboundedSender<CrawlResult>,
        visited: Arc<DashSet<String>>,
        cli: Cli,
        base_url: Url,
    ) {
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

                let body = response.text().await.unwrap_or_default();
                let size = if size == 0 { body.len() as u64 } else { size };

                // Parse and discover new URLs
                if let Some(ct) = &content_type {
                    if task.depth < cli.depth
                        && (ct.contains("text/html")
                            || (cli.js && ct.contains("javascript")))
                    {
                        let new_urls =
                            extract_urls(&body, &task.url, &base_url, cli.subdomains, cli.js);
                        for u in new_urls {
                            if visited.insert(u.to_string()) {
                                let _ = task_tx.send(Task { url: u, depth: task.depth + 1 }).await;
                            }
                        }
                    }
                }

                let _ = result_tx.send(CrawlResult {
                    url: task.url.to_string(),
                    status,
                    content_type,
                    size,
                    depth: task.depth,
                    response_time_ms: elapsed,
                });
            }
            Err(_e) => {
                let _ = result_tx.send(CrawlResult {
                    url: task.url.to_string(),
                    status: 0,
                    content_type: None,
                    size: 0,
                    depth: task.depth,
                    response_time_ms: elapsed,
                });
            }
        }
    }
}
