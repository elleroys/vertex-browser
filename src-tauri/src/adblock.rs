use crate::db::Database;
use crate::error::VortexResult;
use adblock::lists::{FilterSet, ParseOptions};
use adblock::request::Request;
use adblock::Engine;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use url::Url;

/// Вбудований базовий набір правил у форматі EasyList/EasyPrivacy для миттєвого запуску без мережі.
const BUNDLED_BASE_RULES: &str = r#"
||doubleclick.net^
||googlesyndication.com^
||googleadservices.com^
||adservice.google.com^
||pagead2.googlesyndication.com^
||google-analytics.com^
||googletagmanager.com^
||connect.facebook.net^
||an.facebook.com^
||ads.twitter.com^
||analytics.twitter.com^
||adnxs.com^
||criteo.com^
||criteo.net^
||taboola.com^
||outbrain.com^
||pubmatic.com^
||rubiconproject.com^
||openx.net^
||amazon-adsystem.com^
||scorecardresearch.com^
||quantserve.com^
||hotjar.com^
||mixpanel.com^
||segment.io^
||segment.com^
||mc.yandex.ru^
||an.yandex.ru^
||ads.yahoo.com^
||advertising.com^
||zedo.com^
||popads.net^
||propellerads.com^
||mgid.com^
||adroll.com^
||chartbeat.com^
||newrelic.com^
||sentry.io^$third-party
/ads/banner
/adserver/
/pagead/
/tracking.js
/pixel.gif
"#;

pub const EASYLIST_URL: &str = "https://easylist.to/easylist/easylist.txt";
pub const EASYPRIVACY_URL: &str = "https://easylist.to/easylist/easyprivacy.txt";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdBlockDomainStatus {
    pub domain: String,
    pub global_enabled: bool,
    pub is_whitelisted: bool,
    pub is_active_on_site: bool,
    pub blocked_on_tab: u64,
    pub total_blocked: i64,
    pub whitelist: Vec<String>,
    pub rules_loaded: usize,
    pub last_updated_at: Option<i64>,
}

pub struct AdBlockEngine {
    engine: RwLock<Engine>,
    whitelist: RwLock<HashSet<String>>,
    global_enabled: RwLock<bool>,
    tab_counters: RwLock<HashMap<String, u64>>,
    rules_count: RwLock<usize>,
    last_updated_at: RwLock<Option<i64>>,
    cache_dir: PathBuf,
    db: Arc<Database>,
}

impl AdBlockEngine {
    pub fn new(db: Arc<Database>, cache_dir: PathBuf, global_enabled: bool) -> VortexResult<Self> {
        std::fs::create_dir_all(&cache_dir)?;
        let initial_whitelist: HashSet<String> = db
            .list_adblock_whitelist()?
            .into_iter()
            .map(|d| normalize_domain(&d))
            .collect();

        // Швидкий запуск на вбудованих базових правилах; великі списки EasyList/EasyPrivacy
        // підвантажуються асинхронно у фоновому потоці після старту вікна.
        let (engine, rules_count) = Self::compile_bundled();

        Ok(Self {
            engine: RwLock::new(engine),
            whitelist: RwLock::new(initial_whitelist),
            global_enabled: RwLock::new(global_enabled),
            tab_counters: RwLock::new(HashMap::new()),
            rules_count: RwLock::new(rules_count),
            last_updated_at: RwLock::new(None),
            cache_dir,
            db,
        })
    }

    fn compile_bundled() -> (Engine, usize) {
        let mut filter_set = FilterSet::new(false);
        let total_lines = BUNDLED_BASE_RULES.lines().count();
        filter_set.add_filter_list(BUNDLED_BASE_RULES.to_string(), ParseOptions::default());
        (Engine::new_with_filter_set(filter_set), total_lines)
    }

    fn compile_from_cache(cache_dir: &PathBuf) -> Option<(Engine, usize)> {
        let mut filter_set = FilterSet::new(false);
        let mut total_lines = 0usize;
        let mut loaded_external = false;

        for file_name in ["easylist.txt", "easyprivacy.txt"] {
            let path = cache_dir.join(file_name);
            if let Ok(content) = std::fs::read_to_string(&path) {
                total_lines += content.lines().count();
                filter_set.add_filter_list(content, ParseOptions::default());
                loaded_external = true;
            }
        }

        if loaded_external && total_lines > 0 {
            Some((Engine::new_with_filter_set(filter_set), total_lines))
        } else {
            None
        }
    }

    pub async fn load_cached_filters(&self) {
        let cache_dir = self.cache_dir.clone();
        if let Ok(Some((new_engine, total_lines))) =
            tokio::task::spawn_blocking(move || Self::compile_from_cache(&cache_dir)).await
        {
            *self.engine.write().unwrap() = new_engine;
            *self.rules_count.write().unwrap() = total_lines;
        }
    }

    pub fn needs_filter_update(&self) -> bool {
        let now = std::time::SystemTime::now();
        for file_name in ["easylist.txt", "easyprivacy.txt"] {
            let path = self.cache_dir.join(file_name);
            let Ok(meta) = std::fs::metadata(&path) else {
                return true;
            };
            if meta.len() == 0 {
                return true;
            }
            let Ok(modified) = meta.modified() else {
                return true;
            };
            if let Ok(age) = now.duration_since(modified) {
                if age.as_secs() > 86_400 {
                    return true;
                }
            }
        }
        false
    }

    pub fn set_global_enabled(&self, enabled: bool) {
        *self.global_enabled.write().unwrap() = enabled;
    }

    pub fn is_domain_whitelisted(&self, domain: &str) -> bool {
        let normalized = normalize_domain(domain);
        if normalized.is_empty() {
            return false;
        }
        let wl = self.whitelist.read().unwrap();
        if wl.contains(&normalized) {
            return true;
        }
        // Перевірка батьківських доменів (наприклад, якщо у винятках example.com, то news.example.com теж дозволено)
        let parts: Vec<&str> = normalized.split('.').collect();
        for i in 1..parts.len().saturating_sub(1) {
            let parent = parts[i..].join(".");
            if wl.contains(&parent) {
                return true;
            }
        }
        false
    }

    pub fn toggle_domain_whitelist(&self, domain: &str, disable_adblock: bool) -> VortexResult<bool> {
        let normalized = normalize_domain(domain);
        if normalized.is_empty() {
            return Ok(false);
        }
        if disable_adblock {
            self.db.add_adblock_whitelist(&normalized)?;
            self.whitelist.write().unwrap().insert(normalized);
            Ok(true)
        } else {
            self.db.remove_adblock_whitelist(&normalized)?;
            self.whitelist.write().unwrap().remove(&normalized);
            Ok(false)
        }
    }

    pub fn reset_tab_counter(&self, tab_id: &str) {
        self.tab_counters.write().unwrap().insert(tab_id.to_string(), 0);
    }

    /// Головний метод перехоплення мережевих запитів WebView2 (`WebResourceRequested`).
    /// Повертає `true`, якщо запит має бути заблокований на мережевому рівні.
    pub fn should_block_request(
        &self,
        tab_id: &str,
        request_url: &str,
        source_page_url: &str,
        resource_type: &str,
    ) -> bool {
        if !*self.global_enabled.read().unwrap() {
            return false;
        }

        let source_domain = extract_domain(source_page_url);
        if !source_domain.is_empty() && self.is_domain_whitelisted(&source_domain) {
            // Сайт у білому списку — не блокуємо жоден запит на цьому домені
            return false;
        }

        // Не блокуємо основну навігацію верхнього рівня (document), лише сторонні/рекламні ресурси або перевіряємо через Request
        let Ok(req) = Request::new(request_url, source_page_url, resource_type, "GET") else {
            return false;
        };

        let should_block = {
            let engine = self.engine.read().unwrap();
            engine.check_network_request(&req).should_block()
        };

        if should_block {
            {
                let mut counters = self.tab_counters.write().unwrap();
                let entry = counters.entry(tab_id.to_string()).or_insert(0);
                *entry += 1;
            }
            if !source_domain.is_empty() {
                let db = Arc::clone(&self.db);
                tauri::async_runtime::spawn(async move {
                    let _ = db.increment_adblock_stat(&source_domain, 1);
                });
            }
            return true;
        }

        false
    }

    pub fn get_status_for_url(&self, tab_id: &str, page_url: &str) -> VortexResult<AdBlockDomainStatus> {
        let domain = extract_domain(page_url);
        let global_enabled = *self.global_enabled.read().unwrap();
        let is_whitelisted = !domain.is_empty() && self.is_domain_whitelisted(&domain);
        let blocked_on_tab = *self.tab_counters.read().unwrap().get(tab_id).unwrap_or(&0);
        let total_blocked = self.db.total_adblock_blocked()?;
        let mut whitelist: Vec<String> = self.whitelist.read().unwrap().iter().cloned().collect();
        whitelist.sort();

        Ok(AdBlockDomainStatus {
            domain,
            global_enabled,
            is_whitelisted,
            is_active_on_site: global_enabled && !is_whitelisted,
            blocked_on_tab,
            total_blocked,
            whitelist,
            rules_loaded: *self.rules_count.read().unwrap(),
            last_updated_at: *self.last_updated_at.read().unwrap(),
        })
    }

    /// Автоматичне оновлення фільтрів EasyList та EasyPrivacy з офіційних джерел.
    pub async fn update_filter_lists(&self) -> VortexResult<usize> {
        let client = reqwest::Client::builder()
            .user_agent("VortexBrowser/1.0")
            .timeout(std::time::Duration::from_secs(20))
            .build()?;

        let mut texts: Vec<String> = Vec::new();
        let mut total_lines = 0usize;

        for (file_name, url) in [("easylist.txt", EASYLIST_URL), ("easyprivacy.txt", EASYPRIVACY_URL)] {
            if let Ok(resp) = client.get(url).send().await {
                if resp.status().is_success() {
                    if let Ok(text) = resp.text().await {
                        let _ = std::fs::write(self.cache_dir.join(file_name), &text);
                        total_lines += text.lines().count();
                        texts.push(text);
                    }
                }
            }
        }

        if total_lines > 0 {
            if let Ok(new_engine) = tokio::task::spawn_blocking(move || {
                let mut filter_set = FilterSet::new(false);
                for text in texts {
                    filter_set.add_filter_list(text, ParseOptions::default());
                }
                Engine::new_with_filter_set(filter_set)
            })
            .await
            {
                *self.engine.write().unwrap() = new_engine;
                *self.rules_count.write().unwrap() = total_lines;
                *self.last_updated_at.write().unwrap() = Some(chrono::Utc::now().timestamp_millis());
            }
        }

        Ok(*self.rules_count.read().unwrap())
    }
}

pub fn normalize_domain(input: &str) -> String {
    input
        .trim()
        .to_lowercase()
        .strip_prefix("www.")
        .unwrap_or(input.trim())
        .to_lowercase()
}

pub fn extract_domain(raw_url: &str) -> String {
    if let Ok(parsed) = Url::parse(raw_url) {
        if let Some(host) = parsed.host_str() {
            return normalize_domain(host);
        }
    }
    String::new()
}
