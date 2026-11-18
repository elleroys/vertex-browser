use std::{collections::HashSet, io::Read, time::Duration};

use reqwest::blocking::Client;
use url::Url;

const OPENPHISH_FEED_URL: &str = "https://openphish.com/feed.txt";
const MAX_FEED_BYTES: u64 = 20 * 1024 * 1024;
const MAX_FEED_ENTRIES: usize = 500_000;
const MAX_URL_LENGTH: usize = 8_192;

#[derive(Debug, Clone, Default)]
pub struct ThreatFeed {
    pub urls: HashSet<String>,
    pub ignored: usize,
}

pub fn canonical_url(url: &Url) -> String {
    let mut normalized = url.clone();
    normalized.set_fragment(None);
    normalized.to_string()
}

pub fn parse_stored_feed(contents: &str) -> Result<HashSet<String>, String> {
    if contents.trim().is_empty() { return Ok(HashSet::new()); }
    Ok(parse_feed(contents)?.urls)
}

pub fn parse_feed(contents: &str) -> Result<ThreatFeed, String> {
    if contents.len() > MAX_FEED_BYTES as usize {
        return Err("Файл бази перевищує ліміт 20 MiB".to_owned());
    }
    let trimmed = contents.trim_start().to_ascii_lowercase();
    if trimmed.starts_with("<!doctype html") || trimmed.starts_with("<html") || trimmed.starts_with("<head") {
        return Err("Замість текстової бази сервер повернув HTML-сторінку".to_owned());
    }
    let mut urls = HashSet::new();
    let mut ignored = 0usize;
    let mut candidates = 0usize;
    for raw_line in contents.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('!') { continue; }
        candidates += 1;
        if candidates > MAX_FEED_ENTRIES { return Err("База містить понад 500 000 рядків".to_owned()); }
        if line.len() > MAX_URL_LENGTH { ignored += 1; continue; }
        let Ok(url) = Url::parse(line) else { ignored += 1; continue; };
        if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none()
            || !url.username().is_empty() || url.password().is_some()
        { ignored += 1; continue; }
        urls.insert(canonical_url(&url));
    }
    if urls.is_empty() { return Err("У відповіді немає жодної коректної HTTP(S) URL-адреси".to_owned()); }
    if candidates > 0 && ignored * 5 > candidates {
        return Err(format!("Відхилено підозрілу базу: некоректних рядків {ignored} із {candidates}"));
    }
    Ok(ThreatFeed { urls, ignored })
}

pub fn fetch_openphish_feed() -> Result<ThreatFeed, String> {
    let client = Client::builder()
        .timeout(Duration::from_secs(35))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= 5 || attempt.url().scheme() != "https"
                || attempt.url().host_str() != Some("openphish.com")
            { attempt.stop() } else { attempt.follow() }
        }))
        .build()
        .map_err(|error| format!("Не вдалося створити HTTPS-клієнт: {error}"))?;
    let response = client.get(OPENPHISH_FEED_URL)
        .header(reqwest::header::ACCEPT, "text/plain")
        .header(reqwest::header::USER_AGENT, "VertexBrowser-Security/0.1")
        .send()
        .map_err(|error| format!("Помилка завантаження OpenPhish: {error}"))?
        .error_for_status()
        .map_err(|error| format!("OpenPhish повернув помилку HTTP: {error}"))?;
    if response.url().scheme() != "https" || response.url().host_str() != Some("openphish.com") {
        return Err("Відхилено неочікувану адресу джерела після перенаправлення".to_owned());
    }
    if response.content_length().is_some_and(|length| length > MAX_FEED_BYTES) {
        return Err("База OpenPhish перевищує ліміт 20 MiB".to_owned());
    }
    let mut limited = response.take(MAX_FEED_BYTES + 1);
    let mut bytes = Vec::new();
    limited.read_to_end(&mut bytes).map_err(|error| format!("Помилка читання бази OpenPhish: {error}"))?;
    if bytes.len() as u64 > MAX_FEED_BYTES { return Err("База OpenPhish перевищує ліміт 20 MiB".to_owned()); }
    let body = String::from_utf8(bytes).map_err(|_| "База OpenPhish має некоректне UTF-8 кодування".to_owned())?;
    parse_feed(&body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_only_http_urls_and_deduplicates_canonical_entries() {
        let feed = parse_feed("# OpenPhish snapshot\nhttps://EXAMPLE.com/login#fragment\nhttps://example.com/login\nfile:///etc/passwd\njavascript:alert(1)\nnot a url\n").unwrap();
        assert_eq!(feed.urls.len(), 1);
        assert!(feed.urls.contains("https://example.com/login"));
        assert_eq!(feed.ignored, 3);
    }

    #[test]
    fn rejects_empty_html_and_invalid_feeds() {
        assert!(parse_feed("").is_err());
        assert!(parse_feed("<!doctype html><html></html>").is_err());
        assert!(parse_feed("not a url\njavascript:alert(1)").is_err());
    }

    #[test]
    fn rejects_feeds_with_too_many_invalid_lines() {
        let feed = "https://valid.example/a\ninvalid\ninvalid\ninvalid\ninvalid\ninvalid\n";
        assert!(parse_feed(feed).unwrap_err().contains("некоректних рядків"));
    }

    #[test]
    fn exact_url_matching_does_not_block_entire_domains() {
        let feed = parse_feed("https://example.com/phishing/login\n").unwrap();
        let listed = Url::parse("https://example.com/phishing/login#form").unwrap();
        let other_path = Url::parse("https://example.com/").unwrap();
        let subdomain = Url::parse("https://safe.example.com/phishing/login").unwrap();
        assert!(feed.urls.contains(&canonical_url(&listed)));
        assert!(!feed.urls.contains(&canonical_url(&other_path)));
        assert!(!feed.urls.contains(&canonical_url(&subdomain)));
    }

    #[test]
    fn rejects_urls_with_embedded_credentials() {
        assert!(parse_feed("https://user:pass@example.com/login\n").is_err());
    }
}
