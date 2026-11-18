//! Lightweight host-based request blocking for the native Servo embedder.
//!
//! This is an intentionally small built-in denylist, not a replacement for
//! EasyList/uBlock filter syntax. Matching is performed on host boundaries so
//! that e.g. `notdoubleclick.net` is not accidentally blocked.

const BLOCKED_HOSTS: &[&str] = &[
    "doubleclick.net",
    "googlesyndication.com",
    "googleadservices.com",
    "adservice.google.com",
    "adnxs.com",
    "scorecardresearch.com",
    "taboola.com",
    "outbrain.com",
    "pubmatic.com",
    "openx.net",
    "adsrvr.org",
    "criteo.com",
    "casalemedia.com",
    "rubiconproject.com",
    "serving-sys.com",
];

/// Returns true when a URL host matches a built-in advertising or tracking host.
pub fn is_blocked_url(url: &url::Url) -> bool {
    let Some(host) = url.host_str() else {
        return false;
    };
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    BLOCKED_HOSTS.iter().any(|blocked| {
        host == *blocked || host.strip_suffix(blocked).is_some_and(|prefix| prefix.ends_with('.'))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_exact_hosts_and_subdomains() {
        assert!(is_blocked_url(&url::Url::parse("https://doubleclick.net/ad.js").unwrap()));
        assert!(is_blocked_url(&url::Url::parse("https://pagead.googlesyndication.com/x").unwrap()));
    }

    #[test]
    fn does_not_block_lookalike_hosts_or_first_party_content() {
        assert!(!is_blocked_url(&url::Url::parse("https://notdoubleclick.net/").unwrap()));
        assert!(!is_blocked_url(&url::Url::parse("https://example.com/").unwrap()));
        assert!(!is_blocked_url(&url::Url::parse("data:text/plain,hello").unwrap()));
    }
}
