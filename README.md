# Pathfinder

A fast web path discovery and crawling CLI tool written in Rust. Unlike traditional wordlist-based tools like Gobuster, Pathfinder combines **automatic crawling**, **built-in path brute-forcing (~600 common paths)**, and **link extraction** to discover as many valid URLs as possible on a target website.

## Features

- **Automatic path discovery** — built-in wordlist of ~600 common paths (admin panels, APIs, config files, backups, debug endpoints, CMS paths, etc.)
- **Recursive crawling** — follows links found in HTML to discover more pages
- **HTML parsing** via `scraper` — extracts URLs from `<a>`, `<form>`, `<script>`, `<link>`, `<img>`, `<iframe>`, `<video>`, `<source>` tags
- **JavaScript URL extraction** (optional) — regex-based path detection in JS files
- **robots.txt & sitemap.xml** — automatically parsed and queued
- **High performance** — async Tokio runtime, connection pooling, configurable concurrency
- **Domain-scoped** — stays on target domain by default, optional subdomain support
- **Output options** — text file and JSON export

## Installation

### From source

```bash
git clone https://github.com/Prathewsh/pathfinder.git
cd pathfinder
cargo install --path .
```

### Build only

```bash
cargo build --release
# Binary at: target/release/pathfinder
```

## Usage

### Basic scan

```bash
pathfinder -u https://example.com
```

### Custom depth and concurrency

```bash
pathfinder -u https://example.com -d 3 -c 50
```

### Show only 200 responses

```bash
pathfinder -u https://example.com --status 200
```

### Enable JavaScript URL extraction

```bash
pathfinder -u https://example.com --js
```

### Allow subdomain crawling

```bash
pathfinder -u https://example.com --subdomains
```

### Save results

```bash
pathfinder -u https://example.com -o results.txt
pathfinder -u https://example.com --json results.json
```

### Full example

```bash
pathfinder -u https://example.com -d 3 -c 100 --js --status 200 --json output.json
```

## Options

| Flag                | Description                               | Default   |
| ------------------- | ----------------------------------------- | --------- |
| `-u, --url`         | Target URL (required)                     | —         |
| `-d, --depth`       | Maximum crawl depth                       | `3`       |
| `-c, --concurrency` | Maximum concurrent requests               | `100`     |
| `--status`          | Show only responses with this status code | all       |
| `--js`              | Extract URLs from JavaScript files        | off       |
| `--subdomains`      | Allow crawling subdomains                 | off       |
| `--rate`            | Rate limit (requests/second)              | unlimited |
| `-o, --output`      | Save results to text file                 | —         |
| `--json`            | Save results to JSON file                 | —         |
| `-w, --wordlist`    | Supplemental wordlist file                | —         |

## How It Works

```
Target URL
    ↓
HTTP request + built-in path brute-force (~600 common paths)
    ↓
Parse HTML (scraper) / JS (regex) / robots.txt / sitemap.xml
    ↓
Discover new URLs
    ↓
Queue unseen URLs → concurrent workers
    ↓
Repeat until depth limit reached
```

**Key difference from Gobuster:** Pathfinder doesn't just check a wordlist. It crawls, parses responses, and discovers URLs dynamically — plus it has a built-in wordlist so no external files are needed.

## Example Output

```
Pathfinder v0.1

Target: https://example.com
Depth:  2
Concurrency: 20

[200]  https://example.com/
[200]  https://example.com/login
[200]  https://example.com/about
[200]  https://example.com/blog
[200]  https://example.com/catalog
[200]  https://example.com/my-account
[405]  https://example.com/catalog/product/stock
[404]  https://example.com/robots.txt
[404]  https://example.com/sitemap.xml

Discovered: 80
200 OK:     77
Redirects:  0
Errors:     3
```

## JSON Output Format

```json
{
  "url": "https://example.com/login",
  "status": 200,
  "content_type": "text/html",
  "size": 12345,
  "depth": 1,
  "response_time_ms": 142
}
```

## Architecture

```
src/
├── main.rs          # Entry point
├── cli.rs           # CLI argument parsing (clap)
├── crawler.rs       # Async crawl engine (tokio, semaphore, channels)
├── parser.rs        # HTML link extraction (scraper) + JS regex
├── discovery.rs     # Built-in common path wordlist (~600 paths)
└── models.rs        # Data structures (Task, CrawlResult)
```

## Built-in Discovery Paths

The embedded wordlist covers:

- Admin panels (`/admin`, `/dashboard`, `/cpanel`, `/panel`)
- API endpoints (`/api/v1`, `/api/users`, `/graphql`, `/swagger`)
- Config files (`/.env`, `/config.json`, `/web.config`)
- Git/VCS (`/.git/HEAD`, `/.gitignore`, `/.svn`)
- Backups (`/backup.sql`, `/backup.zip`, `/db.sql`)
- CMS paths (WordPress, Drupal, Joomla)
- Health/monitoring (`/health`, `/metrics`, `/actuator`)
- Logs (`/error.log`, `/access.log`, `/debug.log`)
- Cloud configs (`/.aws/credentials`, `/.docker/config.json`)
- And many more...

## License

MIT
