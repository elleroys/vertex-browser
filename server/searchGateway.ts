import crypto from 'node:crypto';

export interface SearchResultItem {
  title: string;
  url: string;
  snippet: string;
  source: string;
  favicon?: string;
}

export interface SearchGatewayResult {
  query: string;
  engineName: string;
  originalSearchUrl: string;
  featuredSummary?: {
    title: string;
    description?: string;
    extract: string;
    url: string;
    thumbnail?: string;
  };
  results: SearchResultItem[];
}

const UA = 'VortexBrowser/1.0 (https://vortex-browser.example; mailto:dev@vortex-browser.example)';

const STOP_WORDS = new Set([
  'скільки', 'як', 'що', 'хто', 'де', 'коли', 'чому', 'чи', 'який', 'яка', 'яке', 'які',
  'та', 'і', 'й', 'в', 'у', 'на', 'з', 'із', 'зі', 'до', 'про', 'для', 'від', 'за', 'над', 'під',
  'how', 'what', 'when', 'where', 'why', 'who', 'which', 'is', 'are', 'was', 'were',
  'the', 'a', 'an', 'of', 'in', 'on', 'to', 'for', 'with', 'much', 'many', 'does', 'do', 'did',
]);

export function parseSearchEngineUrl(rawUrl: string): { engineName: string; query: string } | null {
  try {
    const u = new URL(rawUrl);
    const host = u.hostname.toLowerCase().replace(/^www\./, '');

    if ((host === 'google.com' || host.startsWith('google.')) && u.pathname.startsWith('/search')) {
      const q = u.searchParams.get('q')?.trim();
      if (q) return { engineName: 'Google', query: q };
    }
    if (host === 'duckduckgo.com' || host === 'html.duckduckgo.com' || host === 'lite.duckduckgo.com') {
      const q = u.searchParams.get('q')?.trim();
      if (q) return { engineName: 'DuckDuckGo', query: q };
    }
    if (host === 'search.brave.com' && u.pathname.startsWith('/search')) {
      const q = u.searchParams.get('q')?.trim();
      if (q) return { engineName: 'Brave Search', query: q };
    }
    if (host === 'bing.com' && u.pathname.startsWith('/search')) {
      const q = u.searchParams.get('q')?.trim();
      if (q) return { engineName: 'Bing', query: q };
    }
  } catch {
    return null;
  }
  return null;
}

function stripTags(html: string): string {
  return html
    .replace(/<[^>]+>/g, ' ')
    .replace(/&amp;/g, '&')
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&quot;/g, '"')
    .replace(/&#39;/g, "'")
    .replace(/&nbsp;/g, ' ')
    .replace(/\s+/g, ' ')
    .trim();
}

function escapeHtml(str: string): string {
  return str
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

function extractMeaningfulTokens(query: string): string[] {
  return query
    .toLowerCase()
    .replace(/[^\p{L}\p{N}\s-]/gu, ' ')
    .split(/\s+/)
    .filter((w) => w.length >= 3 && !STOP_WORDS.has(w));
}

async function searchWikipedia(
  lang: 'uk' | 'en',
  query: string
): Promise<{
  pages: Array<{ key: string; title: string; excerpt: string; description?: string; thumbnailUrl?: string }>;
}> {
  const fetchWiki = async (term: string) => {
    try {
      const url = `https://${lang}.wikipedia.org/w/rest.php/v1/search/page?q=${encodeURIComponent(term)}&limit=8`;
      const r = await fetch(url, {
        headers: { 'User-Agent': UA, Accept: 'application/json' },
        signal: AbortSignal.timeout(6000),
      });
      if (!r.ok) return [];
      const data = (await r.json()) as {
        pages?: Array<{
          key: string;
          title: string;
          excerpt?: string;
          description?: string;
          thumbnail?: { url?: string };
        }>;
      };
      return (data.pages || []).map((p) => ({
        key: p.key,
        title: p.title,
        excerpt: stripTags(p.excerpt || ''),
        description: p.description,
        thumbnailUrl: p.thumbnail?.url
          ? p.thumbnail.url.startsWith('//')
            ? `https:${p.thumbnail.url}`
            : p.thumbnail.url
          : undefined,
      }));
    } catch {
      return [];
    }
  };

  const tokens = extractMeaningfulTokens(query);
  let pages = await fetchWiki(query);

  const scorePage = (p: { title: string; excerpt: string }) => {
    const titleLower = p.title.toLowerCase();
    const excerptLower = p.excerpt.toLowerCase();
    let score = 0;
    for (const t of tokens) {
      if (titleLower === t) score += 100;
      else if (titleLower.includes(t)) score += 40;
      else if (excerptLower.includes(t)) score += 10;
    }
    return score;
  };

  const hasStrongMatch = pages.some((p) => scorePage(p) >= 40);

  if (!hasStrongMatch && tokens.length > 0) {
    const candidates = Array.from(
      new Set([tokens.join(' '), ...tokens].filter((c) => c && c !== query.toLowerCase()))
    ).slice(0, 3);

    const extraSets = await Promise.all(candidates.map((c) => fetchWiki(c)));
    const merged = new Map<string, (typeof pages)[number]>();
    for (const set of extraSets) {
      for (const p of set) {
        if (!merged.has(p.key)) merged.set(p.key, p);
      }
    }

    const extraList = Array.from(merged.values());
    const relevantExtra = extraList.filter((p) => scorePage(p) > 0);
    if (relevantExtra.length > 0) {
      pages = relevantExtra;
    } else if (pages.length === 0) {
      pages = extraList;
    }
  }

  pages.sort((a, b) => scorePage(b) - scorePage(a));
  return { pages };
}

async function fetchWikiSummary(
  lang: 'uk' | 'en',
  key: string
): Promise<{ title: string; description?: string; extract: string; url: string; thumbnail?: string } | undefined> {
  try {
    const r = await fetch(
      `https://${lang}.wikipedia.org/api/rest_v1/page/summary/${encodeURIComponent(key)}`,
      {
        headers: { 'User-Agent': UA, Accept: 'application/json' },
        signal: AbortSignal.timeout(5000),
      }
    );
    if (!r.ok) return undefined;
    const data = (await r.json()) as {
      title?: string;
      description?: string;
      extract?: string;
      content_urls?: { desktop?: { page?: string } };
      thumbnail?: { source?: string };
    };
    if (!data.extract) return undefined;
    return {
      title: data.title || key,
      description: data.description,
      extract: data.extract,
      url: data.content_urls?.desktop?.page || `https://${lang}.wikipedia.org/wiki/${encodeURIComponent(key)}`,
      thumbnail: data.thumbnail?.source,
    };
  } catch {
    return undefined;
  }
}

async function searchBingRssRelevant(query: string, tokens: string[]): Promise<SearchResultItem[]> {
  try {
    const r = await fetch(
      `https://www.bing.com/search?q=${encodeURIComponent(query)}&format=rss`,
      {
        headers: {
          'User-Agent':
            'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36',
        },
        signal: AbortSignal.timeout(5000),
      }
    );
    if (!r.ok) return [];
    const xml = await r.text();
    const items = [
      ...xml.matchAll(
        /<item><title>([\s\S]*?)<\/title><link>([\s\S]*?)<\/link><description>([\s\S]*?)<\/description>/gi
      ),
    ];

    const out: SearchResultItem[] = [];
    for (const it of items) {
      const title = stripTags(it[1]);
      const url = it[2].trim();
      const snippet = stripTags(it[3]);
      const haystack = `${title} ${url} ${snippet}`.toLowerCase();

      // Відфільтровуємо нерелевантний datacenter-спам, якщо жодне ключове слово запиту не збігається
      const matched =
        tokens.length === 0 ||
        tokens.some((t) => haystack.includes(t) || haystack.includes(t.slice(0, Math.max(3, t.length - 2))));
      if (!matched) continue;

      let domain = '';
      try {
        domain = new URL(url).hostname;
      } catch {
        // ignore
      }
      out.push({
        title,
        url,
        snippet,
        source: 'Веб-пошук',
        favicon: domain ? `https://www.google.com/s2/favicons?domain=${encodeURIComponent(domain)}&sz=32` : undefined,
      });
    }
    return out;
  } catch {
    return [];
  }
}

async function searchHackerNewsAndGitHub(query: string): Promise<SearchResultItem[]> {
  const results: SearchResultItem[] = [];
  // Виконуємо для латинських або технічних запитів
  if (!/[a-zA-Z]{2,}/.test(query)) return results;

  const [hnRes, ghRes] = await Promise.allSettled([
    fetch(`https://hn.algolia.com/api/v1/search?query=${encodeURIComponent(query)}&hitsPerPage=4`, {
      headers: { 'User-Agent': UA },
      signal: AbortSignal.timeout(4500),
    }).then((r) => (r.ok ? r.json() : null)),
    fetch(`https://api.github.com/search/repositories?q=${encodeURIComponent(query)}&per_page=3`, {
      headers: { 'User-Agent': UA, Accept: 'application/vnd.github+json' },
      signal: AbortSignal.timeout(4500),
    }).then((r) => (r.ok ? r.json() : null)),
  ]);

  if (ghRes.status === 'fulfilled' && ghRes.value?.items) {
    for (const repo of ghRes.value.items as Array<{
      full_name: string;
      html_url: string;
      description?: string;
      stargazers_count?: number;
    }>) {
      results.push({
        title: `${repo.full_name} — GitHub`,
        url: repo.html_url,
        snippet: `${repo.description || 'Репозиторій GitHub'} · ★ ${repo.stargazers_count ?? 0}`,
        source: 'GitHub',
        favicon: 'https://www.google.com/s2/favicons?domain=github.com&sz=32',
      });
    }
  }

  if (hnRes.status === 'fulfilled' && hnRes.value?.hits) {
    for (const hit of hnRes.value.hits as Array<{
      title?: string;
      url?: string;
      objectID: string;
      points?: number;
    }>) {
      if (!hit.title || !hit.url) continue;
      results.push({
        title: hit.title,
        url: hit.url,
        snippet: `Обговорення на Hacker News · ${hit.points ?? 0} балів`,
        source: 'Hacker News',
        favicon: 'https://www.google.com/s2/favicons?domain=news.ycombinator.com&sz=32',
      });
    }
  }

  return results;
}

export async function runWebSearchGateway(
  query: string,
  engineName: string,
  originalSearchUrl: string
): Promise<SearchGatewayResult> {
  const hasCyrillic = /[а-яіїєґ]/i.test(query);
  const primaryLang: 'uk' | 'en' = hasCyrillic ? 'uk' : 'en';
  const tokens = extractMeaningfulTokens(query);

  const [wikiPrimary, bingResults, devResults] = await Promise.all([
    searchWikipedia(primaryLang, query),
    searchBingRssRelevant(query, tokens),
    searchHackerNewsAndGitHub(query),
  ]);

  let wikiPages = wikiPrimary.pages;
  let wikiLang: 'uk' | 'en' = primaryLang;

  if (wikiPages.length === 0 && primaryLang === 'uk') {
    const fallbackEn = await searchWikipedia('en', query);
    if (fallbackEn.pages.length > 0) {
      wikiPages = fallbackEn.pages;
      wikiLang = 'en';
    }
  }

  let featuredSummary: SearchGatewayResult['featuredSummary'] = undefined;
  if (wikiPages.length > 0) {
    // Вибираємо найкращу статтю (віддаємо перевагу збігу в назві)
    const bestPage =
      wikiPages.find((p) => tokens.some((t) => p.title.toLowerCase().includes(t))) || wikiPages[0];
    featuredSummary = await fetchWikiSummary(wikiLang, bestPage.key);
  }

  const wikiItems: SearchResultItem[] = wikiPages.map((p) => ({
    title: `${p.title} — Вікіпедія`,
    url: `https://${wikiLang}.wikipedia.org/wiki/${encodeURIComponent(p.key)}`,
    snippet: p.excerpt || p.description || `Стаття Вікіпедії про «${p.title}»`,
    source: wikiLang === 'uk' ? 'Вікіпедія (UK)' : 'Wikipedia (EN)',
    favicon: `https://www.google.com/s2/favicons?domain=${wikiLang}.wikipedia.org&sz=32`,
  }));

  // Комбінуємо результати без дублікатів URL
  const seenUrls = new Set<string>();
  const combined: SearchResultItem[] = [];
  for (const item of [...bingResults, ...wikiItems, ...devResults]) {
    if (seenUrls.has(item.url)) continue;
    seenUrls.add(item.url);
    combined.push(item);
  }

  return {
    query,
    engineName,
    originalSearchUrl,
    featuredSummary,
    results: combined,
  };
}

export function renderSearchResultsHtml(
  data: SearchGatewayResult,
  tabId: string,
  theme: 'dark' | 'light' = 'dark'
): string {
  const isDark = theme === 'dark';
  const bg = isDark ? '#0B0D11' : '#F4F6FA';
  const cardBg = isDark ? '#12161E' : '#FFFFFF';
  const border = isDark ? 'rgba(255,255,255,0.08)' : '#E2E8F0';
  const text = isDark ? '#E6EAF2' : '#0F172A';
  const muted = isDark ? '#8F98A8' : '#64748B';
  const linkColor = isDark ? '#60A5FA' : '#2563EB';
  const urlColor = isDark ? '#34D399' : '#059669';

  const nonce = crypto.randomBytes(8).toString('hex');

  const summaryHtml = data.featuredSummary
    ? `
      <section class="summary-card">
        <div class="summary-body">
          <div class="summary-kicker">Швидка довідка · Вікіпедія</div>
          <h2><a href="${escapeHtml(data.featuredSummary.url)}">${escapeHtml(data.featuredSummary.title)}</a></h2>
          ${data.featuredSummary.description ? `<div class="summary-desc">${escapeHtml(data.featuredSummary.description)}</div>` : ''}
          <p>${escapeHtml(data.featuredSummary.extract)}</p>
          <a class="summary-url" href="${escapeHtml(data.featuredSummary.url)}">${escapeHtml(data.featuredSummary.url)}</a>
        </div>
        ${data.featuredSummary.thumbnail ? `<img class="summary-thumb" src="${escapeHtml(data.featuredSummary.thumbnail)}" alt="${escapeHtml(data.featuredSummary.title)}" />` : ''}
      </section>
    `
    : '';

  const resultsHtml =
    data.results.length > 0
      ? data.results
          .map(
            (r) => `
          <article class="result-item">
            <div class="result-meta">
              ${r.favicon ? `<img src="${escapeHtml(r.favicon)}" alt="" class="favicon" />` : ''}
              <span class="result-url">${escapeHtml(r.url)}</span>
              <span class="sep">·</span>
              <span class="result-source">${escapeHtml(r.source)}</span>
            </div>
            <h3><a href="${escapeHtml(r.url)}">${escapeHtml(r.title)}</a></h3>
            <p class="result-snippet">${escapeHtml(r.snippet)}</p>
          </article>
        `
          )
          .join('\n')
      : `
        <div class="empty-state">
          <h3>За запитом «${escapeHtml(data.query)}» прямих збігів не знайдено</h3>
          <p>Спробуйте уточнити формулювання або відкрити пошук у зовнішньому вікні за посиланням вище.</p>
        </div>
      `;

  return `<!doctype html>
<html lang="uk">
<head>
  <meta charset="UTF-8" />
  <title>${escapeHtml(data.query)} — Пошук Vortex</title>
  <style>
    * { box-sizing: border-box; }
    body {
      margin: 0;
      padding: 0;
      background: ${bg};
      color: ${text};
      font-family: 'Manrope', -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
      line-height: 1.55;
    }
    .container {
      max-width: 820px;
      margin: 0 auto;
      padding: 24px 20px 56px;
    }
    .search-header {
      display: flex;
      flex-wrap: wrap;
      align-items: center;
      gap: 12px;
      padding-bottom: 18px;
      border-bottom: 1px solid ${border};
      margin-bottom: 20px;
    }
    .search-form {
      display: flex;
      flex: 1;
      min-width: 260px;
      gap: 8px;
    }
    .search-input {
      flex: 1;
      height: 40px;
      padding: 0 14px;
      border-radius: 10px;
      border: 1px solid ${border};
      background: ${cardBg};
      color: ${text};
      font-size: 14px;
      outline: none;
    }
    .search-input:focus {
      border-color: #3B82F6;
    }
    .search-btn {
      height: 40px;
      padding: 0 18px;
      border-radius: 10px;
      border: none;
      background: #2563EB;
      color: #FFFFFF;
      font-size: 13px;
      font-weight: 600;
      cursor: pointer;
    }
    .search-btn:hover { background: #1D4ED8; }
    .meta-bar {
      display: flex;
      flex-wrap: wrap;
      align-items: center;
      justify-content: space-between;
      gap: 8px;
      font-size: 12px;
      color: ${muted};
      margin-bottom: 20px;
    }
    .ext-link {
      color: ${linkColor};
      text-decoration: none;
    }
    .ext-link:hover { text-decoration: underline; }
    .summary-card {
      display: flex;
      gap: 18px;
      padding: 18px 20px;
      border-radius: 12px;
      background: ${cardBg};
      border: 1px solid ${border};
      margin-bottom: 24px;
    }
    .summary-body { flex: 1; min-width: 0; }
    .summary-kicker {
      font-size: 12px;
      color: ${muted};
      margin-bottom: 4px;
    }
    .summary-card h2 {
      margin: 0 0 4px;
      font-size: 20px;
      font-weight: 600;
    }
    .summary-card h2 a { color: ${linkColor}; text-decoration: none; }
    .summary-card h2 a:hover { text-decoration: underline; }
    .summary-desc {
      font-size: 12px;
      color: ${muted};
      margin-bottom: 10px;
    }
    .summary-card p {
      margin: 0 0 10px;
      font-size: 14px;
    }
    .summary-url {
      font-size: 12px;
      font-family: monospace;
      color: ${urlColor};
      text-decoration: none;
      word-break: break-all;
    }
    .summary-thumb {
      width: 112px;
      height: 112px;
      object-fit: cover;
      border-radius: 10px;
      flex-shrink: 0;
    }
    .results-list {
      display: flex;
      flex-direction: column;
      gap: 20px;
    }
    .result-item {
      padding-bottom: 16px;
      border-bottom: 1px solid ${border};
    }
    .result-meta {
      display: flex;
      align-items: center;
      gap: 6px;
      font-size: 12px;
      color: ${muted};
      margin-bottom: 4px;
    }
    .favicon { width: 14px; height: 14px; border-radius: 2px; }
    .result-url {
      font-family: monospace;
      color: ${urlColor};
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
      max-width: 480px;
    }
    .result-item h3 {
      margin: 0 0 4px;
      font-size: 17px;
      font-weight: 600;
    }
    .result-item h3 a {
      color: ${linkColor};
      text-decoration: none;
    }
    .result-item h3 a:hover { text-decoration: underline; }
    .result-snippet {
      margin: 0;
      font-size: 13px;
      color: ${muted};
    }
    .empty-state {
      padding: 32px;
      border-radius: 12px;
      background: ${cardBg};
      border: 1px solid ${border};
      text-align: center;
    }
  </style>
</head>
<body>
  <div class="container">
    <div class="search-header">
      <form class="search-form" id="vortex-serp-form-${nonce}">
        <input class="search-input" id="vortex-serp-input-${nonce}" type="text" value="${escapeHtml(data.query)}" placeholder="Введіть пошуковий запит..." />
        <button class="search-btn" type="submit">Знайти</button>
      </form>
    </div>

    <div class="meta-bar">
      <span>Пошук Vortex (${escapeHtml(data.engineName)}) · Знайдено результатів: ${data.results.length}</span>
      <a class="ext-link" data-external="true" href="${escapeHtml(data.originalSearchUrl)}" target="_blank" rel="noopener noreferrer">
        Відкрити у ${escapeHtml(data.engineName)} напряму ↗
      </a>
    </div>

    ${summaryHtml}

    <div class="results-list">
      ${resultsHtml}
    </div>
  </div>

  <script>
    (function() {
      const form = document.getElementById('vortex-serp-form-${nonce}');
      const input = document.getElementById('vortex-serp-input-${nonce}');
      form.addEventListener('submit', function(e) {
        e.preventDefault();
        const q = (input.value || '').trim();
        if (!q) return;
        window.parent.postMessage({
          type: 'vortex:navigate',
          tabId: ${JSON.stringify(tabId)},
          url: q,
          newTab: false
        }, '*');
      });

      document.addEventListener('click', function(e) {
        const a = e.target && e.target.closest ? e.target.closest('a[href]') : null;
        if (!a) return;
        if (a.getAttribute('data-external') === 'true') return;
        const href = a.getAttribute('href');
        if (!href || href.startsWith('#')) return;
        e.preventDefault();
        window.parent.postMessage({
          type: 'vortex:navigate',
          tabId: ${JSON.stringify(tabId)},
          url: href,
          newTab: e.ctrlKey || e.metaKey
        }, '*');
      }, true);
    })();
  </script>
</body>
</html>`;
}
