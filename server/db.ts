import { DatabaseSync } from 'node:sqlite';
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';

export interface BrowserSettings {
  theme: 'dark' | 'light';
  startupPage: string;
  searchEngine: 'google' | 'duckduckgo' | 'brave' | 'bing';
  showBookmarksBar: boolean;
  adblockEnabled: boolean;
  adblockAutoUpdate: boolean;
  memorySaverMode: 'off' | 'standard' | 'aggressive';
  memorySaverSuspendMins: number;
  memorySaverDiscardMins: number;
  downloadDir: string;
  askDownloadLocation: boolean;
  doNotTrack: boolean;
  blockThirdPartyCookies: boolean;
  restoreTabsOnStartup: boolean;
}

export const DEFAULT_SETTINGS: BrowserSettings = {
  theme: 'dark',
  startupPage: 'vortex://newtab',
  searchEngine: 'google',
  showBookmarksBar: true,
  adblockEnabled: true,
  adblockAutoUpdate: true,
  memorySaverMode: 'standard',
  memorySaverSuspendMins: 10,
  memorySaverDiscardMins: 30,
  downloadDir: 'data/downloads',
  askDownloadLocation: false,
  doNotTrack: true,
  blockThirdPartyCookies: true,
  restoreTabsOnStartup: true,
};

export interface BookmarkFolder {
  id: string;
  name: string;
  parentId: string | null;
  sortOrder: number;
}

export interface BookmarkItem {
  id: string;
  title: string;
  url: string;
  folderId: string | null;
  favicon: string | null;
  onBar: boolean;
  createdAt: number;
}

export interface HistoryItem {
  id: string;
  title: string;
  url: string;
  favicon: string | null;
  visitedAt: number;
  visitCount: number;
}

export interface DownloadRecord {
  id: string;
  filename: string;
  url: string;
  savePath: string;
  totalBytes: number;
  receivedBytes: number;
  status: 'downloading' | 'paused' | 'completed' | 'failed' | 'canceled';
  mimeType: string | null;
  supportsResume: boolean;
  startedAt: number;
  completedAt: number | null;
}

export interface SessionTabRecord {
  id: string;
  url: string;
  title: string;
  favicon: string | null;
  isPinned: boolean;
  isActive: boolean;
  sortOrder: number;
}

export interface EncryptedVaultRow {
  id: string;
  origin: string;
  title: string;
  usernameEnc: string;
  passwordEnc: string;
  notesEnc: string | null;
  createdAt: number;
  updatedAt: number;
}

export class VortexDatabase {
  public db: DatabaseSync;

  constructor(dbPath: string) {
    fs.mkdirSync(path.dirname(dbPath), { recursive: true });
    this.db = new DatabaseSync(dbPath);
    this.db.exec(`
      PRAGMA journal_mode = WAL;
      PRAGMA synchronous = NORMAL;
      PRAGMA foreign_keys = ON;
      PRAGMA temp_store = MEMORY;
      PRAGMA cache_size = -16000;
    `);
    this.initSchema();
  }

  private initSchema(): void {
    this.db.exec(`
      CREATE TABLE IF NOT EXISTS bookmark_folders (
        id TEXT PRIMARY KEY,
        name TEXT NOT NULL,
        parent_id TEXT,
        sort_order INTEGER NOT NULL DEFAULT 0
      );

      CREATE TABLE IF NOT EXISTS bookmarks (
        id TEXT PRIMARY KEY,
        title TEXT NOT NULL,
        url TEXT NOT NULL,
        folder_id TEXT,
        favicon TEXT,
        on_bar INTEGER NOT NULL DEFAULT 1,
        created_at INTEGER NOT NULL,
        FOREIGN KEY(folder_id) REFERENCES bookmark_folders(id) ON DELETE SET NULL
      );

      CREATE TABLE IF NOT EXISTS history (
        id TEXT PRIMARY KEY,
        title TEXT NOT NULL,
        url TEXT NOT NULL,
        favicon TEXT,
        visited_at INTEGER NOT NULL,
        visit_count INTEGER NOT NULL DEFAULT 1
      );
      CREATE INDEX IF NOT EXISTS idx_history_visited ON history(visited_at DESC);
      CREATE INDEX IF NOT EXISTS idx_history_url ON history(url);

      CREATE TABLE IF NOT EXISTS downloads (
        id TEXT PRIMARY KEY,
        filename TEXT NOT NULL,
        url TEXT NOT NULL,
        save_path TEXT NOT NULL,
        total_bytes INTEGER NOT NULL DEFAULT 0,
        received_bytes INTEGER NOT NULL DEFAULT 0,
        status TEXT NOT NULL,
        mime_type TEXT,
        supports_resume INTEGER NOT NULL DEFAULT 0,
        started_at INTEGER NOT NULL,
        completed_at INTEGER
      );

      CREATE TABLE IF NOT EXISTS tabs_session (
        id TEXT PRIMARY KEY,
        url TEXT NOT NULL,
        title TEXT NOT NULL,
        favicon TEXT,
        is_pinned INTEGER NOT NULL DEFAULT 0,
        is_active INTEGER NOT NULL DEFAULT 0,
        sort_order INTEGER NOT NULL DEFAULT 0
      );

      CREATE TABLE IF NOT EXISTS adblock_whitelist (
        domain TEXT PRIMARY KEY,
        added_at INTEGER NOT NULL
      );

      CREATE TABLE IF NOT EXISTS adblock_stats (
        domain TEXT PRIMARY KEY,
        blocked_count INTEGER NOT NULL DEFAULT 0,
        last_blocked_at INTEGER NOT NULL
      );

      CREATE TABLE IF NOT EXISTS vault_meta (
        id INTEGER PRIMARY KEY CHECK (id = 1),
        salt_b64 TEXT NOT NULL,
        verifier_enc TEXT NOT NULL,
        kdf_params TEXT NOT NULL,
        created_at INTEGER NOT NULL
      );

      CREATE TABLE IF NOT EXISTS password_entries (
        id TEXT PRIMARY KEY,
        origin TEXT NOT NULL,
        title TEXT NOT NULL,
        username_enc TEXT NOT NULL,
        password_enc TEXT NOT NULL,
        notes_enc TEXT,
        created_at INTEGER NOT NULL,
        updated_at INTEGER NOT NULL
      );
      CREATE INDEX IF NOT EXISTS idx_passwords_origin ON password_entries(origin);

      CREATE TABLE IF NOT EXISTS settings (
        key TEXT PRIMARY KEY,
        value_json TEXT NOT NULL
      );

      CREATE TABLE IF NOT EXISTS site_cookies (
        id TEXT PRIMARY KEY,
        domain TEXT NOT NULL,
        name TEXT NOT NULL,
        size_bytes INTEGER NOT NULL DEFAULT 0,
        updated_at INTEGER NOT NULL
      );

      CREATE TABLE IF NOT EXISTS benchmark_runs (
        id TEXT PRIMARY KEY,
        created_at INTEGER NOT NULL,
        payload_json TEXT NOT NULL
      );
    `);

    const folderCount = this.db.prepare('SELECT COUNT(*) as cnt FROM bookmark_folders').get() as { cnt: number };
    if (folderCount.cnt === 0) {
      this.db.prepare('INSERT INTO bookmark_folders (id, name, parent_id, sort_order) VALUES (?, ?, NULL, ?)').run('quick-bar', 'Панель закладок', 0);
      this.db.prepare('INSERT INTO bookmark_folders (id, name, parent_id, sort_order) VALUES (?, ?, NULL, ?)').run('dev', 'Розробка та документація', 1);

      const now = Date.now();
      const seedBookmarks = [
        { id: crypto.randomUUID(), title: 'Вікіпедія', url: 'https://uk.wikipedia.org', folderId: 'quick-bar', onBar: 1 },
        { id: crypto.randomUUID(), title: 'Rust Lang', url: 'https://www.rust-lang.org', folderId: 'dev', onBar: 1 },
        { id: crypto.randomUUID(), title: 'MDN Web Docs', url: 'https://developer.mozilla.org', folderId: 'dev', onBar: 1 },
        { id: crypto.randomUUID(), title: 'Hacker News', url: 'https://news.ycombinator.com', folderId: 'quick-bar', onBar: 1 },
        { id: crypto.randomUUID(), title: 'Example Domain', url: 'https://example.com', folderId: 'quick-bar', onBar: 1 },
      ];
      const stmt = this.db.prepare(
        'INSERT INTO bookmarks (id, title, url, folder_id, favicon, on_bar, created_at) VALUES (?, ?, ?, ?, NULL, ?, ?)'
      );
      for (const b of seedBookmarks) {
        stmt.run(b.id, b.title, b.url, b.folderId, b.onBar, now);
      }
    }
  }

  getSettings(): BrowserSettings {
    const row = this.db.prepare("SELECT value_json FROM settings WHERE key = 'browser_settings'").get() as { value_json: string } | undefined;
    if (row) {
      try {
        return { ...DEFAULT_SETTINGS, ...JSON.parse(row.value_json) };
      } catch {
        return DEFAULT_SETTINGS;
      }
    }
    return DEFAULT_SETTINGS;
  }

  saveSettings(settings: BrowserSettings): void {
    this.db.prepare(
      `INSERT INTO settings (key, value_json) VALUES ('browser_settings', ?)
       ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json`
    ).run(JSON.stringify(settings));
  }

  // --- AdBlock ---
  listWhitelist(): string[] {
    const rows = this.db.prepare('SELECT domain FROM adblock_whitelist ORDER BY added_at DESC').all() as { domain: string }[];
    return rows.map((r) => r.domain);
  }

  addWhitelist(domain: string): void {
    this.db.prepare('INSERT OR REPLACE INTO adblock_whitelist (domain, added_at) VALUES (?, ?)').run(domain.toLowerCase(), Date.now());
  }

  removeWhitelist(domain: string): void {
    this.db.prepare('DELETE FROM adblock_whitelist WHERE domain = ?').run(domain.toLowerCase());
  }

  incrementAdblockStat(domain: string, delta: number): void {
    this.db.prepare(
      `INSERT INTO adblock_stats (domain, blocked_count, last_blocked_at) VALUES (?, ?, ?)
       ON CONFLICT(domain) DO UPDATE SET blocked_count = blocked_count + excluded.blocked_count, last_blocked_at = excluded.last_blocked_at`
    ).run(domain.toLowerCase(), delta, Date.now());
  }

  getTotalBlocked(): number {
    const row = this.db.prepare('SELECT SUM(blocked_count) as total FROM adblock_stats').get() as { total: number | null };
    return row?.total ?? 0;
  }

  // --- Bookmarks ---
  listBookmarkFolders(): BookmarkFolder[] {
    const rows = this.db.prepare('SELECT id, name, parent_id, sort_order FROM bookmark_folders ORDER BY sort_order ASC').all() as Array<{
      id: string;
      name: string;
      parent_id: string | null;
      sort_order: number;
    }>;
    return rows.map((r) => ({ id: r.id, name: r.name, parentId: r.parent_id, sortOrder: r.sort_order }));
  }

  createBookmarkFolder(name: string): BookmarkFolder {
    const id = crypto.randomUUID();
    const maxRow = this.db.prepare('SELECT COALESCE(MAX(sort_order), 0) as m FROM bookmark_folders').get() as { m: number };
    const sortOrder = maxRow.m + 1;
    this.db.prepare('INSERT INTO bookmark_folders (id, name, parent_id, sort_order) VALUES (?, ?, NULL, ?)').run(id, name, sortOrder);
    return { id, name, parentId: null, sortOrder };
  }

  listBookmarks(): BookmarkItem[] {
    const rows = this.db.prepare('SELECT id, title, url, folder_id, favicon, on_bar, created_at FROM bookmarks ORDER BY created_at DESC').all() as Array<{
      id: string;
      title: string;
      url: string;
      folder_id: string | null;
      favicon: string | null;
      on_bar: number;
      created_at: number;
    }>;
    return rows.map((r) => ({
      id: r.id,
      title: r.title,
      url: r.url,
      folderId: r.folder_id,
      favicon: r.favicon,
      onBar: r.on_bar !== 0,
      createdAt: r.created_at,
    }));
  }

  upsertBookmark(item: BookmarkItem): void {
    this.db.prepare(
      `INSERT INTO bookmarks (id, title, url, folder_id, favicon, on_bar, created_at)
       VALUES (?, ?, ?, ?, ?, ?, ?)
       ON CONFLICT(id) DO UPDATE SET title = excluded.title, url = excluded.url, folder_id = excluded.folder_id, favicon = excluded.favicon, on_bar = excluded.on_bar`
    ).run(item.id, item.title, item.url, item.folderId, item.favicon, item.onBar ? 1 : 0, item.createdAt);
  }

  deleteBookmark(id: string): void {
    this.db.prepare('DELETE FROM bookmarks WHERE id = ?').run(id);
  }

  // --- History ---
  recordHistory(title: string, url: string, favicon: string | null = null): void {
    if (!url || url.startsWith('vortex://') || url.startsWith('about:')) return;
    const now = Date.now();
    const existing = this.db.prepare('SELECT id FROM history WHERE url = ? ORDER BY visited_at DESC LIMIT 1').get(url) as { id: string } | undefined;
    if (existing) {
      this.db.prepare('UPDATE history SET title = ?, favicon = COALESCE(?, favicon), visited_at = ?, visit_count = visit_count + 1 WHERE id = ?').run(
        title || url,
        favicon,
        now,
        existing.id
      );
    } else {
      this.db.prepare('INSERT INTO history (id, title, url, favicon, visited_at, visit_count) VALUES (?, ?, ?, ?, ?, 1)').run(
        crypto.randomUUID(),
        title || url,
        url,
        favicon,
        now
      );
    }
  }

  listHistory(query = '', limit = 200): HistoryItem[] {
    const q = query.trim().toLowerCase();
    const pattern = `%${q}%`;
    const rows = this.db.prepare(
      `SELECT id, title, url, favicon, visited_at, visit_count FROM history
       WHERE (? = '' OR LOWER(title) LIKE ? OR LOWER(url) LIKE ?)
       ORDER BY visited_at DESC LIMIT ?`
    ).all(q, pattern, pattern, limit) as Array<{
      id: string;
      title: string;
      url: string;
      favicon: string | null;
      visited_at: number;
      visit_count: number;
    }>;
    return rows.map((r) => ({
      id: r.id,
      title: r.title,
      url: r.url,
      favicon: r.favicon,
      visitedAt: r.visited_at,
      visitCount: r.visit_count,
    }));
  }

  deleteHistoryItem(id: string): void {
    this.db.prepare('DELETE FROM history WHERE id = ?').run(id);
  }

  clearHistorySince(minTs: number): void {
    this.db.prepare('DELETE FROM history WHERE visited_at >= ?').run(minTs);
  }

  // --- Downloads ---
  upsertDownload(dl: DownloadRecord): void {
    this.db.prepare(
      `INSERT INTO downloads (id, filename, url, save_path, total_bytes, received_bytes, status, mime_type, supports_resume, started_at, completed_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
       ON CONFLICT(id) DO UPDATE SET total_bytes = excluded.total_bytes, received_bytes = excluded.received_bytes, status = excluded.status, supports_resume = excluded.supports_resume, completed_at = excluded.completed_at`
    ).run(
      dl.id,
      dl.filename,
      dl.url,
      dl.savePath,
      dl.totalBytes,
      dl.receivedBytes,
      dl.status,
      dl.mimeType,
      dl.supportsResume ? 1 : 0,
      dl.startedAt,
      dl.completedAt
    );
  }

  listDownloads(): DownloadRecord[] {
    const rows = this.db.prepare(
      'SELECT id, filename, url, save_path, total_bytes, received_bytes, status, mime_type, supports_resume, started_at, completed_at FROM downloads ORDER BY started_at DESC'
    ).all() as Array<{
      id: string;
      filename: string;
      url: string;
      save_path: string;
      total_bytes: number;
      received_bytes: number;
      status: DownloadRecord['status'];
      mime_type: string | null;
      supports_resume: number;
      started_at: number;
      completed_at: number | null;
    }>;
    return rows.map((r) => ({
      id: r.id,
      filename: r.filename,
      url: r.url,
      savePath: r.save_path,
      totalBytes: r.total_bytes,
      receivedBytes: r.received_bytes,
      status: r.status,
      mimeType: r.mime_type,
      supportsResume: r.supports_resume !== 0,
      startedAt: r.started_at,
      completedAt: r.completed_at,
    }));
  }

  deleteDownload(id: string): void {
    this.db.prepare('DELETE FROM downloads WHERE id = ?').run(id);
  }

  // --- Tabs Session ---
  saveSessionTabs(tabs: SessionTabRecord[]): void {
    this.db.exec('BEGIN');
    try {
      this.db.exec('DELETE FROM tabs_session');
      const stmt = this.db.prepare(
        'INSERT INTO tabs_session (id, url, title, favicon, is_pinned, is_active, sort_order) VALUES (?, ?, ?, ?, ?, ?, ?)'
      );
      for (const t of tabs) {
        stmt.run(t.id, t.url, t.title, t.favicon, t.isPinned ? 1 : 0, t.isActive ? 1 : 0, t.sortOrder);
      }
      this.db.exec('COMMIT');
    } catch (e) {
      this.db.exec('ROLLBACK');
      throw e;
    }
  }

  loadSessionTabs(): SessionTabRecord[] {
    const rows = this.db.prepare('SELECT id, url, title, favicon, is_pinned, is_active, sort_order FROM tabs_session ORDER BY sort_order ASC').all() as Array<{
      id: string;
      url: string;
      title: string;
      favicon: string | null;
      is_pinned: number;
      is_active: number;
      sort_order: number;
    }>;
    return rows.map((r) => ({
      id: r.id,
      url: r.url,
      title: r.title,
      favicon: r.favicon,
      isPinned: r.is_pinned !== 0,
      isActive: r.is_active !== 0,
      sortOrder: r.sort_order,
    }));
  }

  // --- Password Vault ---
  getVaultMeta(): { saltB64: string; verifierEnc: string; kdfParams: string } | null {
    const row = this.db.prepare('SELECT salt_b64, verifier_enc, kdf_params FROM vault_meta WHERE id = 1').get() as
      | { salt_b64: string; verifier_enc: string; kdf_params: string }
      | undefined;
    if (!row) return null;
    return { saltB64: row.salt_b64, verifierEnc: row.verifier_enc, kdfParams: row.kdf_params };
  }

  saveVaultMeta(saltB64: string, verifierEnc: string, kdfParams: string): void {
    this.db.prepare(
      'INSERT OR REPLACE INTO vault_meta (id, salt_b64, verifier_enc, kdf_params, created_at) VALUES (1, ?, ?, ?, ?)'
    ).run(saltB64, verifierEnc, kdfParams, Date.now());
  }

  upsertPasswordRow(row: EncryptedVaultRow): void {
    this.db.prepare(
      `INSERT INTO password_entries (id, origin, title, username_enc, password_enc, notes_enc, created_at, updated_at)
       VALUES (?, ?, ?, ?, ?, ?, ?, ?)
       ON CONFLICT(id) DO UPDATE SET origin = excluded.origin, title = excluded.title, username_enc = excluded.username_enc, password_enc = excluded.password_enc, notes_enc = excluded.notes_enc, updated_at = excluded.updated_at`
    ).run(row.id, row.origin, row.title, row.usernameEnc, row.passwordEnc, row.notesEnc, row.createdAt, row.updatedAt);
  }

  listPasswordRows(): EncryptedVaultRow[] {
    const rows = this.db.prepare(
      'SELECT id, origin, title, username_enc, password_enc, notes_enc, created_at, updated_at FROM password_entries ORDER BY updated_at DESC'
    ).all() as Array<{
      id: string;
      origin: string;
      title: string;
      username_enc: string;
      password_enc: string;
      notes_enc: string | null;
      created_at: number;
      updated_at: number;
    }>;
    return rows.map((r) => ({
      id: r.id,
      origin: r.origin,
      title: r.title,
      usernameEnc: r.username_enc,
      passwordEnc: r.password_enc,
      notesEnc: r.notes_enc,
      createdAt: r.created_at,
      updatedAt: r.updated_at,
    }));
  }

  deletePasswordRow(id: string): void {
    this.db.prepare('DELETE FROM password_entries WHERE id = ?').run(id);
  }

  // --- Site Data & Cookies ---
  recordSiteData(domain: string, name: string, sizeBytes: number): void {
    if (!domain) return;
    this.db.prepare(
      'INSERT OR REPLACE INTO site_cookies (id, domain, name, size_bytes, updated_at) VALUES (?, ?, ?, ?, ?)'
    ).run(`${domain}:${name}`, domain, name, sizeBytes, Date.now());
  }

  listSiteData(): Array<{ domain: string; cookieCount: number; totalBytes: number; updatedAt: number }> {
    const rows = this.db.prepare(
      'SELECT domain, COUNT(*) as cnt, SUM(size_bytes) as bytes, MAX(updated_at) as ts FROM site_cookies GROUP BY domain ORDER BY ts DESC'
    ).all() as Array<{ domain: string; cnt: number; bytes: number; ts: number }>;
    return rows.map((r) => ({
      domain: r.domain,
      cookieCount: r.cnt,
      totalBytes: r.bytes || 0,
      updatedAt: r.ts,
    }));
  }

  clearSiteData(domain?: string): void {
    if (domain) {
      this.db.prepare('DELETE FROM site_cookies WHERE domain = ?').run(domain);
    } else {
      this.db.exec('DELETE FROM site_cookies');
    }
  }

  // --- Benchmark Runs ---
  saveBenchmarkRun(payload: unknown): string {
    const id = crypto.randomUUID();
    this.db.prepare('INSERT INTO benchmark_runs (id, created_at, payload_json) VALUES (?, ?, ?)').run(
      id,
      Date.now(),
      JSON.stringify(payload)
    );
    return id;
  }

  listBenchmarkRuns(): Array<{ id: string; createdAt: number; payload: unknown }> {
    const rows = this.db.prepare('SELECT id, created_at, payload_json FROM benchmark_runs ORDER BY created_at DESC LIMIT 20').all() as Array<{
      id: string;
      created_at: number;
      payload_json: string;
    }>;
    return rows.map((r) => ({ id: r.id, createdAt: r.created_at, payload: JSON.parse(r.payload_json) }));
  }
}
