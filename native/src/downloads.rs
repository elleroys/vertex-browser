use std::{
    error::Error,
    fs::{self, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
    time::Duration,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumeInfo {
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadResult {
    pub final_url: String,
    pub file_name: String,
    pub bytes_received: u64,
    pub total_bytes: Option<u64>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}

/// Download to a stable partial file. Resume is attempted only when a validator
/// (ETag or Last-Modified) was persisted with the partial file.
pub fn download_to_path<F>(
    raw_url: &str,
    destination: &Path,
    partial_path: &Path,
    resume: ResumeInfo,
    mut progress: F,
) -> Result<DownloadResult, Box<dyn Error + Send + Sync>>
where
    F: FnMut(u64, Option<u64>, &ResumeInfo) -> bool,
{
    let url = url::Url::parse(raw_url)?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none()
        || !url.username().is_empty() || url.password().is_some()
    {
        return Err("Only credential-free HTTP(S) download URLs are allowed".into());
    }

    let client = reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(20))
        .timeout(Duration::from_secs(60 * 30))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= 10 {
                return attempt.error("too many redirects");
            }
            if !matches!(attempt.url().scheme(), "http" | "https") {
                return attempt.error("redirect to a non-HTTP(S) scheme blocked");
            }
            if attempt.previous().iter().any(|previous| previous.scheme() == "https")
                && attempt.url().scheme() == "http"
            {
                return attempt.error("HTTPS to HTTP downgrade blocked");
            }
            attempt.follow()
        }))
        .build()?;

    let parent = destination.parent().filter(|path| !path.as_os_str().is_empty()).unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;
    if destination.exists() {
        return Err(format!("Destination already exists: {}", destination.display()).into());
    }
    if let Some(partial_parent) = partial_path.parent().filter(|path| !path.as_os_str().is_empty()) {
        fs::create_dir_all(partial_parent)?;
    }

    let mut existing = fs::metadata(partial_path).map(|m| m.len()).unwrap_or(0);
    let validator = resume.etag.as_deref().or(resume.last_modified.as_deref());
    if existing > 0 && validator.is_none() {
        fs::remove_file(partial_path)?;
        existing = 0;
    }

    let mut request = client.get(url.clone());
    if existing > 0 {
        request = request.header(reqwest::header::RANGE, format!("bytes={existing}-"));
        if let Some(value) = validator {
            request = request.header(reqwest::header::IF_RANGE, value);
        }
    }
    let mut response = request.send()?;

    // A stale partial may refer to a representation that no longer exists.
    // Start over once on 416; never append bytes from a different representation.
    if response.status() == reqwest::StatusCode::RANGE_NOT_SATISFIABLE && existing > 0 {
        fs::remove_file(partial_path)?;
        existing = 0;
        response = client.get(url).send()?;
    }
    let status = response.status();
    if !status.is_success() {
        return Err(format!("Download server returned HTTP {status}").into());
    }
    let append = existing > 0 && status == reqwest::StatusCode::PARTIAL_CONTENT;
    if append {
        let range = response.headers().get(reqwest::header::CONTENT_RANGE)
            .and_then(|v| v.to_str().ok())
            .ok_or("Server returned 206 without Content-Range")?;
        if content_range_start(range) != Some(existing) {
            return Err("Server returned an unexpected Content-Range; partial file preserved".into());
        }
    } else if status == reqwest::StatusCode::PARTIAL_CONTENT {
        return Err("Server returned partial content without a valid resume request".into());
    }

    let response_etag = response.headers().get(reqwest::header::ETAG)
        .and_then(|v| v.to_str().ok()).map(str::to_owned);
    let response_modified = response.headers().get(reqwest::header::LAST_MODIFIED)
        .and_then(|v| v.to_str().ok()).map(str::to_owned);
    if append {
        if let (Some(old), Some(new)) = (resume.etag.as_deref(), response_etag.as_deref()) {
            if old != new { return Err("ETag changed during resume; partial file preserved".into()); }
        }
        if let (Some(old), Some(new)) = (resume.last_modified.as_deref(), response_modified.as_deref()) {
            if old != new { return Err("Last-Modified changed during resume; partial file preserved".into()); }
        }
    }
    let response_info = ResumeInfo {
        etag: response_etag.or(resume.etag),
        last_modified: response_modified.or(resume.last_modified),
    };
    let remaining = response.content_length();
    let total_bytes = if append { remaining.map(|n| n.saturating_add(existing)) } else { remaining };

    let file_name = response.headers()
        .get(reqwest::header::CONTENT_DISPOSITION)
        .and_then(|value| value.to_str().ok())
        .and_then(filename_from_content_disposition)
        .or_else(|| response.url().path_segments().and_then(|segments| segments.filter(|s| !s.is_empty()).next_back()).map(str::to_owned))
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| "download".to_owned());
    let file_name = sanitize_file_name(&file_name);

    let mut output = OpenOptions::new().create(true).write(true).open(partial_path)?;
    if append {
        output.seek(SeekFrom::End(0))?;
    } else {
        output.set_len(0)?;
    }
    let mut received = if append { existing } else { 0 };
    if !progress(received, total_bytes, &response_info) {
        return Err(std::io::Error::new(std::io::ErrorKind::Interrupted, "Download cancelled").into());
    }
    let mut buffer = vec![0u8; 64 * 1024];
    let mut response = response;
    loop {
        let count = response.read(&mut buffer)?;
        if count == 0 { break; }
        output.write_all(&buffer[..count])?;
        received = received.saturating_add(count as u64);
        if !progress(received, total_bytes, &response_info) {
            output.flush()?;
            output.sync_all()?;
            return Err(std::io::Error::new(std::io::ErrorKind::Interrupted, "Download cancelled").into());
        }
    }
    output.flush()?;
    output.sync_all()?;
    if !progress(received, total_bytes, &response_info) {
        return Err(std::io::Error::new(std::io::ErrorKind::Interrupted, "Download cancelled").into());
    }
    if destination.exists() {
        return Err(format!("Destination already exists: {}", destination.display()).into());
    }
    fs::rename(partial_path, destination)?;
    Ok(DownloadResult {
        final_url: response.url().to_string(),
        file_name,
        bytes_received: received,
        total_bytes,
        etag: response_info.etag,
        last_modified: response_info.last_modified,
    })
}

fn content_range_start(value: &str) -> Option<u64> {
    let range = value.strip_prefix("bytes ")?;
    let (span, _) = range.split_once('/')?;
    let (start, _) = span.split_once('-')?;
    start.parse().ok()
}

pub fn suggested_file_name(raw_url: &str) -> String {
    let candidate = url::Url::parse(raw_url).ok()
        .and_then(|url| url.path_segments().and_then(|mut segments| segments.filter(|part| !part.is_empty()).next_back().map(str::to_owned)))
        .unwrap_or_else(|| "download".to_owned());
    sanitize_file_name(&candidate)
}

pub fn sanitize_file_name(input: &str) -> String {
    let leaf = input.rsplit(['/', '\\\\']).next().unwrap_or(input);
    let cleaned: String = leaf.chars()
        .filter(|character| !character.is_control() && !matches!(character, '<' | '>' | ':' | '"' | '/' | '\\\\' | '|' | '?' | '*'))
        .collect();
    let cleaned = cleaned.trim().trim_matches('.');
    if cleaned.is_empty() { "download".to_owned() } else { cleaned.chars().take(180).collect() }
}

fn filename_from_content_disposition(header: &str) -> Option<String> {
    for part in header.split(';').skip(1) {
        let Some((key, value)) = part.trim().split_once('=') else { continue; };
        if key.eq_ignore_ascii_case("filename") || key.eq_ignore_ascii_case("filename*") {
            let value = value.trim().trim_matches('"');
            let value = value.strip_prefix("UTF-8''").unwrap_or(value);
            let decoded = url::form_urlencoded::parse(format!("name={value}").as_bytes())
                .next().map(|(_, value)| value.into_owned()).unwrap_or_default();
            if !decoded.trim().is_empty() { return Some(decoded); }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_content_range_start() {
        assert_eq!(content_range_start("bytes 123-999/1000"), Some(123));
        assert_eq!(content_range_start("bytes */1000"), None);
        assert_eq!(content_range_start("items 1-2/3"), None);
    }

    #[test]
    fn sanitizes_windows_paths_and_control_characters() {
        assert_eq!(sanitize_file_name(r"..\\folder\\report?.pdf"), "report.pdf");
        assert_eq!(sanitize_file_name(""), "download");
    }

    #[test]
    fn reads_content_disposition_filenames() {
        assert_eq!(filename_from_content_disposition("attachment; filename=\"archive.zip\""), Some("archive.zip".to_owned()));
        assert_eq!(filename_from_content_disposition("attachment; filename*=UTF-8''hello%20world.zip"), Some("hello world.zip".to_owned()));
    }
}
