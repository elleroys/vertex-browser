//! Small, dependency-free subset of EasyList network filter matching.
//!
//! Supported rules:
//! - `||example.com^` — a domain and its subdomains.
//! - `||example.com/path^` — a domain/path prefix.
//! - `@@||example.com^` — an exception rule.
//! - Plain host names such as `ads.example.com`.
//!
//! Cosmetic rules, regular expressions, resource-type modifiers, scriptlets and
//! the full uBlock Origin syntax are deliberately not claimed or implemented.

const BUILTIN_RULES: &[&str] = &[
    "||doubleclick.net^",
    "||googlesyndication.com^",
    "||googleadservices.com^",
    "||adservice.google.com^",
    "||adnxs.com^",
    "||scorecardresearch.com^",
    "||taboola.com^",
    "||outbrain.com^",
    "||pubmatic.com^",
    "||openx.net^",
    "||adsrvr.org^",
    "||criteo.com^",
    "||casalemedia.com^",
    "||rubiconproject.com^",
    "||serving-sys.com^",
];

#[derive(Debug, PartialEq, Eq)]
enum Rule<'a> {
    Domain { host: &'a str, path: Option<&'a str> },
    Invalid,
}

fn parse_rule(line: &str) -> (bool, Rule<'_>) {
    let line = line.trim();
    if line.is_empty() || line.starts_with('!') || line.starts_with('#') || line.contains('$') {
        return (false, Rule::Invalid);
    }

    let (exception, body) = if let Some(body) = line.strip_prefix("@@") {
        (true, body)
    } else {
        (false, line)
    };

    let body = body.strip_prefix("||").unwrap_or(body);
    let body = body.strip_suffix('^').unwrap_or(body);
    let (host, path) = match body.split_once('/') {
        Some((host, path)) => (host, Some(path)),
        None => (body, None),
    };
    let host = host.trim_end_matches('.');
    if host.is_empty()
        || !host.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
        || host.starts_with('.')
        || host.ends_with('.')
    {
        return (exception, Rule::Invalid);
    }

    (exception, Rule::Domain { host, path })
}

fn host_matches(host: &str, rule_host: &str) -> bool {
    host.eq_ignore_ascii_case(rule_host)
        || host.strip_suffix(rule_host).is_some_and(|prefix| prefix.ends_with('.'))
}

fn rule_matches(url: &url::Url, rule: &Rule<'_>) -> bool {
    let Rule::Domain { host: rule_host, path } = rule else {
        return false;
    };
    let Some(host) = url.host_str() else {
        return false;
    };
    if !host_matches(&host.trim_end_matches('.').to_ascii_lowercase(), rule_host) {
        return false;
    }
    match path {
        Some(path) => url.path().trim_start_matches('/').starts_with(path),
        None => true,
    }
}

/// Returns true when a URL matches the built-in ad/tracker rules.
pub fn is_blocked_url(url: &url::Url) -> bool {
    is_blocked_url_with_rules(url, "")
}

/// Evaluate built-in rules plus newline-separated custom EasyList-subset rules.
/// Exception rules take precedence over blocking rules.
pub fn is_blocked_url_with_rules(url: &url::Url, custom_rules: &str) -> bool {
    let mut blocked = false;
    for line in BUILTIN_RULES.iter().copied().chain(custom_rules.lines()) {
        let (exception, rule) = parse_rule(line);
        if rule_matches(url, &rule) {
            if exception {
                return false;
            }
            blocked = true;
        }
    }
    blocked
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(value: &str) -> url::Url {
        url::Url::parse(value).unwrap()
    }

    #[test]
    fn blocks_builtin_exact_hosts_and_subdomains() {
        assert!(is_blocked_url(&url("https://doubleclick.net/ad.js")));
        assert!(is_blocked_url(&url("https://pagead.googlesyndication.com/x")));
    }

    #[test]
    fn does_not_block_lookalike_hosts_or_first_party_content() {
        assert!(!is_blocked_url(&url("https://notdoubleclick.net/")));
        assert!(!is_blocked_url(&url("https://example.com/")));
        assert!(!is_blocked_url(&url("data:text/plain,hello")));
    }

    #[test]
    fn parses_domain_and_path_rules() {
        let rules = "||ads.example.com^\n||cdn.example.net/ads/";
        assert!(is_blocked_url_with_rules(&url("https://ads.example.com/banner"), rules));
        assert!(is_blocked_url_with_rules(&url("https://sub.ads.example.com/banner"), rules));
        assert!(is_blocked_url_with_rules(&url("https://cdn.example.net/ads/banner.js"), rules));
        assert!(!is_blocked_url_with_rules(&url("https://cdn.example.net/assets/banner.js"), rules));
        assert!(!is_blocked_url_with_rules(&url("https://notads.example.com/"), rules));
    }

    #[test]
    fn exception_rules_override_block_rules() {
        let rules = "||example.com^\n@@||safe.example.com^";
        assert!(is_blocked_url_with_rules(&url("https://ads.example.com/"), rules));
        assert!(!is_blocked_url_with_rules(&url("https://safe.example.com/"), rules));
    }

    #[test]
    fn ignores_comments_and_unsupported_modifiers() {
        let rules = "! comment\n# heading\n||ads.example.com^$script";
        assert!(!is_blocked_url_with_rules(&url("https://ads.example.com/"), rules));
    }
}
