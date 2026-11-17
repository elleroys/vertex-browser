import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { isTauriRuntime, vortexApi } from './bridge/vortexApi';
import { BookmarksBar } from './components/BookmarksBar';
import { OmniboxBar } from './components/OmniboxBar';
import { SidePanel } from './components/SidePanel';
import { TabStrip } from './components/TabStrip';
import { WebViewport } from './components/WebViewport';
import type {
  AdBlockStatus,
  BenchmarkRunRecord,
  BookmarkFolder,
  BookmarkItem,
  BrowserSettings,
  BrowserTab,
  DownloadRecord,
  HistoryItem,
  PasswordCredential,
  SearchEngineId,
  SidePanelSection,
  SiteDataSummary,
  VaultStatus,
} from './types/browser';

const DEFAULT_SETTINGS: BrowserSettings = {
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

function buildSearchUrl(query: string, engine: SearchEngineId): string {
  const encoded = encodeURIComponent(query);
  switch (engine) {
    case 'duckduckgo':
      return `https://duckduckgo.com/?q=${encoded}`;
    case 'brave':
      return `https://search.brave.com/search?q=${encoded}`;
    case 'bing':
      return `https://www.bing.com/search?q=${encoded}`;
    case 'google':
    default:
      return `https://www.google.com/search?q=${encoded}`;
  }
}

function normalizeUrlInput(raw: string, engine: SearchEngineId = 'google'): string {
  const trimmed = raw.trim();
  if (!trimmed) return 'vortex://newtab';

  const hasWhitespace = /\s/.test(trimmed);
  const hasProtocol = /^(vortex|https?):\/\//i.test(trimmed);

  // Будь-який рядок із пробілами завжди є пошуковим запитом
  if (hasWhitespace) {
    return buildSearchUrl(trimmed, engine);
  }

  // 1. Явний протокол (vortex://, http://, https://)
  if (hasProtocol) {
    if (/^vortex:\/\//i.test(trimmed)) {
      return trimmed.replace(/^vortex:\/\//i, 'vortex://');
    }
    const normalizedScheme = trimmed.replace(/^https?:\/\//i, (m) => m.toLowerCase());
    try {
      new URL(normalizedScheme);
      return normalizedScheme;
    } catch {
      return buildSearchUrl(trimmed, engine);
    }
  }

  // 2. Явні доменні шаблони без протоколу: localhost або домен із крапками (наприклад example.com, site.org) чи IPv4
  const isLocalhost = /^localhost(?::\d{1,5})?(?:[/?#].*)?$/i.test(trimmed);
  if (isLocalhost) {
    const candidate = `http://${trimmed.replace(/^localhost/i, 'localhost')}`;
    try {
      new URL(candidate);
      return candidate;
    } catch {
      return buildSearchUrl(trimmed, engine);
    }
  }

  const hasDot = trimmed.includes('.');
  const isDomainPattern =
    hasDot &&
    /^(?:[a-zA-Z0-9-]+\.)+(?:[a-zA-Z]{2,}|xn--[a-zA-Z0-9-]+)(?::\d{1,5})?(?:[/?#].*)?$/.test(
      trimmed
    );
  const isIpv4Pattern =
    hasDot &&
    /^(?:25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)(?:\.(?:25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)){3}(?::\d{1,5})?(?:[/?#].*)?$/.test(
      trimmed
    );

  if (isDomainPattern || isIpv4Pattern) {
    const scheme = isIpv4Pattern ? 'http://' : 'https://';
    const candidate = `${scheme}${trimmed}`;
    try {
      new URL(candidate);
      return candidate;
    } catch {
      return buildSearchUrl(trimmed, engine);
    }
  }

  // 3. Без протоколу та валідного доменного шаблону — завжди пошуковий запит (Google за замовчуванням)
  return buildSearchUrl(trimmed, engine);
}

function internalTitleForUrl(url: string): string {
  if (url === 'vortex://newtab') return 'Нова вкладка';
  if (url === 'vortex://settings') return 'Налаштування';
  if (url === 'vortex://benchmarks') return 'Тестування продуктивності';
  return url;
}

function extractErrorMessage(err: unknown, fallback: string): string {
  if (err instanceof Error && err.message) return err.message;
  if (typeof err === 'string' && err.trim()) return err;
  if (err && typeof err === 'object' && 'message' in err) {
    const msg = (err as { message?: unknown }).message;
    if (typeof msg === 'string' && msg.trim()) return msg;
  }
  return fallback;
}

function createTabObject(url = 'vortex://newtab', isIncognito = false): BrowserTab {
  const id = crypto.randomUUID();
  return {
    id,
    webviewLabel: `tab-${id.slice(0, 8)}`,
    url,
    title: internalTitleForUrl(url),
    favicon: null,
    isLoading: false,
    isHttps: url.startsWith('https://'),
    isPinned: false,
    isIncognito,
    isPlayingAudio: false,
    isMuted: false,
    isSuspended: false,
    isDiscarded: false,
    hasActiveDownload: false,
    blockedCount: 0,
    lastActiveAt: Date.now(),
    historyStack: [url],
    historyIndex: 0,
  };
}

export default function App() {
  const [settings, setSettings] = useState<BrowserSettings>(DEFAULT_SETTINGS);
  const [tabs, setTabs] = useState<BrowserTab[]>(() => [createTabObject('vortex://newtab')]);
  const [activeTabId, setActiveTabId] = useState<string>('');
  const [closedTabsStack, setClosedTabsStack] = useState<BrowserTab[]>([]);

  const [adblockStatus, setAdblockStatus] = useState<AdBlockStatus | null>(null);
  const [bookmarks, setBookmarks] = useState<BookmarkItem[]>([]);
  const [bookmarkFolders, setBookmarkFolders] = useState<BookmarkFolder[]>([]);
  const [history, setHistory] = useState<HistoryItem[]>([]);
  const [downloads, setDownloads] = useState<DownloadRecord[]>([]);
  const [vaultStatus, setVaultStatus] = useState<VaultStatus>({
    isConfigured: false,
    isUnlocked: false,
    entryCount: 0,
  });
  const [passwords, setPasswords] = useState<PasswordCredential[]>([]);
  const [siteData, setSiteData] = useState<SiteDataSummary[]>([]);
  const [benchmarkRuns, setBenchmarkRuns] = useState<BenchmarkRunRecord[]>([]);

  const [sidePanelOpen, setSidePanelOpen] = useState(false);
  const [sidePanelSection, setSidePanelSection] = useState<SidePanelSection>('bookmarks');
  const [omniboxFocusSignal, setOmniboxFocusSignal] = useState(0);
  const [omniboxOverlayOpen, setOmniboxOverlayOpen] = useState(false);
  const [bookmarksOverlayOpen, setBookmarksOverlayOpen] = useState(false);
  const [tabMenuOverlayOpen, setTabMenuOverlayOpen] = useState(false);
  const [isHtmlFullscreen, setIsHtmlFullscreen] = useState(false);

  const [loginFormDetected, setLoginFormDetected] = useState(false);
  const [matchingPasswords, setMatchingPasswords] = useState<PasswordCredential[]>([]);
  const [pendingSavePrompt, setPendingSavePrompt] = useState<{
    origin: string;
    title: string;
    username: string;
    password: string;
  } | null>(null);

  const iframeRef = useRef<HTMLIFrameElement | null>(null);
  const hydratedRef = useRef(false);

  const activeTab = useMemo(
    () => tabs.find((t) => t.id === activeTabId) || tabs[0],
    [tabs, activeTabId]
  );

  const shouldHideNativeWebview = Boolean(
    omniboxOverlayOpen ||
      bookmarksOverlayOpen ||
      tabMenuOverlayOpen ||
      pendingSavePrompt ||
      (loginFormDetected && matchingPasswords.length > 0) ||
      activeTab?.errorMessage ||
      activeTab?.isSuspended ||
      activeTab?.isDiscarded
  );

  useEffect(() => {
    if (!isTauriRuntime()) return;
    void vortexApi.setNativeOverlayHidden(shouldHideNativeWebview).catch((err) => {
      console.error('setNativeOverlayHidden error:', err);
    });
  }, [shouldHideNativeWebview]);

  // Синхронізація activeTabId при ініціалізації
  useEffect(() => {
    if (!activeTabId && tabs.length > 0) {
      setActiveTabId(tabs[0].id);
    }
  }, [activeTabId, tabs]);

  // Динамічна синхронізація класу 'dark' на кореневому елементі <html>
  useEffect(() => {
    document.documentElement.classList.toggle('dark', settings.theme === 'dark');
  }, [settings.theme]);

  const refreshAdblockStatus = useCallback(async (tabId: string, url: string) => {
    try {
      const st = await vortexApi.getAdBlockStatus(tabId, url);
      setAdblockStatus(st);
    } catch {
      // ignore
    }
  }, []);

  const refreshBookmarks = useCallback(async () => {
    try {
      const data = await vortexApi.listBookmarks();
      setBookmarkFolders(data.folders);
      setBookmarks(data.items);
    } catch {
      // ignore
    }
  }, []);

  const refreshHistory = useCallback(async (q = '') => {
    try {
      const items = await vortexApi.listHistory(q);
      setHistory(items);
    } catch {
      // ignore
    }
  }, []);

  const refreshDownloads = useCallback(async () => {
    try {
      const items = await vortexApi.listDownloads();
      setDownloads(items);
    } catch {
      // ignore
    }
  }, []);

  const refreshVault = useCallback(async () => {
    try {
      const st = await vortexApi.vaultStatus();
      setVaultStatus(st);
      if (st.isUnlocked) {
        const list = await vortexApi.vaultList('');
        setPasswords(list);
      } else {
        setPasswords([]);
      }
    } catch {
      // ignore
    }
  }, []);

  const refreshSiteData = useCallback(async () => {
    try {
      setSiteData(await vortexApi.listSiteData());
    } catch {
      // ignore
    }
  }, []);

  const refreshBenchmarks = useCallback(async () => {
    try {
      setBenchmarkRuns(await vortexApi.listBenchmarks());
    } catch {
      // ignore
    }
  }, []);

  const loadTabContent = useCallback(
    async (tabId: string, targetUrl: string, incognito: boolean) => {
      if (targetUrl.startsWith('vortex://')) {
        setTabs((prev) =>
          prev.map((t) =>
            t.id === tabId
              ? {
                  ...t,
                  url: targetUrl,
                  title: internalTitleForUrl(targetUrl),
                  isLoading: false,
                  isHttps: false,
                  isSuspended: false,
                  isDiscarded: false,
                  blockedCount: 0,
                  errorMessage: undefined,
                  renderedHtml: undefined,
                }
              : t
          )
        );
        if (isTauriRuntime()) {
          await vortexApi.nativeNavigateTab(tabId, targetUrl).catch(() => {});
        }
        await refreshAdblockStatus(tabId, targetUrl);
        return;
      }

      if (isTauriRuntime()) {
        setTabs((prev) =>
          prev.map((t) =>
            t.id === tabId
              ? {
                  ...t,
                  url: targetUrl,
                  isLoading: true,
                  isHttps: targetUrl.startsWith('https://'),
                  isSuspended: false,
                  isDiscarded: false,
                  errorMessage: undefined,
                }
              : t
          )
        );
        try {
          await vortexApi.nativeNavigateTab(tabId, targetUrl);
          await refreshAdblockStatus(tabId, targetUrl);
        } catch (err: unknown) {
          console.error('nativeNavigateTab error:', err);
          const message = extractErrorMessage(err, 'Помилка навігації WebView2');
          setTabs((prev) =>
            prev.map((t) =>
              t.id === tabId
                ? { ...t, isLoading: false, errorMessage: message }
                : t
            )
          );
        }
        return;
      }

      setTabs((prev) =>
        prev.map((t) =>
          t.id === tabId
            ? {
                ...t,
                url: targetUrl,
                isLoading: true,
                isHttps: targetUrl.startsWith('https://'),
                isSuspended: false,
                isDiscarded: false,
                errorMessage: undefined,
              }
            : t
        )
      );
      setLoginFormDetected(false);
      setMatchingPasswords([]);

      try {
        const res = await vortexApi.browsePage(targetUrl, tabId, incognito);
        setTabs((prev) =>
          prev.map((t) =>
            t.id === tabId
              ? {
                  ...t,
                  url: res.finalUrl,
                  title: res.title || res.finalUrl,
                  favicon: res.favicon,
                  isLoading: false,
                  isHttps: res.isHttps,
                  blockedCount: res.blockedCount,
                  blockedDomains: res.blockedDomains,
                  loadTimeMs: res.loadTimeMs,
                  rawBytes: res.rawBytes,
                  filteredBytes: res.filteredBytes,
                  renderedHtml: res.html,
                  errorMessage: undefined,
                  lastActiveAt: Date.now(),
                }
              : t
          )
        );
        await refreshAdblockStatus(tabId, res.finalUrl);
        if (!incognito) void refreshHistory('');
      } catch (err: unknown) {
        const message = err instanceof Error ? err.message : 'Помилка завантаження сторінки';
        setTabs((prev) =>
          prev.map((t) =>
            t.id === tabId
              ? {
                  ...t,
                  isLoading: false,
                  errorMessage: message,
                }
              : t
          )
        );
      }
    },
    [refreshAdblockStatus, refreshHistory]
  );

  // Початкове завантаження налаштувань, сесії, закладок, історії, завантажень та паролів
  useEffect(() => {
    (async () => {
      try {
        const loadedSettings = await vortexApi.getSettings();
        setSettings(loadedSettings);

        if (isTauriRuntime()) {
          const [nativeTabs, nativeActiveId] = await Promise.all([
            vortexApi.nativeGetTabs(),
            vortexApi.nativeGetActiveTabId(),
          ]);

          if (nativeTabs.length > 0) {
            const hydratedTabs: BrowserTab[] = nativeTabs.map((t) => ({
              ...t,
              historyStack: [t.url],
              historyIndex: 0,
            }));
            const targetActiveId =
              (nativeActiveId && hydratedTabs.some((t) => t.id === nativeActiveId)
                ? nativeActiveId
                : hydratedTabs[0].id);
            setTabs(hydratedTabs);
            setActiveTabId(targetActiveId);
          } else {
            const first = createTabObject(loadedSettings.startupPage || 'vortex://newtab');
            await vortexApi.nativeCreateTab(first.url, false, true, first.id);
            setTabs([first]);
            setActiveTabId(first.id);
          }
        } else if (loadedSettings.restoreTabsOnStartup) {
          const savedTabs = await vortexApi.loadSessionTabs();
          if (savedTabs.length > 0) {
            const restored: BrowserTab[] = savedTabs.map((s) => ({
              id: s.id,
              webviewLabel: `tab-${s.id.slice(0, 8)}`,
              url: s.url,
              title: s.title,
              favicon: s.favicon,
              isLoading: false,
              isHttps: s.url.startsWith('https://'),
              isPinned: s.isPinned,
              isIncognito: false,
              isPlayingAudio: false,
              isMuted: false,
              isSuspended: false,
              isDiscarded: false,
              hasActiveDownload: false,
              blockedCount: 0,
              lastActiveAt: Date.now(),
              historyStack: [s.url],
              historyIndex: 0,
            }));
            const active = savedTabs.find((s) => s.isActive)?.id || restored[0].id;
            setTabs(restored);
            setActiveTabId(active);
            const activeObj = restored.find((t) => t.id === active);
            if (activeObj && !activeObj.url.startsWith('vortex://')) {
              void loadTabContent(activeObj.id, activeObj.url, false);
            }
          } else {
            const first = createTabObject(loadedSettings.startupPage || 'vortex://newtab');
            setTabs([first]);
            setActiveTabId(first.id);
          }
        } else {
          const first = createTabObject(loadedSettings.startupPage || 'vortex://newtab');
          setTabs([first]);
          setActiveTabId(first.id);
        }
      } catch (err: unknown) {
        console.error('Startup hydration error:', err);
      } finally {
        hydratedRef.current = true;
      }

      void refreshBookmarks();
      void refreshHistory();
      void refreshDownloads();
      void refreshVault();
      void refreshAdblockStatus('default', 'vortex://newtab');
    })();
  }, [loadTabContent, refreshAdblockStatus, refreshBookmarks, refreshDownloads, refreshHistory, refreshVault]);

  // Збереження сесії вкладок у SQLite при зміні (у Web-режимі; у Tauri зберігає Rust WebviewManager)
  useEffect(() => {
    if (!hydratedRef.current || isTauriRuntime()) return;
    const payload = tabs
      .filter((t) => !t.isIncognito)
      .map((t, idx) => ({
        id: t.id,
        url: t.url,
        title: t.title,
        favicon: t.favicon,
        isPinned: t.isPinned,
        isActive: t.id === activeTabId,
        sortOrder: idx,
      }));
    void vortexApi.saveSessionTabs(payload);
  }, [tabs, activeTabId]);

  // Полінг прогресу активних завантажень (лише коли є активні завантаження!)
  const activeDownloadsCount = useMemo(
    () => downloads.filter((d) => d.status === 'downloading').length,
    [downloads]
  );

  useEffect(() => {
    if (activeDownloadsCount === 0) return;
    const timer = setInterval(() => {
      void refreshDownloads();
    }, 500);
    return () => clearInterval(timer);
  }, [activeDownloadsCount, refreshDownloads]);

  // Memory Saver (автоматичне призупинення / вивантаження неактивних вкладок)
  useEffect(() => {
    if (settings.memorySaverMode === 'off') return;
    const interval = setInterval(() => {
      const now = Date.now();
      const suspendMs =
        (settings.memorySaverMode === 'aggressive' ? 2 : settings.memorySaverSuspendMins) * 60_000;
      const discardMs =
        (settings.memorySaverMode === 'aggressive' ? 5 : settings.memorySaverDiscardMins) * 60_000;

      setTabs((prev) =>
        prev.map((t) => {
          if (
            t.id === activeTabId ||
            t.isPinned ||
            t.isPlayingAudio ||
            t.hasActiveDownload ||
            t.url.startsWith('vortex://')
          ) {
            return t;
          }
          const idle = now - t.lastActiveAt;
          if (idle >= discardMs && !t.isDiscarded) {
            return { ...t, isDiscarded: true, isSuspended: false, renderedHtml: undefined };
          }
          if (idle >= suspendMs && !t.isSuspended && !t.isDiscarded) {
            return { ...t, isSuspended: true };
          }
          return t;
        })
      );
    }, 30_000);
    return () => clearInterval(interval);
  }, [settings.memorySaverMode, settings.memorySaverSuspendMins, settings.memorySaverDiscardMins, activeTabId]);

  // Навігація у поточній або новій вкладці
  const navigateTab = useCallback(
    (tabId: string, rawInput: string) => {
      const targetUrl = normalizeUrlInput(rawInput, settings.searchEngine);
      let isIncognito = false;
      setTabs((prev) =>
        prev.map((t) => {
          if (t.id !== tabId) return t;
          isIncognito = t.isIncognito;
          const nextStack = [...t.historyStack.slice(0, t.historyIndex + 1), targetUrl];
          return {
            ...t,
            historyStack: nextStack,
            historyIndex: nextStack.length - 1,
          };
        })
      );
      void loadTabContent(tabId, targetUrl, isIncognito);
    },
    [settings.searchEngine, loadTabContent]
  );

  const createTab = useCallback(
    (url = 'vortex://newtab', incognito = false) => {
      const normalized = url.startsWith('vortex://')
        ? url
        : normalizeUrlInput(url, settings.searchEngine);
      const tab = createTabObject(normalized, incognito);
      setTabs((prev) => [...prev, tab]);
      setActiveTabId(tab.id);
      if (isTauriRuntime()) {
        void vortexApi
          .nativeCreateTab(normalized, incognito, true, tab.id)
          .then(() => refreshAdblockStatus(tab.id, normalized))
          .catch((err: unknown) => {
            console.error('nativeCreateTab error:', err);
            const message = extractErrorMessage(err, 'Не вдалося створити вкладку WebView2');
            setTabs((prev) =>
              prev.map((t) =>
                t.id === tab.id ? { ...t, isLoading: false, errorMessage: message } : t
              )
            );
          });
      } else if (!normalized.startsWith('vortex://')) {
        void loadTabContent(tab.id, normalized, incognito);
      } else {
        void refreshAdblockStatus(tab.id, normalized);
      }
    },
    [settings.searchEngine, loadTabContent, refreshAdblockStatus]
  );

  // Живі події від нативного Rust WebView2 (завантаження сторінки, заголовок, AdBlock, target="_blank")
  useEffect(() => {
    if (!isTauriRuntime()) return;
    let disposed = false;
    const unlisteners: Array<() => void> = [];

    const setup = async () => {
      const u1 = await listen<Omit<BrowserTab, 'historyStack' | 'historyIndex'>>(
        'vortex://tab-updated',
        (event) => {
          const updated = event.payload;
          setTabs((prev) =>
            prev.map((t) => {
              if (t.id !== updated.id) return t;
              const curUrl = t.historyStack[t.historyIndex];
              const urlChanged = updated.url && updated.url !== curUrl;
              const nextStack = urlChanged
                ? [...t.historyStack.slice(0, t.historyIndex + 1), updated.url]
                : t.historyStack;
              return {
                ...t,
                url: updated.url,
                title: updated.title || t.title,
                isLoading: updated.isLoading,
                isHttps: updated.isHttps,
                lastActiveAt: updated.lastActiveAt,
                errorMessage: undefined,
                historyStack: nextStack,
                historyIndex: nextStack.length - 1,
              };
            })
          );
          if (!updated.isLoading && !updated.isIncognito) {
            void refreshHistory('');
          }
        }
      );
      if (disposed) {
        u1();
        return;
      }
      unlisteners.push(u1);

      const u2 = await listen<string>('vortex://adblock-hit', (event) => {
        const hitTabId = event.payload;
        setTabs((prev) =>
          prev.map((t) => (t.id === hitTabId ? { ...t, blockedCount: t.blockedCount + 1 } : t))
        );
      });
      if (disposed) {
        u2();
        return;
      }
      unlisteners.push(u2);

      const u3 = await listen<string>('vortex://open-new-tab', (event) => {
        if (event.payload) {
          createTab(event.payload, false);
        }
      });
      if (disposed) {
        u3();
        return;
      }
      unlisteners.push(u3);

      const u4 = await listen<boolean>('vortex://fullscreen-changed', (event) => {
        setIsHtmlFullscreen(Boolean(event.payload));
      });
      if (disposed) {
        u4();
        return;
      }
      unlisteners.push(u4);
    };

    void setup();
    return () => {
      disposed = true;
      unlisteners.forEach((fn) => fn());
    };
  }, [createTab, refreshHistory]);

  const selectTab = useCallback(
    (tabId: string) => {
      setActiveTabId(tabId);
      setLoginFormDetected(false);
      setMatchingPasswords([]);
      let needReloadUrl: string | null = null;
      let tabIncognito = false;

      setTabs((prev) =>
        prev.map((t) => {
          if (t.id !== tabId) return t;
          const wasSleeping = t.isSuspended || t.isDiscarded;
          if ((t.isDiscarded || (!t.renderedHtml && !t.url.startsWith('vortex://'))) && !isTauriRuntime()) {
            needReloadUrl = t.url;
            tabIncognito = t.isIncognito;
          }
          return {
            ...t,
            lastActiveAt: Date.now(),
            isSuspended: false,
            isDiscarded: false,
            isLoading: wasSleeping && needReloadUrl ? true : t.isLoading,
          };
        })
      );

      if (isTauriRuntime()) {
        void vortexApi.nativeSwitchTab(tabId).catch((err: unknown) => {
          console.error('nativeSwitchTab error:', err);
          const message = extractErrorMessage(err, 'Не вдалося перемкнути вкладку WebView2');
          setTabs((prev) =>
            prev.map((t) =>
              t.id === tabId ? { ...t, isLoading: false, errorMessage: message } : t
            )
          );
        });
      } else if (needReloadUrl) {
        void loadTabContent(tabId, needReloadUrl, tabIncognito);
      }

      const found = tabs.find((t) => t.id === tabId);
      if (found) void refreshAdblockStatus(tabId, found.url);
    },
    [tabs, loadTabContent, refreshAdblockStatus]
  );

  const closeTab = useCallback(
    (tabId: string) => {
      const idx = tabs.findIndex((t) => t.id === tabId);
      if (idx === -1) return;
      const removed = tabs[idx];
      if (!removed.isIncognito) {
        setClosedTabsStack((st) => [removed, ...st].slice(0, 15));
      }

      const remaining = tabs.filter((t) => t.id !== tabId);
      let createdFreshTab: BrowserTab | null = null;

      if (remaining.length === 0) {
        const fresh = createTabObject(settings.startupPage || 'vortex://newtab');
        createdFreshTab = fresh;
        setTabs([fresh]);
        setActiveTabId(fresh.id);
      } else {
        setTabs(remaining);
        if (tabId === activeTabId) {
          const nextIdx = Math.min(idx, remaining.length - 1);
          const nextTab = remaining[nextIdx];
          setActiveTabId(nextTab.id);
          if (
            !isTauriRuntime() &&
            (nextTab.isDiscarded || (!nextTab.renderedHtml && !nextTab.url.startsWith('vortex://')))
          ) {
            void loadTabContent(nextTab.id, nextTab.url, nextTab.isIncognito);
          }
        }
      }

      if (isTauriRuntime()) {
        void (async () => {
          await vortexApi.nativeCloseTab(tabId).catch(() => {});
          if (createdFreshTab) {
            await vortexApi
              .nativeCreateTab(createdFreshTab.url, false, true, createdFreshTab.id)
              .catch(() => {});
          }
        })();
      }
    },
    [tabs, activeTabId, settings.startupPage, loadTabContent]
  );

  // Повідомлення від завантажених сторінок (навігація за посиланнями, форми входу, автозаповнення)
  useEffect(() => {
    const handler = async (ev: MessageEvent) => {
      if (!ev.data || typeof ev.data !== 'object') return;
      if (ev.data.type === 'vortex:navigate' && ev.data.url) {
        if (ev.data.newTab) {
          createTab(ev.data.url, activeTab?.isIncognito ?? false);
        } else if (activeTab) {
          navigateTab(activeTab.id, ev.data.url);
        }
      } else if (ev.data.type === 'vortex:login-form-detected' && ev.data.url) {
        setLoginFormDetected(true);
        try {
          const matches = await vortexApi.vaultMatchUrl(ev.data.url);
          setMatchingPasswords(matches);
        } catch {
          setMatchingPasswords([]);
        }
      } else if (ev.data.type === 'vortex:login-submit' && ev.data.password) {
        setPendingSavePrompt({
          origin: ev.data.origin || '',
          title: ev.data.title || ev.data.origin || '',
          username: ev.data.username || '',
          password: ev.data.password,
        });
      }
    };
    window.addEventListener('message', handler);
    return () => window.removeEventListener('message', handler);
  }, [activeTab, createTab, navigateTab]);

  // Гарячі клавіші браузера (Ctrl+T, Ctrl+W, Ctrl+Tab, Ctrl+L, F5, Alt+Left/Right)
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.ctrlKey && e.key.toLowerCase() === 't') {
        e.preventDefault();
        createTab('vortex://newtab', false);
      } else if (e.ctrlKey && e.key.toLowerCase() === 'w') {
        e.preventDefault();
        if (activeTab) closeTab(activeTab.id);
      } else if (e.ctrlKey && e.key.toLowerCase() === 'l') {
        e.preventDefault();
        setOmniboxFocusSignal((n) => n + 1);
      } else if (e.ctrlKey && e.key === 'Tab') {
        e.preventDefault();
        if (tabs.length < 2) return;
        const curIdx = tabs.findIndex((t) => t.id === activeTabId);
        const nextIdx = e.shiftKey
          ? (curIdx - 1 + tabs.length) % tabs.length
          : (curIdx + 1) % tabs.length;
        selectTab(tabs[nextIdx].id);
      } else if (e.key === 'F5' || (e.ctrlKey && e.key.toLowerCase() === 'r')) {
        e.preventDefault();
        if (activeTab) {
          void loadTabContent(activeTab.id, activeTab.url, activeTab.isIncognito);
        }
      } else if (e.key === 'F11') {
        e.preventDefault();
        if (isTauriRuntime()) {
          const win = getCurrentWindow();
          void win
            .isFullscreen()
            .then((fs) => win.setFullscreen(!fs))
            .catch(() => {});
        } else if (document.fullscreenElement) {
          void document.exitFullscreen().catch(() => {});
        } else {
          void document.documentElement.requestFullscreen().catch(() => {});
        }
      }
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [activeTab, activeTabId, tabs, createTab, closeTab, selectTab, loadTabContent]);

  const handleBack = () => {
    if (!activeTab || activeTab.historyIndex <= 0) return;
    const nextIdx = activeTab.historyIndex - 1;
    const url = activeTab.historyStack[nextIdx];
    setTabs((prev) =>
      prev.map((t) => (t.id === activeTab.id ? { ...t, historyIndex: nextIdx } : t))
    );
    if (isTauriRuntime() && !url.startsWith('vortex://') && !activeTab.url.startsWith('vortex://')) {
      void vortexApi.nativeNavAction(activeTab.id, 'back');
    } else {
      void loadTabContent(activeTab.id, url, activeTab.isIncognito);
    }
  };

  const handleForward = () => {
    if (!activeTab || activeTab.historyIndex >= activeTab.historyStack.length - 1) return;
    const nextIdx = activeTab.historyIndex + 1;
    const url = activeTab.historyStack[nextIdx];
    setTabs((prev) =>
      prev.map((t) => (t.id === activeTab.id ? { ...t, historyIndex: nextIdx } : t))
    );
    if (isTauriRuntime() && !url.startsWith('vortex://') && !activeTab.url.startsWith('vortex://')) {
      void vortexApi.nativeNavAction(activeTab.id, 'forward');
    } else {
      void loadTabContent(activeTab.id, url, activeTab.isIncognito);
    }
  };

  const handleReloadOrStop = () => {
    if (!activeTab) return;
    if (activeTab.isLoading) {
      setTabs((prev) => prev.map((t) => (t.id === activeTab.id ? { ...t, isLoading: false } : t)));
      if (isTauriRuntime()) {
        void vortexApi.nativeNavAction(activeTab.id, 'stop').catch(() => {});
      }
    } else if (
      isTauriRuntime() &&
      !activeTab.url.startsWith('vortex://') &&
      !activeTab.errorMessage
    ) {
      void vortexApi.nativeNavAction(activeTab.id, 'reload').catch(() => {
        void loadTabContent(activeTab.id, activeTab.url, activeTab.isIncognito);
      });
    } else {
      void loadTabContent(activeTab.id, activeTab.url, activeTab.isIncognito);
    }
  };

  const sleepingTabsCount = tabs.filter((t) => t.isSuspended || t.isDiscarded).length;

  return (
    <div className="flex flex-col h-screen w-screen bg-[#F4F6FA] text-slate-900 dark:bg-[#0B0D11] dark:text-[#E6EAF2] overflow-hidden">
      <div className={isHtmlFullscreen ? 'hidden' : 'flex flex-col shrink-0'}>
        {/* Верхня панель вкладок */}
        <TabStrip
        tabs={tabs}
        activeTabId={activeTab?.id || ''}
        canReopenClosed={closedTabsStack.length > 0}
        onSelectTab={selectTab}
        onCreateTab={createTab}
        onCloseTab={closeTab}
        onDuplicateTab={(id) => {
          const t = tabs.find((x) => x.id === id);
          if (t) createTab(t.url, t.isIncognito);
        }}
        onTogglePin={(id) =>
          setTabs((prev) =>
            prev.map((t) => (t.id === id ? { ...t, isPinned: !t.isPinned } : t))
          )
        }
        onToggleMute={(id) =>
          setTabs((prev) => prev.map((t) => (t.id === id ? { ...t, isMuted: !t.isMuted } : t)))
        }
        onSleepTab={(id) =>
          setTabs((prev) =>
            prev.map((t) =>
              t.id === id && id !== activeTabId
                ? { ...t, isSuspended: true, renderedHtml: undefined }
                : t
            )
          )
        }
        onReloadTab={(id) => {
          const t = tabs.find((x) => x.id === id);
          if (t) void loadTabContent(t.id, t.url, t.isIncognito);
        }}
        onCloseOtherTabs={(id) => {
          const toClose = tabs.filter((t) => t.id !== id && !t.isPinned);
          setTabs((prev) => prev.filter((t) => t.id === id || t.isPinned));
          setActiveTabId(id);
          if (isTauriRuntime()) {
            for (const t of toClose) {
              void vortexApi.nativeCloseTab(t.id);
            }
            void vortexApi.nativeSwitchTab(id);
          }
        }}
        onReopenClosedTab={() => {
          const [last, ...rest] = closedTabsStack;
          if (!last) return;
          setClosedTabsStack(rest);
          createTab(last.url, false);
        }}
        onOverlayVisibilityChange={setTabMenuOverlayOpen}
      />

      {/* Панель навігації, адресного рядка та перемикача AdBlock */}
      {activeTab && (
        <OmniboxBar
          activeTab={activeTab}
          settings={settings}
          adblockStatus={adblockStatus}
          bookmarks={bookmarks}
          bookmarkFolders={bookmarkFolders}
          history={history}
          activeDownloadsCount={activeDownloadsCount}
          sidePanelOpen={sidePanelOpen}
          sidePanelSection={sidePanelSection}
          vaultUnlocked={vaultStatus.isUnlocked}
          hasMatchingPasswords={matchingPasswords.length > 0}
          omniboxFocusSignal={omniboxFocusSignal}
          onNavigate={(input) => navigateTab(activeTab.id, input)}
          onBack={handleBack}
          onForward={handleForward}
          onReloadOrStop={handleReloadOrStop}
          onHome={() => navigateTab(activeTab.id, settings.startupPage || 'vortex://newtab')}
          onToggleAdBlockDomain={async (domain, disableOnDomain) => {
            await vortexApi.toggleAdBlockDomain(domain, disableOnDomain);
            await refreshAdblockStatus(activeTab.id, activeTab.url);
            if (!activeTab.url.startsWith('vortex://')) {
              await loadTabContent(activeTab.id, activeTab.url, activeTab.isIncognito);
            }
          }}
          onUpdateAdBlockFilters={async () => {
            await vortexApi.updateAdBlockFilters();
            await refreshAdblockStatus(activeTab.id, activeTab.url);
          }}
          onToggleBookmark={async (title, url, folderId, onBar) => {
            const existing = bookmarks.find((b) => b.url === url);
            await vortexApi.saveBookmark({
              id: existing?.id,
              title,
              url,
              folderId,
              onBar,
            });
            await refreshBookmarks();
          }}
          onDeleteBookmark={async (id) => {
            await vortexApi.deleteBookmark(id);
            await refreshBookmarks();
          }}
          onToggleSidePanel={(section) => {
            if (section) {
              if (sidePanelOpen && sidePanelSection === section) {
                setSidePanelOpen(false);
              } else {
                setSidePanelSection(section);
                setSidePanelOpen(true);
              }
            } else {
              setSidePanelOpen((v) => !v);
            }
          }}
          onOpenInternalPage={(url) => {
            if (activeTab.url === url) return;
            navigateTab(activeTab.id, url);
          }}
          onOverlayVisibilityChange={setOmniboxOverlayOpen}
        />
      )}

      {/* Панель швидкого доступу закладок */}
      {settings.showBookmarksBar && (
        <BookmarksBar
          folders={bookmarkFolders}
          bookmarks={bookmarks}
          onOpenUrl={(url, newTab) => {
            if (newTab) {
              createTab(url, activeTab?.isIncognito ?? false);
            } else if (activeTab) {
              navigateTab(activeTab.id, url);
            }
          }}
          onOverlayVisibilityChange={setBookmarksOverlayOpen}
        />
      )}
      </div>

      {/* Основна робоча область: сторінка + опційна бічна панель */}
      <div className="flex flex-1 min-h-0 min-w-0">
        {activeTab && (
          <WebViewport
            activeTab={activeTab}
            isFullscreen={isHtmlFullscreen}
            settings={settings}
            bookmarks={bookmarks}
            adblockStatus={adblockStatus}
            siteData={siteData}
            benchmarkRuns={benchmarkRuns}
            sleepingTabsCount={sleepingTabsCount}
            totalTabsCount={tabs.length}
            matchingPasswords={matchingPasswords}
            loginFormDetected={loginFormDetected}
            pendingSavePrompt={pendingSavePrompt}
            onNavigate={(input) => navigateTab(activeTab.id, input)}
            onWakeTab={(id) => selectTab(id)}
            onReloadTab={(id) => {
              const t = tabs.find((x) => x.id === id);
              if (t) void loadTabContent(t.id, t.url, t.isIncognito);
            }}
            onUpdateSettings={async (patch) => {
              const next = await vortexApi.updateSettings(patch);
              setSettings(next);
              await refreshAdblockStatus(activeTab.id, activeTab.url);
            }}
            onToggleAdBlockDomain={async (domain, disableOnDomain) => {
              await vortexApi.toggleAdBlockDomain(domain, disableOnDomain);
              await refreshAdblockStatus(activeTab.id, activeTab.url);
            }}
            onUpdateAdBlockFilters={async () => {
              await vortexApi.updateAdBlockFilters();
              await refreshAdblockStatus(activeTab.id, activeTab.url);
            }}
            onClearSiteData={async (domain) => {
              await vortexApi.clearSiteData(domain);
              await refreshSiteData();
            }}
            onExportAllData={() => vortexApi.exportAllData()}
            onImportAllData={async (payload) => {
              await vortexApi.importAllData(payload);
              setSettings(await vortexApi.getSettings());
              await refreshBookmarks();
              await refreshAdblockStatus(activeTab.id, activeTab.url);
            }}
            onOpenSidePanel={(sec) => {
              setSidePanelSection(sec);
              setSidePanelOpen(true);
            }}
            onRefreshSiteData={refreshSiteData}
            onRefreshBenchmarks={refreshBenchmarks}
            onRunLiveBenchmark={async (urls) => {
              await vortexApi.runLiveBenchmark(urls);
              await refreshBenchmarks();
            }}
            onImportBenchmarkReport={async (payload) => {
              await vortexApi.importBenchmarkJson(payload);
              await refreshBenchmarks();
            }}
            onSaveShortcut={async (title, rawUrl) => {
              const url = normalizeUrlInput(rawUrl, settings.searchEngine);
              await vortexApi.saveBookmark({ title, url, folderId: 'quick-bar', onBar: true });
              await refreshBookmarks();
            }}
            onConfirmSavePassword={async () => {
              if (!pendingSavePrompt) return;
              if (!vaultStatus.isUnlocked) {
                setSidePanelSection('passwords');
                setSidePanelOpen(true);
                return;
              }
              await vortexApi.vaultSave(pendingSavePrompt);
              setPendingSavePrompt(null);
              await refreshVault();
            }}
            onDismissSavePassword={() => setPendingSavePrompt(null)}
            onAutofillCredential={(cred) => {
              iframeRef.current?.contentWindow?.postMessage(
                {
                  type: 'vortex:autofill',
                  tabId: activeTab.id,
                  username: cred.username,
                  password: cred.password,
                },
                '*'
              );
              setLoginFormDetected(false);
            }}
            iframeRef={iframeRef}
          />
        )}

        {sidePanelOpen && !isHtmlFullscreen && (
          <SidePanel
            section={sidePanelSection}
            onChangeSection={setSidePanelSection}
            onClose={() => setSidePanelOpen(false)}
            onOpenUrl={(url, newTab) => {
              if (newTab) {
                createTab(url, activeTab?.isIncognito ?? false);
              } else if (activeTab) {
                navigateTab(activeTab.id, url);
              }
            }}
            bookmarks={bookmarks}
            bookmarkFolders={bookmarkFolders}
            onSaveBookmark={async (item) => {
              await vortexApi.saveBookmark(item);
              await refreshBookmarks();
            }}
            onDeleteBookmark={async (id) => {
              await vortexApi.deleteBookmark(id);
              await refreshBookmarks();
            }}
            onCreateFolder={async (name) => {
              await vortexApi.createBookmarkFolder(name);
              await refreshBookmarks();
            }}
            onImportBookmarksHtml={async (content) => {
              const res = await vortexApi.importBookmarksHtml(content);
              await refreshBookmarks();
              return res.imported;
            }}
            history={history}
            onSearchHistory={(q) => void refreshHistory(q)}
            onDeleteHistoryItem={async (id) => {
              await vortexApi.deleteHistoryItem(id);
              await refreshHistory();
            }}
            onClearHistory={async (hoursBack) => {
              await vortexApi.clearHistory(hoursBack);
              await refreshHistory();
            }}
            downloads={downloads}
            onStartDownload={async (url) => {
              await vortexApi.startDownload(url);
              await refreshDownloads();
            }}
            onPauseDownload={async (id) => {
              await vortexApi.pauseDownload(id);
              await refreshDownloads();
            }}
            onResumeDownload={async (id) => {
              await vortexApi.resumeDownload(id);
              await refreshDownloads();
            }}
            onRetryDownload={async (id) => {
              await vortexApi.retryDownload(id);
              await refreshDownloads();
            }}
            onDeleteDownload={async (id) => {
              await vortexApi.deleteDownload(id);
              await refreshDownloads();
            }}
            vaultStatus={vaultStatus}
            passwords={passwords}
            onVaultSetup={async (mp) => {
              await vortexApi.vaultSetup(mp);
              await refreshVault();
            }}
            onVaultUnlock={async (mp) => {
              await vortexApi.vaultUnlock(mp);
              await refreshVault();
            }}
            onVaultLock={async () => {
              await vortexApi.vaultLock();
              await refreshVault();
            }}
            onSavePassword={async (entry) => {
              await vortexApi.vaultSave(entry);
              await refreshVault();
            }}
            onDeletePassword={async (id) => {
              await vortexApi.vaultDelete(id);
              await refreshVault();
            }}
            onImportPasswordsCsv={async (csv) => {
              const res = await vortexApi.vaultImportCsv(csv);
              await refreshVault();
              return res.imported;
            }}
          />
        )}
      </div>
    </div>
  );
}
