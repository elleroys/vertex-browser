//! Small subset of EasyList network filter matching.
//!
//! Supported rules:
//! - `||example.com^` — a domain and its subdomains.
//! - `||example.com/path^` — a domain/path prefix with separator boundary.
//! - `@@||example.com^` — an exception rule.
//! - `||example.com^$third-party,image` — resource-type restrictions.
//! - Plain host names such as `ads.example.com`.
//!
//! Cosmetic rules, regular expressions, unsupported modifiers, scriptlets and
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ResourceType {
    Script,
    Image,
    Stylesheet,
    Font,
    Media,
    Subdocument,
    Object,
    Document,
    XmlHttpRequest,
}

impl ResourceType {
    fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "script" => Self::Script,
            "image" => Self::Image,
            "stylesheet" => Self::Stylesheet,
            "font" => Self::Font,
            "media" => Self::Media,
            "subdocument" => Self::Subdocument,
            "object" => Self::Object,
            "document" => Self::Document,
            "xmlhttprequest" => Self::XmlHttpRequest,
            _ => return None,
        })
    }

    fn matches(self, destination: content_security_policy::Destination) -> bool {
        use content_security_policy::Destination as D;
        match self {
            Self::Script => destination.is_script_like(),
            Self::Image => destination == D::Image,
            Self::Stylesheet => destination == D::Style,
            Self::Font => destination == D::Font,
            Self::Media => matches!(destination, D::Audio | D::Video | D::Track),
            Self::Subdocument => matches!(destination, D::Frame | D::IFrame),
            Self::Object => matches!(destination, D::Object | D::Embed),
            Self::Document => destination == D::Document,
            Self::XmlHttpRequest => destination == D::None,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct ResourceTypeFilter {
    included: Vec<ResourceType>,
    excluded: Vec<ResourceType>,
}

impl ResourceTypeFilter {
    fn parse(value: &str) -> Option<Self> {
        if value.is_empty() {
            return None;
        }
        let mut included = Vec::new();
        let mut excluded = Vec::new();
        for token in value.split(',') {
            let (is_excluded, name) = match token.strip_prefix('~') {
                Some(name) => (true, name),
                None => (false, token),
            };
            let resource_type = ResourceType::parse(name)?;
            let target = if is_excluded { &mut excluded } else { &mut included };
            if !target.contains(&resource_type) {
                target.push(resource_type);
            }
        }
        Some(Self { included, excluded })
    }

    fn matches(&self, destination: content_security_policy::Destination) -> bool {
        if self.excluded.iter().any(|kind| kind.matches(destination)) {
            return false;
        }
        self.included.is_empty() || self.included.iter().any(|kind| kind.matches(destination))
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Rule {
    Domain {
        host: String,
        path: Option<String>,
        path_separator: bool,
        resource_types: Option<ResourceTypeFilter>,
    },
}

fn parse_rule(line: &str) -> Option<(bool, Rule)> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('!') || line.starts_with('#') {
        return None;
    }

    let (body, resource_types) = match line.split_once('$') {
        Some((body, modifier)) => (body, Some(ResourceTypeFilter::parse(modifier)?)),
        None => (line, None),
    };

    let (exception, body) = if let Some(body) = body.strip_prefix("@@") {
        (true, body)
    } else {
        (false, body)
    };

    let body = body.strip_prefix("||").unwrap_or(body);
    let (body, path_separator) = match body.strip_suffix('^') {
        Some(body) => (body, true),
        None => (body, false),
    };
    let (host, path) = match body.split_once('/') {
        Some((host, path)) if !path.is_empty() => (host, Some(path)),
        Some(_) => return None,
        None => (body, None),
    };
    let host = host.trim_end_matches('.');
    if host.is_empty()
        || !host.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
        || host.starts_with('.')
        || host.ends_with('.')
    {
        return None;
    }

    Some((exception, Rule::Domain {
        host: host.to_owned(),
        path: path.map(str::to_owned),
        path_separator: path.is_some() && path_separator,
        resource_types,
    }))
}

fn host_matches(host: &str, rule_host: &str) -> bool {
    if host.eq_ignore_ascii_case(rule_host) || host.len() <= rule_host.len() {
        return host.eq_ignore_ascii_case(rule_host);
    }
    let suffix_start = host.len() - rule_host.len();
    host.as_bytes()[suffix_start - 1] == b'.'
        && host[suffix_start..].eq_ignore_ascii_case(rule_host)
}

fn is_filter_separator(character: char) -> bool {
    !character.is_ascii_alphanumeric() && !matches!(character, '_' | '-' | '.' | '%')
}

// Match a filter-list path prefix containing '*' wildcards. Filter patterns are ASCII,
// and URL paths are percent-encoded ASCII, so byte offsets are safe here.
fn wildcard_prefix_end(value: &str, pattern: &str) -> Option<usize> {
    if !pattern.contains('*') {
        return value.strip_prefix(pattern).map(str::len);
    }

    let parts = pattern.split('*').collect::<Vec<_>>();
    let mut cursor = 0usize;
    let mut first = true;
    for part in parts.iter().filter(|part| !part.is_empty()) {
        if first && !pattern.starts_with('*') {
            if !value[cursor..].starts_with(part) {
                return None;
            }
            cursor += part.len();
        } else {
            let offset = value[cursor..].find(part)?;
            cursor += offset + part.len();
        }
        first = false;
    }
    Some(cursor)
}

fn rule_matches(
    url: &url::Url,
    destination: content_security_policy::Destination,
    rule: &Rule,
) -> bool {
    let Rule::Domain {
        host: rule_host,
        path,
        path_separator,
        resource_types,
    } = rule;
    if resource_types.as_ref().is_some_and(|filter| !filter.matches(destination)) {
        return false;
    }
    let Some(host) = url.host_str() else {
        return false;
    };
    if !host_matches(host.trim_end_matches('.'), rule_host) {
        return false;
    }
    let Some(path) = path else {
        return true;
    };
    let url_path = url.path().trim_start_matches('/');
    let Some(matched_end) = wildcard_prefix_end(url_path, path) else {
        return false;
    };
    let remainder = &url_path[matched_end..];
    if !path_separator || remainder.is_empty() {
        return true;
    }
    remainder.chars().next().is_some_and(is_filter_separator)
}

/// A parsed set of built-in and custom network rules.
pub struct RuleSet {
    rules: Vec<(bool, Rule)>,
}

impl RuleSet {
    /// Parse built-in rules and newline-separated custom EasyList-subset rules once.
    pub fn new(custom_rules: &str) -> Self {
        let rules = BUILTIN_RULES
            .iter()
            .copied()
            .chain(custom_rules.lines())
            .filter_map(parse_rule)
            .collect();
        Self { rules }
    }

    /// Evaluate a URL against this pre-parsed rule set.
    /// Exception rules take precedence over blocking rules.
    pub fn is_blocked(&self, url: &url::Url) -> bool {
        self.is_blocked_resource(url, content_security_policy::Destination::None)
    }

    /// Evaluate a URL and its Fetch destination against this pre-parsed rule set.
    pub fn is_blocked_resource(
        &self,
        url: &url::Url,
        destination: content_security_policy::Destination,
    ) -> bool {
        let mut blocked = false;
        for (exception, rule) in &self.rules {
            if rule_matches(url, destination, rule) {
                if *exception {
                    return false;
                }
                blocked = true;
            }
        }
        blocked
    }
}


/// Maximum accepted remote filter-list size (5 MiB).
pub const MAX_FILTER_LIST_BYTES: usize = 5 * 1024 * 1024;

/// Fetch a plain-text filter list without blocking the browser's UI thread.
/// Only HTTPS URLs and HTTPS redirects are accepted.
pub fn fetch_filter_list(raw_url: &str) -> Result<String, String> {
    use std::io::Read;
    use std::time::Duration;

    let url = url::Url::parse(raw_url.trim())
        .map_err(|_| "Некоректна URL-адреса списку".to_owned())?;
    if url.scheme() != "https" || url.host_str().is_none()
        || !url.username().is_empty() || url.password().is_some()
    {
        return Err("Список фільтрів має бути доступний за безпечною HTTPS-адресою".to_owned());
    }

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(25))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= 5 {
                return attempt.stop();
            }
            if attempt.url().scheme() != "https" {
                return attempt.stop();
            }
            attempt.follow()
        }))
        .build()
        .map_err(|error| format!("Не вдалося створити HTTP-клієнт: {error}"))?;
    let mut response = client.get(url).send()
        .map_err(|error| format!("Помилка завантаження списку: {error}"))?;
    if !response.status().is_success() {
        return Err(format!("Сервер повернув HTTP {}", response.status()));
    }
    if response.url().scheme() != "https" {
        return Err("Сервер перенаправив на незахищене з'єднання".to_owned());
    }
    if response.content_length().is_some_and(|size| size > MAX_FILTER_LIST_BYTES as u64) {
        return Err("Список завеликий: максимум 5 МБ".to_owned());
    }

    let mut bytes = Vec::new();
    response.take((MAX_FILTER_LIST_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("Не вдалося прочитати список: {error}"))?;
    if bytes.len() > MAX_FILTER_LIST_BYTES {
        return Err("Список завеликий: максимум 5 МБ".to_owned());
    }
    let text = String::from_utf8(bytes)
        .map_err(|_| "Список має бути текстом у кодуванні UTF-8".to_owned())?;
    let supported = text.lines().filter(|line| parse_rule(line).is_some()).count();
    if supported == 0 {
        return Err("У файлі не знайдено правил підтримуваного формату".to_owned());
    }
    Ok(text)
}

/// Merge imported filter-list text into the editable custom rules.
/// Only rules understood by this matcher are counted as supported; comments and
/// unsupported EasyList syntax are retained in the editor but do not affect matching.
pub fn merge_imported_rules(existing: &str, imported: &str) -> (String, usize, usize) {
    use std::collections::HashSet;

    let mut merged = existing.lines().map(str::to_owned).collect::<Vec<_>>();
    let mut seen = merged.iter().map(|line| line.trim().to_owned()).collect::<HashSet<_>>();
    let mut supported = 0usize;
    let mut ignored = 0usize;

    for line in imported.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || !seen.insert(trimmed.to_owned()) {
            continue;
        }
        if parse_rule(trimmed).is_some() {
            supported += 1;
        } else if !trimmed.starts_with('!') && !trimmed.starts_with('#') {
            ignored += 1;
        }
        merged.push(trimmed.to_owned());
    }

    (merged.join("\n"), supported, ignored)
}

/// Returns true when a URL matches the built-in ad/tracker rules.
pub fn is_blocked_url(url: &url::Url) -> bool {
    RuleSet::new("").is_blocked(url)
}

/// Evaluate built-in rules plus newline-separated custom EasyList-subset rules.
pub fn is_blocked_url_with_rules(url: &url::Url, custom_rules: &str) -> bool {
    RuleSet::new(custom_rules).is_blocked(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(value: &str) -> url::Url {
        url::Url::parse(value).unwrap()
    }

    #[test]
    fn imported_rules_are_merged_without_duplicates_and_report_unsupported_syntax() {
        let (merged, supported, ignored) = merge_imported_rules(
            "||existing.example^",
            "! EasyList comment\n||ads.example.com^\n||existing.example^\n||ads.example.com^$third-party\n",
        );
        assert_eq!(supported, 1);
        assert_eq!(ignored, 1);
        assert_eq!(merged.lines().filter(|line| *line == "||existing.example^").count(), 1);
        assert_eq!(merged.lines().filter(|line| *line == "||ads.example.com^").count(), 1);
        assert!(RuleSet::new(&merged).is_blocked(&url("https://ads.example.com/banner")));
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
    fn path_wildcards_match_filter_list_segments_and_respect_separators() {
        let rules = "||cdn.example.net/ads/*/banner^\n||static.example.org/assets/*";
        assert!(is_blocked_url_with_rules(
            &url("https://cdn.example.net/ads/mobile/banner.js"), rules
        ));
        assert!(!is_blocked_url_with_rules(
            &url("https://cdn.example.net/ads/mobile/bannerExtra.js"), rules
        ));
        assert!(is_blocked_url_with_rules(
            &url("https://static.example.org/assets/v2/app.css"), rules
        ));
        assert!(!is_blocked_url_with_rules(
            &url("https://static.example.org/other/v2/app.css"), rules
        ));
    }

    #[test]
    fn host_matching_is_case_insensitive_and_label_bounded() {
        let rules = "||Ads.Example.COM^";
        assert!(is_blocked_url_with_rules(&url("https://ADS.EXAMPLE.COM/"), rules));
        assert!(is_blocked_url_with_rules(&url("https://sub.ads.example.com/"), rules));
        assert!(!is_blocked_url_with_rules(&url("https://notads.example.com/"), rules));
    }

    #[test]
    fn path_separator_does_not_match_longer_words() {
        let rules = "||example.com/ads^";
        assert!(is_blocked_url_with_rules(&url("https://example.com/ads"), rules));
        assert!(is_blocked_url_with_rules(&url("https://example.com/ads/banner.js"), rules));
        assert!(!is_blocked_url_with_rules(&url("https://example.com/adserver.js"), rules));
    }

    #[test]
    fn compiled_rules_match_the_string_helper() {
        let rules = "||ads.example.com^\n@@||safe.ads.example.com^";
        let compiled = RuleSet::new(rules);
        for value in [
            "https://ads.example.com/banner.js",
            "https://safe.ads.example.com/banner.js",
            "https://example.org/",
        ] {
            let parsed = url(value);
            assert_eq!(
                compiled.is_blocked(&parsed),
                is_blocked_url_with_rules(&parsed, rules)
            );
        }
    }

    #[test]
    fn exception_rules_override_block_rules() {
        let rules = "||example.com^\n@@||safe.example.com^";
        assert!(is_blocked_url_with_rules(&url("https://ads.example.com/"), rules));
        assert!(!is_blocked_url_with_rules(&url("https://safe.example.com/"), rules));
    }

    #[test]
    fn ignores_comments_and_unsupported_modifiers() {
        let rules = "! comment\n# heading\n||ads.example.com^$third-party";
        assert!(!is_blocked_url_with_rules(&url("https://ads.example.com/"), rules));
    }

    #[test]
    fn resource_type_modifiers_match_servo_fetch_destinations() {
        use content_security_policy::Destination as D;
        let rules = RuleSet::new(
            "||cdn.example.net^$third-party,image\n||assets.example.net^$stylesheet\n||api.example.net^$xmlhttprequest"
        );
        assert!(rules.is_blocked_resource(&url("https://cdn.example.net/ad.js"), D::Script));
        assert!(rules.is_blocked_resource(&url("https://cdn.example.net/ad.png"), D::Image));
        assert!(!rules.is_blocked_resource(&url("https://cdn.example.net/page"), D::Document));
        assert!(rules.is_blocked_resource(&url("https://assets.example.net/site.css"), D::Style));
        assert!(!rules.is_blocked_resource(&url("https://assets.example.net/font.woff2"), D::Font));
        assert!(rules.is_blocked_resource(&url("https://api.example.net/data"), D::None));
        assert!(!rules.is_blocked_resource(&url("https://api.example.net/data"), D::Script));
    }

    #[test]
    fn negated_resource_type_modifiers_exclude_only_the_named_type() {
        use content_security_policy::Destination as D;
        let rules = RuleSet::new("||cdn.example.net^$~image");
        assert!(rules.is_blocked_resource(&url("https://cdn.example.net/ad.js"), D::Script));
        assert!(!rules.is_blocked_resource(&url("https://cdn.example.net/ad.png"), D::Image));
    }

    #[test]
    fn unsupported_resource_modifiers_are_ignored_as_a_whole() {
        use content_security_policy::Destination as D;
        let rules = RuleSet::new("||cdn.example.net^$third-party,third-party");
        assert!(!rules.is_blocked_resource(&url("https://cdn.example.net/ad.js"), D::Script));
    }

    #[test]
    fn path_exceptions_override_only_the_matching_path() {
        let rules = "||cdn.example.net/ads/*/banner^\n@@||cdn.example.net/ads/safe/banner^";
        assert!(is_blocked_url_with_rules(
            &url("https://cdn.example.net/ads/mobile/banner.js"), rules
        ));
        assert!(!is_blocked_url_with_rules(
            &url("https://cdn.example.net/ads/safe/banner.js"), rules
        ));
        assert!(is_blocked_url_with_rules(
            &url("https://cdn.example.net/ads/safe/other.js"), rules
        ));
    }

    #[test]
    fn wildcard_path_exceptions_take_precedence_independent_of_rule_order() {
        let blocked_then_allowed =
            "||cdn.example.net/ads/*\n@@||cdn.example.net/ads/*/trusted^";
        let allowed_then_blocked =
            "@@||cdn.example.net/ads/*/trusted^\n||cdn.example.net/ads/*";
        for rules in [blocked_then_allowed, allowed_then_blocked] {
            assert!(!is_blocked_url_with_rules(
                &url("https://cdn.example.net/ads/mobile/trusted.js"), rules
            ));
            assert!(is_blocked_url_with_rules(
                &url("https://cdn.example.net/ads/mobile/banner.js"), rules
            ));
        }
    }
}
