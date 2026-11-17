import React, { useEffect, useRef } from 'react';
import { AlertTriangle, KeyRound, Moon, Play, RotateCw, ShieldCheck } from 'lucide-react';
import { isTauriRuntime, vortexApi } from '../bridge/vortexApi';
import type {
  AdBlockStatus,
  BenchmarkRunRecord,
  BookmarkItem,
  BrowserSettings,
  BrowserTab,
  PasswordCredential,
  SearchEngineId,
  SidePanelSection,
  SiteDataSummary,
} from '../types/browser';
import { NewTabPage } from './NewTabPage';
import { SettingsPage } from './SettingsPage';
import { BenchmarksPage } from './BenchmarksPage';

interface WebViewportProps {
  activeTab: BrowserTab;
  isFullscreen?: boolean;
  settings: BrowserSettings;
  bookmarks: BookmarkItem[];
  adblockStatus: AdBlockStatus | null;
  siteData: SiteDataSummary[];
  benchmarkRuns: BenchmarkRunRecord[];
  sleepingTabsCount: number;
  totalTabsCount: number;
  matchingPasswords: PasswordCredential[];
  loginFormDetected: boolean;
  pendingSavePrompt: { origin: string; title: string; username: string; password: string } | null;
  onNavigate: (urlOrQuery: string) => void;
  onWakeTab: (tabId: string) => void;
  onReloadTab: (tabId: string) => void;
  onUpdateSettings: (patch: Partial<BrowserSettings>) => Promise<void>;
  onToggleAdBlockDomain: (domain: string, disableOnDomain: boolean) => Promise<void>;
  onUpdateAdBlockFilters: () => Promise<void>;
  onClearSiteData: (domain?: string) => Promise<void>;
  onExportAllData: () => Promise<Record<string, unknown>>;
  onImportAllData: (payload: Record<string, unknown>) => Promise<void>;
  onOpenSidePanel: (section: SidePanelSection) => void;
  onRefreshSiteData: () => void;
  onRefreshBenchmarks: () => void;
  onRunLiveBenchmark: (urls: string[]) => Promise<void>;
  onImportBenchmarkReport: (payload: Record<string, unknown>) => Promise<void>;
  onSaveShortcut: (title: string, url: string) => Promise<void>;
  onConfirmSavePassword: () => Promise<void>;
  onDismissSavePassword: () => void;
  onAutofillCredential: (cred: PasswordCredential) => void;
  iframeRef: React.RefObject<HTMLIFrameElement | null>;
}

export const WebViewport: React.FC<WebViewportProps> = ({
  activeTab,
  isFullscreen = false,
  settings,
  bookmarks,
  adblockStatus,
  siteData,
  benchmarkRuns,
  sleepingTabsCount,
  totalTabsCount,
  matchingPasswords,
  loginFormDetected,
  pendingSavePrompt,
  onNavigate,
  onWakeTab,
  onReloadTab,
  onUpdateSettings,
  onToggleAdBlockDomain,
  onUpdateAdBlockFilters,
  onClearSiteData,
  onExportAllData,
  onImportAllData,
  onOpenSidePanel,
  onRefreshSiteData,
  onRefreshBenchmarks,
  onRunLiveBenchmark,
  onImportBenchmarkReport,
  onSaveShortcut,
  onConfirmSavePassword,
  onDismissSavePassword,
  onAutofillCredential,
  iframeRef,
}) => {
  const containerRef = useRef<HTMLDivElement>(null);
  const lastRectKeyRef = useRef<string>('');

  useEffect(() => {
    if (!isTauriRuntime() || !containerRef.current || isFullscreen) return;
    const el = containerRef.current;
    let rafId = 0;

    const sync = () => {
      const r = el.getBoundingClientRect();
      const x = Math.round(r.left);
      const y = Math.round(r.top);
      const width = Math.max(100, Math.round(r.width));
      const height = Math.max(100, Math.round(r.height));
      const key = `${activeTab.id}|${activeTab.url}|${x},${y},${width},${height}`;
      if (lastRectKeyRef.current === key) return;
      lastRectKeyRef.current = key;
      void vortexApi.syncNativeViewport({ x, y, width, height });
    };

    const scheduleSync = () => {
      if (rafId) cancelAnimationFrame(rafId);
      rafId = requestAnimationFrame(sync);
    };

    sync();
    const ro = new ResizeObserver(scheduleSync);
    ro.observe(el);
    window.addEventListener('resize', scheduleSync);
    return () => {
      if (rafId) cancelAnimationFrame(rafId);
      ro.disconnect();
      window.removeEventListener('resize', scheduleSync);
    };
  }, [activeTab.id, activeTab.url, isFullscreen]);

  const isInternal = activeTab.url.startsWith('vortex://');

  return (
    <div
      ref={containerRef}
      className="relative flex-1 min-w-0 min-h-0 bg-[#F4F6FA] dark:bg-[#0B0D11] overflow-hidden"
    >
      {/* Панель пропозиції збереження пароля */}
      {pendingSavePrompt && (
        <div className="absolute top-3 right-4 z-40 flex items-center gap-3 rounded-xl bg-white dark:bg-[#151922] border border-blue-500/40 shadow-2xl px-4 py-2.5 text-[13px]">
          <KeyRound className="w-4 h-4 text-blue-600 dark:text-blue-400 shrink-0" />
          <div className="min-w-0">
            <span className="font-medium text-slate-900 dark:text-white">
              Зберегти пароль для {pendingSavePrompt.origin}?
            </span>
            {pendingSavePrompt.username && (
              <span className="ml-2 text-slate-500 dark:text-[#8F98A8] font-mono text-[12px]">
                {pendingSavePrompt.username}
              </span>
            )}
          </div>
          <button
            type="button"
            onClick={onConfirmSavePassword}
            className="px-3 py-1 rounded-lg bg-blue-600 hover:bg-blue-500 text-white text-[12px] font-medium"
          >
            Зберегти
          </button>
          <button
            type="button"
            onClick={onDismissSavePassword}
            className="px-2 py-1 text-[12px] text-slate-500 hover:text-slate-900 dark:text-[#8F98A8] dark:hover:text-white"
          >
            Не зараз
          </button>
        </div>
      )}

      {/* Панель підтвердження автозаповнення */}
      {!pendingSavePrompt && loginFormDetected && matchingPasswords.length > 0 && (
        <div className="absolute top-3 right-4 z-40 flex items-center gap-3 rounded-xl bg-white dark:bg-[#151922] border border-emerald-500/40 shadow-2xl px-4 py-2.5 text-[13px]">
          <KeyRound className="w-4 h-4 text-emerald-600 dark:text-emerald-400 shrink-0" />
          <span className="text-slate-900 dark:text-white">
            Знайдено обліковий запис для{' '}
            <strong className="font-mono">{matchingPasswords[0].origin}</strong>
          </span>
          <button
            type="button"
            onClick={() => onAutofillCredential(matchingPasswords[0])}
            className="px-3 py-1 rounded-lg bg-emerald-600 hover:bg-emerald-500 text-white text-[12px] font-medium"
          >
            Автозаповнити ({matchingPasswords[0].username || 'логін'})
          </button>
        </div>
      )}

      {/* Внутрішні сторінки */}
      {activeTab.url === 'vortex://newtab' && (
        <NewTabPage
          isIncognito={activeTab.isIncognito}
          settings={settings}
          bookmarks={bookmarks}
          adblockStatus={adblockStatus}
          sleepingTabsCount={sleepingTabsCount}
          totalTabsCount={totalTabsCount}
          onNavigate={onNavigate}
          onChangeSearchEngine={(engine: SearchEngineId) =>
            void onUpdateSettings({ searchEngine: engine })
          }
          onOpenSidePanel={onOpenSidePanel}
          onSaveShortcut={onSaveShortcut}
        />
      )}

      {activeTab.url === 'vortex://settings' && (
        <SettingsPage
          settings={settings}
          adblockStatus={adblockStatus}
          siteData={siteData}
          onUpdateSettings={onUpdateSettings}
          onToggleAdBlockDomain={onToggleAdBlockDomain}
          onUpdateAdBlockFilters={onUpdateAdBlockFilters}
          onClearSiteData={onClearSiteData}
          onExportAllData={onExportAllData}
          onImportAllData={onImportAllData}
          onOpenSidePanel={onOpenSidePanel}
          onRefreshSiteData={onRefreshSiteData}
        />
      )}

      {activeTab.url === 'vortex://benchmarks' && (
        <BenchmarksPage
          runs={benchmarkRuns}
          onRefreshRuns={onRefreshBenchmarks}
          onRunLiveBenchmark={onRunLiveBenchmark}
          onImportReport={onImportBenchmarkReport}
        />
      )}

      {/* Екран призупиненої / вивантаженої вкладки */}
      {!isInternal && (activeTab.isSuspended || activeTab.isDiscarded) && (
        <div className="flex flex-col items-center justify-center h-full w-full bg-[#F4F6FA] dark:bg-[#0B0D11] p-8 text-center">
          <div className="max-w-md space-y-4">
            <div className="mx-auto flex items-center justify-center w-12 h-12 rounded-2xl bg-white dark:bg-[#151922] border border-slate-200 dark:border-white/[0.08] text-blue-600 dark:text-blue-400">
              <Moon className="w-6 h-6" />
            </div>
            <h2 className="text-[18px] font-semibold text-slate-900 dark:text-white">
              Вкладка {activeTab.isDiscarded ? 'вивантажена з RAM' : 'призупинена'} (Memory Saver)
            </h2>
            <p className="text-[13px] text-slate-500 dark:text-[#8F98A8] leading-relaxed">
              Vortex Browser звільнив оперативну пам’ять та зупинив фонові скрипти для{' '}
              <span className="font-mono text-slate-700 dark:text-[#CBD5E1]">{activeTab.url}</span>.
            </p>
            <button
              type="button"
              onClick={() => onWakeTab(activeTab.id)}
              className="inline-flex items-center gap-2 px-5 py-2.5 rounded-lg bg-blue-600 hover:bg-blue-500 text-white text-[13px] font-medium"
            >
              <Play className="w-4 h-4" />
              <span>Відновити вкладку</span>
            </button>
          </div>
        </div>
      )}

      {/* Нативний WebView2 хост */}
      {!isInternal &&
        !activeTab.isSuspended &&
        !activeTab.isDiscarded &&
        !activeTab.errorMessage &&
        isTauriRuntime() && (
          <div className="flex h-full w-full items-center justify-center bg-[#F4F6FA] dark:bg-[#0B0D11] text-[13px] text-slate-500 dark:text-[#64748B]">
            WebView2 рендерить сторінку у нативному дочірньому вікні...
          </div>
        )}

      {/* Помилка завантаження сторінки */}
      {!isInternal && !activeTab.isSuspended && !activeTab.isDiscarded && activeTab.errorMessage && (
          <div className="flex flex-col items-center justify-center h-full w-full bg-[#F4F6FA] dark:bg-[#0B0D11] p-8 text-center">
            <div className="max-w-lg space-y-4">
              <div className="mx-auto flex items-center justify-center w-12 h-12 rounded-2xl bg-amber-500/10 border border-amber-500/25 text-amber-600 dark:text-amber-400">
                <AlertTriangle className="w-6 h-6" />
              </div>
              <h2 className="text-[18px] font-semibold text-slate-900 dark:text-white">
                Не вдалося відкрити сторінку
              </h2>
              <p className="text-[13px] text-slate-500 dark:text-[#8F98A8] font-mono break-all">
                {activeTab.url}
              </p>
              <p className="text-[13px] text-amber-700 dark:text-amber-300/90">
                {activeTab.errorMessage}
              </p>
              <div className="flex items-center justify-center gap-3 pt-2">
                <button
                  type="button"
                  onClick={() => onReloadTab(activeTab.id)}
                  className="inline-flex items-center gap-2 px-4 py-2 rounded-lg bg-blue-600 hover:bg-blue-500 text-white text-[13px] font-medium"
                >
                  <RotateCw className="w-4 h-4" />
                  <span>Спробувати знову</span>
                </button>
              </div>
            </div>
          </div>
        )}

      {/* Рендер реальної сторінки */}
      {!isInternal &&
        !activeTab.isSuspended &&
        !activeTab.isDiscarded &&
        !isTauriRuntime() &&
        !activeTab.errorMessage && (
          <div className="flex flex-col h-full w-full">
            {activeTab.loadTimeMs !== undefined && (
              <div className="flex items-center justify-between px-3 py-1 bg-slate-100 dark:bg-[#0E1117] border-b border-slate-200 dark:border-white/[0.06] text-[11px] text-slate-600 dark:text-[#8F98A8] font-mono tabular-nums">
                <div className="flex items-center gap-2 truncate">
                  <ShieldCheck className="w-3.5 h-3.5 text-emerald-600 dark:text-emerald-400 shrink-0" />
                  <span className="truncate">
                    Завантажено за {activeTab.loadTimeMs} мс · Заблоковано ресурсів:{' '}
                    {activeTab.blockedCount}
                    {activeTab.rawBytes && activeTab.filteredBytes
                      ? ` · Розмір: ${(activeTab.filteredBytes / 1024).toFixed(1)} КБ`
                      : ''}
                  </span>
                </div>
                {activeTab.blockedDomains && activeTab.blockedDomains.length > 0 && (
                  <span className="truncate max-w-[360px] text-slate-400 dark:text-[#64748B]">
                    Заблоковано: {activeTab.blockedDomains.slice(0, 4).join(', ')}
                  </span>
                )}
              </div>
            )}
            <iframe
              ref={iframeRef}
              title={activeTab.title}
              srcDoc={activeTab.renderedHtml || ''}
              sandbox="allow-scripts allow-forms allow-same-origin"
              allow="fullscreen; autoplay; encrypted-media; picture-in-picture"
              allowFullScreen
              className="flex-1 w-full border-0 bg-white"
            />
          </div>
        )}
    </div>
  );
};
