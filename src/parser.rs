use regex::Regex;
use scraper::{Html, Selector};
use url::Url;

pub fn extract_urls(html: &str, base: &Url, domain: &Url, subdomains: bool, parse_js: bool) -> Vec<Url> {
    let mut urls = Vec::new();
    
    // Parse HTML
    let document = Html::parse_document(html);
    let selectors = vec![
        ("a", "href"),
        ("form", "action"),
        ("script", "src"),
        ("link", "href"),
        ("img", "src"),
        ("iframe", "src"),
        ("video", "src"),
        ("source", "src"),
    ];
    
    for (tag, attr) in selectors {
        if let Ok(selector) = Selector::parse(tag) {
            for element in document.select(&selector) {
                if let Some(val) = element.value().attr(attr) {
                    if let Ok(mut parsed) = base.join(val) {
                        parsed.set_fragment(None);
                        if is_allowed(&parsed, domain, subdomains) {
                            urls.push(parsed);
                        }
                    }
                }
            }
        }
    }
    
    // Quick regex for JS if requested
    if parse_js {
        let re = Regex::new(r#"["'](/[a-zA-Z0-9_\-\./]+)["']"#).unwrap();
        for cap in re.captures_iter(html) {
            if let Some(mat) = cap.get(1) {
                if let Ok(mut parsed) = base.join(mat.as_str()) {
                    parsed.set_fragment(None);
                    if is_allowed(&parsed, domain, subdomains) {
                        urls.push(parsed);
                    }
                }
            }
        }
    }
    
    urls
}

fn is_allowed(url: &Url, domain: &Url, subdomains: bool) -> bool {
    if url.scheme() != "http" && url.scheme() != "https" {
        return false;
    }
    if let (Some(u_host), Some(d_host)) = (url.host_str(), domain.host_str()) {
        if u_host == d_host {
            return true;
        }
        if subdomains && u_host.ends_with(d_host) {
            return true;
        }
    }
    false
}
