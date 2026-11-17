import { invoke } from '@tauri-apps/api/core';
import type {
  AdBlockStatus,
  BenchmarkRunRecord,
  BookmarkFolder,
  BookmarkItem,
  BrowseResponse,
  BrowserSettings,
  BrowserTab,
  DownloadRecord,
  HistoryItem,
  LiveBenchItem,
  PasswordCredential,
  SiteDataSummary,
  VaultStatus,
} from '../types/browser';

export function isTauriRuntime(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

async function apiJson<T>(url: string, init?: RequestInit): Promise<T> {
  const resp = await fetch(url, {
    ...init,
    headers: {
      'Content-Type': 'application/json',
      ...(init?.headers || {}),
    },
  });
  const data = await resp.json().catch(() => ({}));
  if (!resp.ok) {
    throw new Error((data as { error?: string }).error || `HTTP ${resp.status}`);
  }
  return data as T;
}

export const vortexApi = {
  // --- Native WebView2 Control (Tauri 2) ---
  async syncNativeViewport(rect: { x: number; y: number; width: number; height: number }): Promise<void> {
    if (!isTauriRuntime()) return;
    await invoke('set_viewport_bounds', { rect });
  },

  async setNativeOverlayHidden(hidden: boolean): Promise<void> {
    if (!isTauriRuntime()) return;
    await invoke('set_overlay_hidden', { hidden });
  },

  async nativeGetTabs(): Promise<Array<Omit<BrowserTab, 'historyStack' | 'historyIndex'>>> {
    if (!isTauriRuntime()) return [];
    return invoke('get_tabs');
  },

  async nativeGetActiveTabId(): Promise<string | null> {
    if (!isTauriRuntime()) return null;
    return invoke('get_active_tab_id');
  },

  async nativeCreateTab(
    url: string,
    isIncognito: boolean,
    activate: boolean,
    tabId?: string
  ): Promise<Omit<BrowserTab, 'historyStack' | 'historyIndex'> | null> {
    if (!isTauriRuntime()) return null;
    return invoke('create_tab', { url, isIncognito, activate, tabId: tabId ?? null });
  },

  async nativeSwitchTab(tabId: string): Promise<void> {
    if (!isTauriRuntime()) return;
    await invoke('switch_tab', { tabId });
  },

  async nativeCloseTab(tabId: string): Promise<void> {
    if (!isTauriRuntime()) return;
    await invoke('close_tab', { tabId });
  },

  async nativeNavigateTab(tabId: string, url: string): Promise<void> {
    if (!isTauriRuntime()) return;
    await invoke('navigate_tab', { tabId, url });
  },

  async nativeNavAction(tabId: string, action: 'back' | 'forward' | 'reload' | 'stop'): Promise<void> {
    if (!isTauriRuntime()) return;
    await invoke('tab_nav_action', { tabId, action });
  },

  // --- Browse Proxy (Web Mode) ---
  async browsePage(url: string, tabId: string, incognito: boolean): Promise<BrowseResponse> {
    return apiJson<BrowseResponse>('/api/browse', {
      method: 'POST',
      body: JSON.stringify({ url, tabId, incognito }),
    });
  },

  // --- Session Tabs ---
  async loadSessionTabs(): Promise<
    Array<{
      id: string;
      url: string;
      title: string;
      favicon: string | null;
      isPinned: boolean;
      isActive: boolean;
      sortOrder: number;
    }>
  > {
    return apiJson('/api/session/tabs');
  },

  async saveSessionTabs(
    tabs: Array<{
      id: string;
      url: string;
      title: string;
      favicon: string | null;
      isPinned: boolean;
      isActive: boolean;
      sortOrder: number;
    }>
  ): Promise<void> {
    await apiJson('/api/session/tabs', {
      method: 'POST',
      body: JSON.stringify({ tabs }),
    });
  },

  // --- AdBlock ---
  async getAdBlockStatus(tabId: string, url: string): Promise<AdBlockStatus> {
    if (isTauriRuntime()) {
      return invoke('get_adblock_status', { tabId, pageUrl: url });
    }
    return apiJson(`/api/adblock/status?tabId=${encodeURIComponent(tabId)}&url=${encodeURIComponent(url)}`);
  },

  async toggleAdBlockDomain(domain: string, disableOnDomain: boolean): Promise<void> {
    if (isTauriRuntime()) {
      await invoke('toggle_adblock_domain', { domain, disableOnDomain });
      return;
    }
    await apiJson('/api/adblock/whitelist', {
      method: 'POST',
      body: JSON.stringify({ domain, disableOnDomain }),
    });
  },

  async updateAdBlockFilters(): Promise<{ rulesLoaded: number; lastUpdatedAt?: number }> {
    if (isTauriRuntime()) {
      const rulesLoaded = await invoke<number>('update_adblock_filters');
      return { rulesLoaded, lastUpdatedAt: Date.now() };
    }
    return apiJson('/api/adblock/update', { method: 'POST' });
  },

  // --- Settings ---
  async getSettings(): Promise<BrowserSettings> {
    if (isTauriRuntime()) {
      return invoke('get_settings');
    }
    return apiJson('/api/settings');
  },

  async updateSettings(settings: Partial<BrowserSettings>): Promise<BrowserSettings> {
    if (isTauriRuntime()) {
      const current = await invoke<BrowserSettings>('get_settings');
      const next = { ...current, ...settings };
      await invoke('update_settings', { settings: next });
      return next;
    }
    return apiJson('/api/settings', {
      method: 'POST',
      body: JSON.stringify(settings),
    });
  },

  // --- Bookmarks ---
  async listBookmarks(): Promise<{ folders: BookmarkFolder[]; items: BookmarkItem[] }> {
    if (isTauriRuntime()) {
      const [folders, items] = await invoke<[BookmarkFolder[], BookmarkItem[]]>('list_bookmarks');
      return { folders, items };
    }
    return apiJson('/api/bookmarks');
  },

  async createBookmarkFolder(name: string): Promise<BookmarkFolder> {
    return apiJson('/api/bookmarks/folders', {
      method: 'POST',
      body: JSON.stringify({ name }),
    });
  },

  async saveBookmark(item: Partial<BookmarkItem>): Promise<BookmarkItem> {
    if (isTauriRuntime()) {
      const full: BookmarkItem = {
        id: item.id || crypto.randomUUID(),
        title: item.title || item.url || '',
        url: item.url || '',
        folderId: item.folderId ?? 'quick-bar',
        favicon: item.favicon ?? null,
        onBar: item.onBar ?? true,
        createdAt: item.createdAt || Date.now(),
      };
      await invoke('save_bookmark', { bookmark: full });
      return full;
    }
    return apiJson('/api/bookmarks', {
      method: 'POST',
      body: JSON.stringify(item),
    });
  },

  async deleteBookmark(id: string): Promise<void> {
    if (isTauriRuntime()) {
      await invoke('delete_bookmark', { id });
      return;
    }
    await apiJson(`/api/bookmarks/${encodeURIComponent(id)}`, { method: 'DELETE' });
  },

  async importBookmarksHtml(content: string): Promise<{ imported: number }> {
    return apiJson('/api/bookmarks/import', {
      method: 'POST',
      body: JSON.stringify({ content }),
    });
  },

  // --- History ---
  async listHistory(query = ''): Promise<HistoryItem[]> {
    if (isTauriRuntime()) {
      return invoke('list_history', { query, limit: 250 });
    }
    return apiJson(`/api/history?q=${encodeURIComponent(query)}`);
  },

  async deleteHistoryItem(id: string): Promise<void> {
    if (isTauriRuntime()) {
      await invoke('delete_history_item', { id });
      return;
    }
    await apiJson(`/api/history/${encodeURIComponent(id)}`, { method: 'DELETE' });
  },

  async clearHistory(hoursBack: number | null): Promise<void> {
    if (isTauriRuntime()) {
      await invoke('clear_history_since', { hoursBack });
      return;
    }
    await apiJson('/api/history/clear', {
      method: 'POST',
      body: JSON.stringify({ hoursBack }),
    });
  },

  // --- Downloads ---
  async listDownloads(): Promise<DownloadRecord[]> {
    if (isTauriRuntime()) {
      return invoke('list_downloads');
    }
    return apiJson('/api/downloads');
  },

  async startDownload(url: string, filename?: string): Promise<DownloadRecord> {
    if (isTauriRuntime()) {
      return invoke('start_download', { url, filename, savePath: null });
    }
    return apiJson('/api/downloads/start', {
      method: 'POST',
      body: JSON.stringify({ url, filename }),
    });
  },

  async pauseDownload(id: string): Promise<void> {
    if (isTauriRuntime()) {
      await invoke('pause_download', { id });
      return;
    }
    await apiJson(`/api/downloads/${encodeURIComponent(id)}/pause`, { method: 'POST' });
  },

  async resumeDownload(id: string): Promise<void> {
    if (isTauriRuntime()) {
      await invoke('resume_download', { id });
      return;
    }
    await apiJson(`/api/downloads/${encodeURIComponent(id)}/resume`, { method: 'POST' });
  },

  async retryDownload(id: string): Promise<void> {
    await apiJson(`/api/downloads/${encodeURIComponent(id)}/retry`, { method: 'POST' });
  },

  async deleteDownload(id: string): Promise<void> {
    await apiJson(`/api/downloads/${encodeURIComponent(id)}`, { method: 'DELETE' });
  },

  async openDownloadFolder(savePath: string): Promise<void> {
    if (isTauriRuntime()) {
      await invoke('open_download_folder', { savePath });
    }
  },

  // --- Password Vault ---
  async vaultStatus(): Promise<VaultStatus> {
    if (isTauriRuntime()) {
      return invoke('vault_status');
    }
    return apiJson('/api/vault/status');
  },

  async vaultSetup(masterPassword: string): Promise<VaultStatus> {
    if (isTauriRuntime()) {
      await invoke('vault_setup', { masterPassword });
      return invoke('vault_status');
    }
    return apiJson('/api/vault/setup', {
      method: 'POST',
      body: JSON.stringify({ masterPassword }),
    });
  },

  async vaultUnlock(masterPassword: string): Promise<VaultStatus> {
    if (isTauriRuntime()) {
      await invoke('vault_unlock', { masterPassword });
      return invoke('vault_status');
    }
    return apiJson('/api/vault/unlock', {
      method: 'POST',
      body: JSON.stringify({ masterPassword }),
    });
  },

  async vaultLock(): Promise<VaultStatus> {
    if (isTauriRuntime()) {
      await invoke('vault_lock');
      return invoke('vault_status');
    }
    return apiJson('/api/vault/lock', { method: 'POST' });
  },

  async vaultList(search = ''): Promise<PasswordCredential[]> {
    if (isTauriRuntime()) {
      return invoke('vault_list', { search });
    }
    return apiJson(`/api/vault/entries?q=${encodeURIComponent(search)}`);
  },

  async vaultMatchUrl(url: string): Promise<PasswordCredential[]> {
    if (isTauriRuntime()) {
      try {
        const st = await invoke<VaultStatus>('vault_status');
        if (!st.isUnlocked) return [];
        const host = new URL(url).hostname.replace(/^www\./, '').toLowerCase();
        const all = await invoke<PasswordCredential[]>('vault_list', { search: host });
        return all.filter((item) => item.origin.toLowerCase().includes(host));
      } catch {
        return [];
      }
    }
    return apiJson(`/api/vault/match?url=${encodeURIComponent(url)}`);
  },

  async vaultSave(entry: {
    id?: string;
    origin: string;
    title: string;
    username: string;
    password: string;
    notes?: string | null;
  }): Promise<PasswordCredential> {
    if (isTauriRuntime()) {
      return invoke('vault_save', entry);
    }
    return apiJson('/api/vault/entries', {
      method: 'POST',
      body: JSON.stringify(entry),
    });
  },

  async vaultDelete(id: string): Promise<void> {
    if (isTauriRuntime()) {
      await invoke('vault_delete', { id });
      return;
    }
    await apiJson(`/api/vault/entries/${encodeURIComponent(id)}`, { method: 'DELETE' });
  },

  async vaultImportCsv(csvContent: string): Promise<{ imported: number }> {
    if (isTauriRuntime()) {
      const imported = await invoke<number>('vault_import_csv', { csvContent });
      return { imported };
    }
    return apiJson('/api/vault/import-csv', {
      method: 'POST',
      body: JSON.stringify({ csvContent }),
    });
  },

  // --- Site Data, Data Export/Import & Benchmarks ---
  async listSiteData(): Promise<SiteDataSummary[]> {
    try {
      return await apiJson('/api/site-data');
    } catch (err) {
      if (isTauriRuntime()) return [];
      throw err;
    }
  },

  async clearSiteData(domain?: string): Promise<void> {
    await apiJson('/api/site-data/clear', {
      method: 'POST',
      body: JSON.stringify({ domain }),
    });
  },

  async exportAllData(): Promise<Record<string, unknown>> {
    return apiJson('/api/data/export');
  },

  async importAllData(payload: Record<string, unknown>): Promise<void> {
    await apiJson('/api/data/import', {
      method: 'POST',
      body: JSON.stringify(payload),
    });
  },

  async listBenchmarks(): Promise<BenchmarkRunRecord[]> {
    try {
      return await apiJson('/api/benchmark/runs');
    } catch (err) {
      if (isTauriRuntime()) return [];
      throw err;
    }
  },

  async runLiveBenchmark(urls: string[]): Promise<{
    id: string;
    generatedAt: string;
    results: LiveBenchItem[];
  }> {
    return apiJson('/api/benchmark/live', {
      method: 'POST',
      body: JSON.stringify({ urls }),
    });
  },

  async importBenchmarkJson(payload: Record<string, unknown>): Promise<{ id: string }> {
    return apiJson('/api/benchmark/import', {
      method: 'POST',
      body: JSON.stringify(payload),
    });
  },
};
