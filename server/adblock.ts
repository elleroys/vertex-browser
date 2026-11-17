import fs from 'node:fs';
import path from 'node:path';
import { VortexDatabase } from './db.ts';

const BUNDLED_RULES = [
  '||doubleclick.net^',
  '||googlesyndication.com^',
  '||googleadservices.com^',
  '||adservice.google.com^',
  '||pagead2.googlesyndication.com^',
  '||google-analytics.com^',
  '||googletagmanager.com^',
  '||connect.facebook.net^',
  '||an.facebook.com^',
  '||ads.twitter.com^',
  '||analytics.twitter.com^',
  '||adnxs.com^',
  '||criteo.com^',
  '||criteo.net^',
  '||taboola.com^',
  '||outbrain.com^',
  '||pubmatic.com^',
  '||rubiconproject.com^',
  '||openx.net^',
  '||amazon-adsystem.com^',
  '||scorecardresearch.com^',
  '||quantserve.com^',
  '||hotjar.com^',
  '||mixpanel.com^',
  '||segment.io^',
  '||segment.com^',
  '||mc.yandex.ru^',
  '||an.yandex.ru^',
  '||ads.yahoo.com^',
  '||advertising.com^',
  '||zedo.com^',
  '||popads.net^',
  '||propellerads.com^',
  '||mgid.com^',
  '||adroll.com^',
  '||chartbeat.com^',
  '||amplitude.com^',
  '||fullstory.com^',
  '||clarity.ms^',
  '||ads.linkedin.com^',
  '||snap.licdn.com^',
  '||bat.bing.com^',
  '||tiktok.com/i18n/pixel^',
  '/ads/banner',
  '/adserver/',
  '/pagead/',
  '/tracking.js',
  '/pixel.gif',
];

export interface AdBlockStatus {
  domain: string;
  globalEnabled: boolean;
  isWhitelisted: boolean;
  isActiveOnSite: boolean;
  blockedOnTab: number;
  totalBlocked: number;
  whitelist: string[];
  rulesLoaded: number;
  lastUpdatedAt: number | null;
}

export class ServerAdBlockEngine {
  private blockedDomains = new Set<string>();
  private pathSubstrings: string[] = [];
  private rulesCount = 0;
  private lastUpdatedAt: number | null = null;
  private tabCounters = new Map<string, number>();
  private cacheDir: string;
  private db: VortexDatabase;

  constructor(db: VortexDatabase, cacheDir = 'data/adblock') {
    this.db = db;
    this.cacheDir = cacheDir;
    fs.mkdirSync(this.cacheDir, { recursive: true });
    this.loadRulesFromCacheOrBundled();
  }

  private parseRulesText(text: string): void {
    for (const rawLine of text.split(/\r?\n/)) {
      const line = rawLine.trim();
      if (!line || line.startsWith('!') || line.startsWith('[') || line.includes('##') || line.includes('#@#')) {
        continue;
      }
      const clean = line.split('$')[0].trim();
      if (clean.startsWith('||')) {
        const domain = clean.slice(2).replace(/\^.*$/, '').replace(/\/.*$/, '').toLowerCase();
        if (domain && domain.includes('.') && !domain.includes('*')) {
          this.blockedDomains.add(domain);
          this.rulesCount++;
        }
      } else if (clean.startsWith('/') && clean.length > 4 && !clean.includes('*')) {
        this.pathSubstrings.push(clean.toLowerCase());
        this.rulesCount++;
      }
    }
  }

  private loadRulesFromCacheOrBundled(): void {
    this.blockedDomains.clear();
    this.pathSubstrings = [];
    this.rulesCount = 0;

    let loadedCache = false;
    for (const file of ['easylist.txt', 'easyprivacy.txt']) {
      const fullPath = path.join(this.cacheDir, file);
      if (fs.existsSync(fullPath)) {
        const content = fs.readFileSync(fullPath, 'utf8');
        this.parseRulesText(content);
        const stat = fs.statSync(fullPath);
        this.lastUpdatedAt = stat.mtimeMs;
        loadedCache = true;
      }
    }
    if (!loadedCache) {
      this.parseRulesText(BUNDLED_RULES.join('\n'));
    }
  }

  async updateFilterLists(): Promise<{ rulesLoaded: number; lastUpdatedAt: number }> {
    const sources = [
      { file: 'easylist.txt', url: 'https://easylist.to/easylist/easylist.txt' },
      { file: 'easyprivacy.txt', url: 'https://easylist.to/easylist/easyprivacy.txt' },
    ];

    let anyUpdated = false;
    for (const src of sources) {
      try {
        const resp = await fetch(src.url, {
          headers: { 'User-Agent': 'VortexBrowser/1.0' },
          signal: AbortSignal.timeout(12000),
        });
        if (resp.ok) {
          const text = await resp.text();
          if (text.length > 5000) {
            fs.writeFileSync(path.join(this.cacheDir, src.file), text, 'utf8');
            anyUpdated = true;
          }
        }
      } catch {
        // Fallback cleanly to bundled/cached rules if offline
      }
    }

    if (anyUpdated) {
      this.loadRulesFromCacheOrBundled();
    } else {
      this.lastUpdatedAt = Date.now();
    }

    return { rulesLoaded: this.rulesCount, lastUpdatedAt: this.lastUpdatedAt || Date.now() };
  }

  public normalizeDomain(input: string): string {
    return input.trim().toLowerCase().replace(/^www\./, '');
  }

  public extractDomain(rawUrl: string): string {
    try {
      const parsed = new URL(rawUrl);
      return this.normalizeDomain(parsed.hostname);
    } catch {
      return '';
    }
  }

  public isDomainWhitelisted(domain: string): boolean {
    const normalized = this.normalizeDomain(domain);
    if (!normalized) return false;
    const wl = new Set(this.db.listWhitelist().map((d) => this.normalizeDomain(d)));
    if (wl.has(normalized)) return true;
    const parts = normalized.split('.');
    for (let i = 1; i < parts.length - 1; i++) {
      if (wl.has(parts.slice(i).join('.'))) return true;
    }
    return false;
  }

  public shouldBlockUrl(resourceUrl: string, pageDomain: string): boolean {
    const settings = this.db.getSettings();
    if (!settings.adblockEnabled) return false;
    if (pageDomain && this.isDomainWhitelisted(pageDomain)) return false;

    try {
      const parsed = new URL(resourceUrl);
      const host = this.normalizeDomain(parsed.hostname);
      if (this.blockedDomains.has(host)) return true;
      const parts = host.split('.');
      for (let i = 1; i < parts.length - 1; i++) {
        if (this.blockedDomains.has(parts.slice(i).join('.'))) return true;
      }
      const lowerPath = (parsed.pathname + parsed.search).toLowerCase();
      for (const sub of this.pathSubstrings) {
        if (lowerPath.includes(sub)) return true;
      }
    } catch {
      const lower = resourceUrl.toLowerCase();
      for (const sub of this.pathSubstrings) {
        if (lower.includes(sub)) return true;
      }
    }
    return false;
  }

  public filterHtmlResources(
    html: string,
    baseUrl: string,
    tabId: string
  ): { filteredHtml: string; blockedCount: number; blockedDomains: string[] } {
    const pageDomain = this.extractDomain(baseUrl);
    const settings = this.db.getSettings();
    const isActive = settings.adblockEnabled && !this.isDomainWhitelisted(pageDomain);

    if (!isActive) {
      this.tabCounters.set(tabId, 0);
      return { filteredHtml: html, blockedCount: 0, blockedDomains: [] };
    }

    let blockedCount = 0;
    const blockedHostSet = new Set<string>();

    const checkAndStrip = (rawSrc: string): boolean => {
      try {
        const abs = new URL(rawSrc, baseUrl).toString();
        if (this.shouldBlockUrl(abs, pageDomain)) {
          blockedCount++;
          const host = this.extractDomain(abs);
          if (host) blockedHostSet.add(host);
          return true;
        }
      } catch {
        // ignore invalid src
      }
      return false;
    };

    // Strip matching external scripts, tracking pixels/images, ad iframes
    let out = html.replace(/<script\b[^>]*\bsrc=["']([^"']+)["'][^>]*>[\s\S]*?<\/script>/gi, (full, src) => {
      return checkAndStrip(src) ? '<!-- blocked by Vortex AdBlock -->' : full;
    });

    out = out.replace(/<iframe\b[^>]*\bsrc=["']([^"']+)["'][^>]*>[\s\S]*?<\/iframe>/gi, (full, src) => {
      return checkAndStrip(src) ? '<!-- ad iframe blocked by Vortex AdBlock -->' : full;
    });

    out = out.replace(/<img\b[^>]*\bsrc=["']([^"']+)["'][^>]*>/gi, (full, src) => {
      return checkAndStrip(src) ? '' : full;
    });

    this.tabCounters.set(tabId, blockedCount);
    if (blockedCount > 0 && pageDomain) {
      this.db.incrementAdblockStat(pageDomain, blockedCount);
    }

    return {
      filteredHtml: out,
      blockedCount,
      blockedDomains: Array.from(blockedHostSet),
    };
  }

  public getStatus(tabId: string, pageUrl: string): AdBlockStatus {
    const settings = this.db.getSettings();
    const domain = this.extractDomain(pageUrl);
    const isWhitelisted = domain ? this.isDomainWhitelisted(domain) : false;
    const blockedOnTab = this.tabCounters.get(tabId) ?? 0;

    return {
      domain,
      globalEnabled: settings.adblockEnabled,
      isWhitelisted,
      isActiveOnSite: settings.adblockEnabled && !isWhitelisted,
      blockedOnTab,
      totalBlocked: this.db.getTotalBlocked(),
      whitelist: this.db.listWhitelist(),
      rulesLoaded: this.rulesCount,
      lastUpdatedAt: this.lastUpdatedAt,
    };
  }
}
