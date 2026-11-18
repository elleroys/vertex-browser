use std::{
    error::Error,
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadResult {
    pub final_url: String,
    pub file_name: String,
    pub bytes_received: u64,
    pub total_bytes: Option<u64>,
}

/// Fetch an HTTP(S) resource into a temporary sibling file, then publish it at the
/// requested destination only after the response has been fully written.
pub fn download_to_path<F>(
    raw_url: &str,
    destination: &Path,
    mut progress: F,
) -> Result<DownloadResult, Box<dyn Error + Send + Sync>>
where
    F: FnMut(u64, Option<u64>) -> bool,
{
    let url = url::Url::parse(raw_url)?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none()
        || !url.username().is_empty() || url.password().is_some()
    {
        return Err("Only credential-free HTTP(S) download URLs are allowed".into());
    }

    let client = reqwest::blocking::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(20))
        .timeout(std::time::Duration::from_secs(60 * 30))
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

    let mut response = client.get(url).send()?.error_for_status()?;
    let final_url = response.url().to_string();
    let total_bytes = response.content_length();
    let file_name = response
        .headers()
        .get(reqwest::header::CONTENT_DISPOSITION)
        .and_then(|value| value.to_str().ok())
        .and_then(filename_from_content_disposition)
        .or_else(|| response.url().path_segments().and_then(|segments| segments.filter(|s| !s.is_empty()).next_back()).map(str::to_owned))
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| "download".to_owned());
    let file_name = sanitize_file_name(&file_name);

    let parent = destination.parent().filter(|path| !path.as_os_str().is_empty()).unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;
    if destination.exists() {
        return Err(format!("Destination already exists: {}", destination.display()).into());
    }

    let mut temporary_name = destination.file_name().unwrap_or_default().to_os_string();
    temporary_name.push(format!(".{}.vertex-part", uuid::Uuid::new_v4()));
    let temporary_path: PathBuf = parent.join(temporary_name);

    let result = (|| -> Result<u64, Box<dyn Error + Send + Sync>> {
        let mut output = OpenOptions::new().write(true).create_new(true).open(&temporary_path)?;
        let mut received = 0u64;
        let mut buffer = vec![0u8; 64 * 1024];
        loop {
            let count = response.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            output.write_all(&buffer[..count])?;
            received = received.saturating_add(count as u64);
            if !progress(received, total_bytes) {
                return Err(std::io::Error::new(std::io::ErrorKind::Interrupted, "Download cancelled").into());
            }
        }
        output.flush()?;
        output.sync_all()?;
        Ok(received)
    })();

    match result {
        Ok(bytes_received) => {
            if destination.exists() {
                let _ = fs::remove_file(&temporary_path);
                return Err(format!("Destination already exists: {}", destination.display()).into());
            }
            fs::rename(&temporary_path, destination)?;
            Ok(DownloadResult { final_url, file_name, bytes_received, total_bytes })
        }
        Err(error) => {
            let _ = fs::remove_file(&temporary_path);
            Err(error)
        }
    }
}

pub fn suggested_file_name(raw_url: &str) -> String {
    let candidate = url::Url::parse(raw_url).ok()
        .and_then(|url| url.path_segments().and_then(|mut segments| segments.filter(|part| !part.is_empty()).next_back().map(str::to_owned)))
        .unwrap_or_else(|| "download".to_owned());
    sanitize_file_name(&candidate)
}

pub fn sanitize_file_name(input: &str) -> String {
    let leaf = input.rsplit(['/', '\\\\']).next().unwrap_or(input);
    let cleaned: String = leaf
        .chars()
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
    fn sanitizes_windows_paths_and_control_characters() {
        assert_eq!(sanitize_file_name(r"..\\folder\\report?.pdf"), "report.pdf");
        assert_eq!(sanitize_file_name(""), "download");
    }

    #[test]
    fn rejects_non_http_and_embedded_credentials() {
        assert!(download_to_path("file:///C:/secret", Path::new("unused"), |_, _| true).is_err());
        assert!(download_to_path("https://user:pass@example.com/file", Path::new("unused"), |_, _| true).is_err());
    }
}
