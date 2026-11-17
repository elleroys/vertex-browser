import express from 'express';
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import net from 'node:net';
import { createServer as createViteServer } from 'vite';
import { VortexDatabase, type BookmarkItem, type DownloadRecord } from './server/db.ts';
import { ServerAdBlockEngine } from './server/adblock.ts';
import { ServerPasswordVault } from './server/vault.ts';
import {
  parseSearchEngineUrl,
  renderSearchResultsHtml,
  runWebSearchGateway,
} from './server/searchGateway.ts';

const PORT = Number(process.env.PORT) || 3000;
const DATA_DIR = path.resolve(process.cwd(), 'data');
const DOWNLOADS_DIR = path.join(DATA_DIR, 'downloads');
fs.mkdirSync(DOWNLOADS_DIR, { recursive: true });

const db = new VortexDatabase(path.join(DATA_DIR, 'vortex.db'));
const adblock = new ServerAdBlockEngine(db, path.join(DATA_DIR, 'adblock'));
const vault = new ServerPasswordVault(db);

// Активні контролери завантажень для Pause / Speed
const activeDownloads = new Map<
  string,
  { controller: AbortController; speedBps: number; paused: boolean }
>();

function isSafePublicUrl(rawUrl: string): { ok: boolean; parsed?: URL; reason?: string } {
  let parsed: URL;
  try {
    parsed = new URL(rawUrl);
  } catch {
    return { ok: false, reason: 'Некоректний формат URL' };
  }
  if (parsed.protocol !== 'http:' && parsed.protocol !== 'https:') {
    return { ok: false, reason: 'Дозволені лише протоколи HTTP та HTTPS' };
  }
  const host = parsed.hostname.toLowerCase();
  if (
    host === 'localhost' ||
    host === '0.0.0.0' ||
    host.endsWith('.local') ||
    host.endsWith('.internal')
  ) {
    return { ok: false, reason: 'Доступ до локальних адрес заблоковано політикою безпеки' };
  }
  if (net.isIP(host)) {
    if (
      host.startsWith('127.') ||
      host.startsWith('10.') ||
      host.startsWith('192.168.') ||
      host.startsWith('169.254.') ||
      host === '::1' ||
      host.startsWith('fc') ||
      host.startsWith('fd')
    ) {
      return { ok: false, reason: 'Приватні IP-діапазони заблоковано політикою безпеки' };
    }
    const octets = host.split('.').map(Number);
    if (octets[0] === 172 && octets[1] >= 16 && octets[1] <= 31) {
      return { ok: false, reason: 'Приватні IP-діапазони заблоковано політикою безпеки' };
    }
  }
  return { ok: true, parsed };
}

function escapeHtml(str: string): string {
  return str
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

function injectVortexBridge(html: string, baseUrl: string, tabId: string): string {
  const safeBase = escapeHtml(baseUrl);
  const bridgeScript = `
<script data-vortex-bridge="true">
(function() {
  const TAB_ID = ${JSON.stringify(tabId)};
  const PAGE_ORIGIN = ${JSON.stringify(new URL(baseUrl).hostname)};

  document.addEventListener('click', function(e) {
    const a = e.target && e.target.closest ? e.target.closest('a[href]') : null;
    if (!a) return;
    const href = a.getAttribute('href');
    if (!href || href.startsWith('#') || href.startsWith('javascript:')) return;
    try {
      const resolved = new URL(href, ${JSON.stringify(baseUrl)}).toString();
      e.preventDefault();
      window.parent.postMessage({
        type: 'vortex:navigate',
        tabId: TAB_ID,
        url: resolved,
        newTab: a.target === '_blank' || e.ctrlKey || e.metaKey
      }, '*');
    } catch (_) {}
  }, true);

  function scanLoginForms() {
    const pwdInputs = document.querySelectorAll('input[type="password"]');
    if (pwdInputs.length > 0) {
      window.parent.postMessage({
        type: 'vortex:login-form-detected',
        tabId: TAB_ID,
        origin: PAGE_ORIGIN,
        url: ${JSON.stringify(baseUrl)}
      }, '*');
    }
  }

  document.addEventListener('submit', function(e) {
    const form = e.target;
    if (!form || !form.querySelector) return;
    const pwd = form.querySelector('input[type="password"]');
    if (!pwd || !pwd.value) return;
    const userInput = form.querySelector('input[type="email"], input[type="text"], input[name*="user"], input[name*="login"], input[name*="email"]');
    window.parent.postMessage({
      type: 'vortex:login-submit',
      tabId: TAB_ID,
      origin: PAGE_ORIGIN,
      title: document.title || PAGE_ORIGIN,
      username: userInput ? userInput.value : '',
      password: pwd.value
    }, '*');
  }, true);

  window.addEventListener('message', function(ev) {
    if (!ev.data || ev.data.type !== 'vortex:autofill' || ev.data.tabId !== TAB_ID) return;
    const pwd = document.querySelector('input[type="password"]');
    const userInput = document.querySelector('input[type="email"], input[type="text"], input[name*="user"], input[name*="login"], input[name*="email"]');
    if (userInput && ev.data.username) {
      userInput.value = ev.data.username;
      userInput.dispatchEvent(new Event('input', { bubbles: true }));
    }
    if (pwd && ev.data.password) {
      pwd.value = ev.data.password;
      pwd.dispatchEvent(new Event('input', { bubbles: true }));
    }
  });

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', scanLoginForms);
  } else {
    scanLoginForms();
  }
})();
</script>`;

  let out = html;
  if (/<head[^>]*>/i.test(out)) {
    out = out.replace(/<head[^>]*>/i, (m) => `${m}\n<base href="${safeBase}">`);
  } else {
    out = `<head><base href="${safeBase}"></head>` + out;
  }

  if (/<\/body>/i.test(out)) {
    out = out.replace(/<\/body>/i, `${bridgeScript}\n</body>`);
  } else {
    out += bridgeScript;
  }
  return out;
}

async function runDownloadWorker(dl: DownloadRecord, resumeOffset = 0): Promise<void> {
  const controller = new AbortController();
  activeDownloads.set(dl.id, { controller, speedBps: 0, paused: false });

  try {
    const headers: Record<string, string> = {
      'User-Agent': 'VortexBrowser/1.0',
    };
    if (resumeOffset > 0) {
      headers['Range'] = `bytes=${resumeOffset}-`;
    }

    const resp = await fetch(dl.url, { headers, signal: controller.signal });
    if (!resp.ok && resp.status !== 206) {
      dl.status = 'failed';
      db.upsertDownload(dl);
      activeDownloads.delete(dl.id);
      return;
    }

    const acceptRanges = resp.headers.get('accept-ranges') || '';
    dl.supportsResume = acceptRanges.includes('bytes');
    dl.mimeType = resp.headers.get('content-type') || dl.mimeType;

    const contentLength = Number(resp.headers.get('content-length') || '0');
    if (resumeOffset === 0 && contentLength > 0) {
      dl.totalBytes = contentLength;
    } else if (resumeOffset > 0 && contentLength > 0 && dl.totalBytes === 0) {
      dl.totalBytes = resumeOffset + contentLength;
    }
    db.upsertDownload(dl);

    const writeStream = fs.createWriteStream(dl.savePath, {
      flags: resumeOffset > 0 ? 'a' : 'w',
    });

    const reader = resp.body?.getReader();
    if (!reader) {
      writeStream.end();
      dl.status = 'failed';
      db.upsertDownload(dl);
      activeDownloads.delete(dl.id);
      return;
    }

    let received = resumeOffset;
    let windowBytes = 0;
    let lastTick = Date.now();

    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      if (value) {
        writeStream.write(Buffer.from(value));
        received += value.byteLength;
        windowBytes += value.byteLength;
        const now = Date.now();
        if (now - lastTick >= 250) {
          const speedBps = Math.round((windowBytes * 1000) / Math.max(1, now - lastTick));
          const state = activeDownloads.get(dl.id);
          if (state) state.speedBps = speedBps;
          dl.receivedBytes = received;
          db.upsertDownload(dl);
          windowBytes = 0;
          lastTick = now;
        }
      }
    }

    await new Promise<void>((resolve) => writeStream.end(() => resolve()));
    dl.receivedBytes = received;
    if (dl.totalBytes === 0) dl.totalBytes = received;
    dl.status = 'completed';
    dl.completedAt = Date.now();
    db.upsertDownload(dl);
  } catch (err: unknown) {
    const state = activeDownloads.get(dl.id);
    if (state?.paused) {
      dl.status = 'paused';
    } else if (err instanceof Error && err.name === 'AbortError') {
      dl.status = 'paused';
    } else {
      dl.status = 'failed';
    }
    db.upsertDownload(dl);
  } finally {
    activeDownloads.delete(dl.id);
  }
}

async function startServer() {
  const app = express();
  app.use(express.json({ limit: '15mb' }));

  // --- Browse / Proxy з реальним AdBlock ---
  app.post('/api/browse', async (req, res) => {
    const { url, tabId = 'default', incognito = false } = req.body as {
      url?: string;
      tabId?: string;
      incognito?: boolean;
    };
    if (!url) {
      res.status(400).json({ error: 'Відсутній URL' });
      return;
    }

    const check = isSafePublicUrl(url);
    if (!check.ok || !check.parsed) {
      res.status(400).json({ error: check.reason || 'Некоректний URL' });
      return;
    }

    const settings = db.getSettings();
    const startMs = performance.now();
    try {
      const searchReq = parseSearchEngineUrl(check.parsed.toString());
      if (searchReq) {
        const searchData = await runWebSearchGateway(
          searchReq.query,
          searchReq.engineName,
          check.parsed.toString()
        );
        const html = renderSearchResultsHtml(searchData, tabId, settings.theme);
        const rawBytes = Buffer.byteLength(html, 'utf8');
        const loadTimeMs = Math.round(performance.now() - startMs);
        const domain = adblock.extractDomain(check.parsed.toString());
        const favicon = `https://www.google.com/s2/favicons?domain=${encodeURIComponent(domain)}&sz=32`;
        const title = `${searchReq.query} — ${searchReq.engineName}`;

        if (!incognito) {
          db.recordHistory(title, check.parsed.toString(), favicon);
        }

        res.json({
          finalUrl: check.parsed.toString(),
          title,
          favicon,
          isHttps: check.parsed.protocol === 'https:',
          status: 200,
          loadTimeMs,
          blockedCount: 0,
          blockedDomains: [],
          rawBytes,
          filteredBytes: rawBytes,
          contentType: 'text/html; charset=utf-8',
          html,
        });
        return;
      }

      const headers: Record<string, string> = {
        'User-Agent':
          'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Vortex/1.0 Chrome/130.0.0.0 Safari/537.36',
        Accept: 'text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8',
        'Accept-Language': 'uk-UA,uk;q=0.9,en-US;q=0.8,en;q=0.7',
      };
      if (settings.doNotTrack) {
        headers['DNT'] = '1';
      }

      const upstream = await fetch(check.parsed.toString(), {
        headers,
        redirect: 'follow',
        signal: AbortSignal.timeout(15000),
      });

      const finalUrl = upstream.url || check.parsed.toString();
      const contentType = upstream.headers.get('content-type') || 'text/html';
      const setCookie = upstream.headers.get('set-cookie');
      const finalDomain = adblock.extractDomain(finalUrl);
      if (setCookie && finalDomain && !incognito) {
        db.recordSiteData(finalDomain, 'session_cookies', Buffer.byteLength(setCookie));
      }

      if (!contentType.includes('text/html') && !contentType.includes('application/xhtml')) {
        const buf = Buffer.from(await upstream.arrayBuffer());
        const loadTimeMs = Math.round(performance.now() - startMs);
        res.json({
          finalUrl,
          title: finalUrl.split('/').pop() || finalDomain,
          favicon: `https://www.google.com/s2/favicons?domain=${encodeURIComponent(finalDomain)}&sz=32`,
          isHttps: finalUrl.startsWith('https://'),
          status: upstream.status,
          loadTimeMs,
          blockedCount: 0,
          blockedDomains: [],
          rawBytes: buf.byteLength,
          filteredBytes: buf.byteLength,
          contentType,
          html: `<body style="background:#0F1218;color:#E6EAF2;font-family:sans-serif;padding:32px;"><h2>Ресурс завантажено (${escapeHtml(contentType)})</h2><p>${escapeHtml(finalUrl)}</p><pre style="white-space:pre-wrap;background:#151922;padding:16px;border-radius:8px;overflow:auto;">${escapeHtml(buf.toString('utf8').slice(0, 12000))}</pre></body>`,
        });
        return;
      }

      const rawHtml = await upstream.text();
      if (
        rawHtml.includes('/httpservice/retry/enablejs') ||
        rawHtml.includes('Якщо вам не вдається отримати доступ до Пошуку Google') ||
        rawHtml.includes("If you're having trouble accessing Google Search")
      ) {
        const fallbackReq =
          parseSearchEngineUrl(finalUrl) ?? parseSearchEngineUrl(check.parsed.toString());
        if (fallbackReq) {
          const searchData = await runWebSearchGateway(
            fallbackReq.query,
            fallbackReq.engineName,
            finalUrl
          );
          const html = renderSearchResultsHtml(searchData, tabId, settings.theme);
          const rawBytes = Buffer.byteLength(html, 'utf8');
          const loadTimeMs = Math.round(performance.now() - startMs);
          const favicon = `https://www.google.com/s2/favicons?domain=${encodeURIComponent(finalDomain)}&sz=32`;
          const title = `${fallbackReq.query} — ${fallbackReq.engineName}`;
          if (!incognito) {
            db.recordHistory(title, finalUrl, favicon);
          }
          res.json({
            finalUrl,
            title,
            favicon,
            isHttps: finalUrl.startsWith('https://'),
            status: 200,
            loadTimeMs,
            blockedCount: 0,
            blockedDomains: [],
            rawBytes,
            filteredBytes: rawBytes,
            contentType: 'text/html; charset=utf-8',
            html,
          });
          return;
        }
      }

      const rawBytes = Buffer.byteLength(rawHtml, 'utf8');
      const { filteredHtml, blockedCount, blockedDomains } = adblock.filterHtmlResources(
        rawHtml,
        finalUrl,
        tabId
      );
      const filteredBytes = Buffer.byteLength(filteredHtml, 'utf8');

      const titleMatch = filteredHtml.match(/<title[^>]*>([\s\S]*?)<\/title>/i);
      const title = titleMatch
        ? titleMatch[1].replace(/\s+/g, ' ').trim()
        : finalDomain || finalUrl;

      const favicon = `https://www.google.com/s2/favicons?domain=${encodeURIComponent(finalDomain)}&sz=32`;
      const loadTimeMs = Math.round(performance.now() - startMs);

      if (!incognito) {
        db.recordHistory(title, finalUrl, favicon);
      }

      const bridgedHtml = injectVortexBridge(filteredHtml, finalUrl, tabId);

      res.json({
        finalUrl,
        title,
        favicon,
        isHttps: finalUrl.startsWith('https://'),
        status: upstream.status,
        loadTimeMs,
        blockedCount,
        blockedDomains,
        rawBytes,
        filteredBytes,
        contentType,
        html: bridgedHtml,
      });
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : 'Не вдалося завантажити сторінку';
      res.status(502).json({
        error: msg,
        finalUrl: url,
        loadTimeMs: Math.round(performance.now() - startMs),
      });
    }
  });

  // --- Tabs Session ---
  app.get('/api/session/tabs', (_req, res) => {
    res.json(db.loadSessionTabs());
  });

  app.post('/api/session/tabs', (req, res) => {
    const { tabs } = req.body;
    if (Array.isArray(tabs)) {
      db.saveSessionTabs(tabs);
    }
    res.json({ ok: true });
  });

  // --- AdBlock ---
  app.get('/api/adblock/status', (req, res) => {
    const tabId = String(req.query.tabId || 'default');
    const url = String(req.query.url || '');
    res.json(adblock.getStatus(tabId, url));
  });

  app.post('/api/adblock/whitelist', (req, res) => {
    const { domain, disableOnDomain } = req.body as { domain?: string; disableOnDomain?: boolean };
    if (!domain) {
      res.status(400).json({ error: 'Домен не вказано' });
      return;
    }
    const normalized = adblock.normalizeDomain(domain);
    if (disableOnDomain) {
      db.addWhitelist(normalized);
    } else {
      db.removeWhitelist(normalized);
    }
    res.json({ ok: true, domain: normalized, isWhitelisted: !!disableOnDomain });
  });

  app.post('/api/adblock/update', async (_req, res) => {
    const result = await adblock.updateFilterLists();
    res.json(result);
  });

  // --- Bookmarks ---
  app.get('/api/bookmarks', (_req, res) => {
    res.json({
      folders: db.listBookmarkFolders(),
      items: db.listBookmarks(),
    });
  });

  app.post('/api/bookmarks/folders', (req, res) => {
    const { name } = req.body as { name?: string };
    if (!name?.trim()) {
      res.status(400).json({ error: 'Назва папки обов’язкова' });
      return;
    }
    res.json(db.createBookmarkFolder(name.trim()));
  });

  app.post('/api/bookmarks', (req, res) => {
    const body = req.body as Partial<BookmarkItem>;
    if (!body.url?.trim()) {
      res.status(400).json({ error: 'URL обов’язковий' });
      return;
    }
    const item: BookmarkItem = {
      id: body.id || crypto.randomUUID(),
      title: body.title?.trim() || body.url,
      url: body.url.trim(),
      folderId: body.folderId ?? 'quick-bar',
      favicon: body.favicon ?? null,
      onBar: body.onBar ?? true,
      createdAt: body.createdAt || Date.now(),
    };
    db.upsertBookmark(item);
    res.json(item);
  });

  app.delete('/api/bookmarks/:id', (req, res) => {
    db.deleteBookmark(req.params.id);
    res.json({ ok: true });
  });

  app.get('/api/bookmarks/export', (_req, res) => {
    const items = db.listBookmarks();
    const htmlLines = [
      '<!DOCTYPE NETSCAPE-Bookmark-file-1>',
      '<META HTTP-EQUIV="Content-Type" CONTENT="text/html; charset=UTF-8">',
      '<TITLE>Закладки Vortex Browser</TITLE>',
      '<H1>Закладки Vortex Browser</H1>',
      '<DL><p>',
      ...items.map(
        (b) =>
          `  <DT><A HREF="${escapeHtml(b.url)}" ADD_DATE="${Math.floor(b.createdAt / 1000)}">${escapeHtml(b.title)}</A>`
      ),
      '</DL><p>',
    ];
    res.setHeader('Content-Type', 'text/html; charset=utf-8');
    res.setHeader('Content-Disposition', 'attachment; filename="vortex-bookmarks.html"');
    res.send(htmlLines.join('\n'));
  });

  app.post('/api/bookmarks/import', (req, res) => {
    const { content } = req.body as { content?: string };
    if (!content) {
      res.status(400).json({ error: 'Порожній файл закладок' });
      return;
    }
    let imported = 0;
    const regex = /<A\s+[^>]*HREF=["']([^"']+)["'][^>]*>([\s\S]*?)<\/A>/gi;
    let m: RegExpExecArray | null;
    while ((m = regex.exec(content)) !== null) {
      const url = m[1].trim();
      const title = m[2].replace(/<[^>]+>/g, '').trim() || url;
      if (url.startsWith('http://') || url.startsWith('https://')) {
        db.upsertBookmark({
          id: crypto.randomUUID(),
          title,
          url,
          folderId: 'quick-bar',
          favicon: null,
          onBar: true,
          createdAt: Date.now(),
        });
        imported++;
      }
    }
    res.json({ imported });
  });

  // --- History ---
  app.get('/api/history', (req, res) => {
    const query = String(req.query.q || '');
    const limit = Number(req.query.limit || 250);
    res.json(db.listHistory(query, limit));
  });

  app.delete('/api/history/:id', (req, res) => {
    db.deleteHistoryItem(req.params.id);
    res.json({ ok: true });
  });

  app.post('/api/history/clear', (req, res) => {
    const { hoursBack } = req.body as { hoursBack?: number | null };
    const minTs = hoursBack && hoursBack > 0 ? Date.now() - hoursBack * 3600 * 1000 : 0;
    db.clearHistorySince(minTs);
    res.json({ ok: true });
  });

  // --- Password Vault ---
  app.get('/api/vault/status', (_req, res) => {
    res.json(vault.status());
  });

  app.post('/api/vault/setup', (req, res) => {
    try {
      const { masterPassword } = req.body as { masterPassword: string };
      vault.setup(masterPassword || '');
      res.json(vault.status());
    } catch (e: unknown) {
      res.status(400).json({ error: e instanceof Error ? e.message : 'Помилка налаштування' });
    }
  });

  app.post('/api/vault/unlock', (req, res) => {
    try {
      const { masterPassword } = req.body as { masterPassword: string };
      vault.unlock(masterPassword || '');
      res.json(vault.status());
    } catch (e: unknown) {
      res.status(401).json({ error: e instanceof Error ? e.message : 'Невірний майстер-пароль' });
    }
  });

  app.post('/api/vault/lock', (_req, res) => {
    vault.lock();
    res.json(vault.status());
  });

  app.get('/api/vault/entries', (req, res) => {
    try {
      const q = String(req.query.q || '');
      res.json(vault.list(q));
    } catch (e: unknown) {
      res.status(403).json({ error: e instanceof Error ? e.message : 'Сховище заблоковане' });
    }
  });

  app.get('/api/vault/match', (req, res) => {
    const url = String(req.query.url || '');
    res.json(vault.findForUrl(url));
  });

  app.post('/api/vault/entries', (req, res) => {
    try {
      const saved = vault.save(req.body);
      res.json(saved);
    } catch (e: unknown) {
      res.status(400).json({ error: e instanceof Error ? e.message : 'Помилка збереження' });
    }
  });

  app.delete('/api/vault/entries/:id', (req, res) => {
    try {
      vault.delete(req.params.id);
      res.json({ ok: true });
    } catch (e: unknown) {
      res.status(403).json({ error: e instanceof Error ? e.message : 'Сховище заблоковане' });
    }
  });

  app.post('/api/vault/import-csv', (req, res) => {
    try {
      const { csvContent } = req.body as { csvContent: string };
      const imported = vault.importGoogleCsv(csvContent || '');
      res.json({ imported });
    } catch (e: unknown) {
      res.status(400).json({ error: e instanceof Error ? e.message : 'Помилка імпорту CSV' });
    }
  });

  // --- Downloads ---
  app.get('/api/downloads', (_req, res) => {
    const list = db.listDownloads().map((d) => ({
      ...d,
      speedBps: activeDownloads.get(d.id)?.speedBps ?? 0,
    }));
    res.json(list);
  });

  app.post('/api/downloads/start', (req, res) => {
    const { url, filename } = req.body as { url?: string; filename?: string };
    if (!url) {
      res.status(400).json({ error: 'Відсутній URL файлу' });
      return;
    }
    const check = isSafePublicUrl(url);
    if (!check.ok || !check.parsed) {
      res.status(400).json({ error: check.reason || 'Некоректний URL' });
      return;
    }

    const resolvedName =
      filename?.trim() ||
      path.basename(check.parsed.pathname) ||
      `vortex-file-${Date.now()}.bin`;
    const safeName = resolvedName.replace(/[^a-zA-Z0-9._-]/g, '_');
    const savePath = path.join(DOWNLOADS_DIR, `${Date.now()}-${safeName}`);

    const dl: DownloadRecord = {
      id: crypto.randomUUID(),
      filename: safeName,
      url: check.parsed.toString(),
      savePath,
      totalBytes: 0,
      receivedBytes: 0,
      status: 'downloading',
      mimeType: null,
      supportsResume: false,
      startedAt: Date.now(),
      completedAt: null,
    };
    db.upsertDownload(dl);
    void runDownloadWorker(dl, 0);
    res.json({ ...dl, speedBps: 0 });
  });

  app.post('/api/downloads/:id/pause', (req, res) => {
    const active = activeDownloads.get(req.params.id);
    if (active) {
      active.paused = true;
      active.controller.abort();
    }
    res.json({ ok: true });
  });

  app.post('/api/downloads/:id/resume', (req, res) => {
    const dl = db.listDownloads().find((d) => d.id === req.params.id);
    if (!dl) {
      res.status(404).json({ error: 'Завантаження не знайдено' });
      return;
    }
    const offset = fs.existsSync(dl.savePath) ? fs.statSync(dl.savePath).size : 0;
    dl.status = 'downloading';
    dl.receivedBytes = offset;
    db.upsertDownload(dl);
    void runDownloadWorker(dl, offset);
    res.json(dl);
  });

  app.post('/api/downloads/:id/retry', (req, res) => {
    const dl = db.listDownloads().find((d) => d.id === req.params.id);
    if (!dl) {
      res.status(404).json({ error: 'Завантаження не знайдено' });
      return;
    }
    dl.status = 'downloading';
    dl.receivedBytes = 0;
    dl.completedAt = null;
    db.upsertDownload(dl);
    void runDownloadWorker(dl, 0);
    res.json(dl);
  });

  app.delete('/api/downloads/:id', (req, res) => {
    const dl = db.listDownloads().find((d) => d.id === req.params.id);
    if (dl && fs.existsSync(dl.savePath)) {
      try {
        fs.unlinkSync(dl.savePath);
      } catch {
        // ignore
      }
    }
    db.deleteDownload(req.params.id);
    res.json({ ok: true });
  });

  app.get('/api/downloads/:id/file', (req, res) => {
    const dl = db.listDownloads().find((d) => d.id === req.params.id);
    if (!dl || !fs.existsSync(dl.savePath)) {
      res.status(404).json({ error: 'Файл не знайдено на диску' });
      return;
    }
    res.download(dl.savePath, dl.filename);
  });

  // --- Settings & Privacy / Site Data ---
  app.get('/api/settings', (_req, res) => {
    res.json(db.getSettings());
  });

  app.post('/api/settings', (req, res) => {
    const current = db.getSettings();
    const updated = { ...current, ...req.body };
    db.saveSettings(updated);
    res.json(updated);
  });

  app.get('/api/site-data', (_req, res) => {
    res.json(db.listSiteData());
  });

  app.post('/api/site-data/clear', (req, res) => {
    const { domain } = req.body as { domain?: string };
    db.clearSiteData(domain);
    res.json({ ok: true });
  });

  // --- Browser Data Export / Import ---
  app.get('/api/data/export', (_req, res) => {
    res.json({
      exportedAt: new Date().toISOString(),
      settings: db.getSettings(),
      bookmarks: db.listBookmarks(),
      folders: db.listBookmarkFolders(),
      adblockWhitelist: db.listWhitelist(),
    });
  });

  app.post('/api/data/import', (req, res) => {
    const data = req.body as {
      settings?: Record<string, unknown>;
      bookmarks?: BookmarkItem[];
      adblockWhitelist?: string[];
    };
    if (data.settings) {
      db.saveSettings({ ...db.getSettings(), ...data.settings });
    }
    if (Array.isArray(data.bookmarks)) {
      for (const b of data.bookmarks) {
        if (b.url && b.title) {
          db.upsertBookmark({
            id: b.id || crypto.randomUUID(),
            title: b.title,
            url: b.url,
            folderId: b.folderId || 'quick-bar',
            favicon: b.favicon || null,
            onBar: b.onBar ?? true,
            createdAt: b.createdAt || Date.now(),
          });
        }
      }
    }
    if (Array.isArray(data.adblockWhitelist)) {
      for (const domain of data.adblockWhitelist) {
        db.addWhitelist(domain);
      }
    }
    res.json({ ok: true });
  });

  // --- Live Benchmarks (вимірювання реального впливу AdBlock та завантаження сторінок) ---
  app.get('/api/benchmark/runs', (_req, res) => {
    res.json(db.listBenchmarkRuns());
  });

  app.post('/api/benchmark/import', (req, res) => {
    const id = db.saveBenchmarkRun(req.body);
    res.json({ id });
  });

  app.post('/api/benchmark/live', async (req, res) => {
    const { urls } = req.body as { urls?: string[] };
    const targets = (urls && urls.length > 0
      ? urls
      : ['https://example.com', 'https://uk.wikipedia.org', 'https://news.ycombinator.com']
    ).slice(0, 6);

    const results: Array<{
      url: string;
      status: number;
      fetchMs: number;
      rawBytes: number;
      filteredBytes: number;
      blockedRequests: number;
      savedPercent: number;
    }> = [];

    for (const target of targets) {
      const check = isSafePublicUrl(target);
      if (!check.ok || !check.parsed) continue;

      const t0 = performance.now();
      try {
        const resp = await fetch(check.parsed.toString(), {
          headers: { 'User-Agent': 'VortexBrowser/1.0' },
          signal: AbortSignal.timeout(10000),
        });
        const text = await resp.text();
        const fetchMs = Math.round(performance.now() - t0);
        const rawBytes = Buffer.byteLength(text, 'utf8');
        const { filteredHtml, blockedCount } = adblock.filterHtmlResources(
          text,
          check.parsed.toString(),
          'bench'
        );
        const filteredBytes = Buffer.byteLength(filteredHtml, 'utf8');
        const savedPercent = rawBytes > 0 ? Math.round(((rawBytes - filteredBytes) / rawBytes) * 1000) / 10 : 0;

        results.push({
          url: check.parsed.toString(),
          status: resp.status,
          fetchMs,
          rawBytes,
          filteredBytes,
          blockedRequests: blockedCount,
          savedPercent,
        });
      } catch {
        results.push({
          url: target,
          status: 0,
          fetchMs: Math.round(performance.now() - t0),
          rawBytes: 0,
          filteredBytes: 0,
          blockedRequests: 0,
          savedPercent: 0,
        });
      }
    }

    const payload = {
      type: 'live_adblock_network_bench',
      generatedAt: new Date().toISOString(),
      results,
    };
    const id = db.saveBenchmarkRun(payload);
    res.json({ id, ...payload });
  });

  // --- Vite / Static Frontend ---
  if (process.env.NODE_ENV !== 'production') {
    const vite = await createViteServer({
      server: { middlewareMode: true },
      appType: 'spa',
    });
    app.use(vite.middlewares);
  } else {
    const distDir = path.resolve(process.cwd(), 'dist');
    app.use(express.static(distDir));
    app.get('*', (_req, res) => {
      res.sendFile(path.join(distDir, 'index.html'));
    });
  }

  app.listen(PORT, '0.0.0.0', () => {
    console.log(`Vortex Browser server listening on http://0.0.0.0:${PORT}`);
  });
}

startServer().catch((err) => {
  console.error('Failed to start Vortex Browser server:', err);
  process.exit(1);
});
