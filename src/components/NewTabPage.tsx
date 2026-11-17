import React, { useState } from 'react';
import { Activity, EyeOff, Globe, KeyRound, Plus, Search, Settings } from 'lucide-react';
import type {
  AdBlockStatus,
  BookmarkItem,
  BrowserSettings,
  SearchEngineId,
  SidePanelSection,
} from '../types/browser';

interface NewTabPageProps {
  isIncognito: boolean;
  settings: BrowserSettings;
  bookmarks: BookmarkItem[];
  adblockStatus: AdBlockStatus | null;
  sleepingTabsCount: number;
  totalTabsCount: number;
  onNavigate: (urlOrQuery: string) => void;
  onChangeSearchEngine: (engine: SearchEngineId) => void;
  onOpenSidePanel: (section: SidePanelSection) => void;
  onSaveShortcut: (title: string, url: string) => Promise<void>;
}

const ENGINE_LABELS: Record<SearchEngineId, string> = {
  google: 'Google',
  duckduckgo: 'DuckDuckGo',
  brave: 'Brave Search',
  bing: 'Bing',
};

const MEMORY_MODE_LABELS: Record<string, string> = {
  off: 'Вимкнено',
  standard: 'Стандартний',
  aggressive: 'Агресивний',
};

export const NewTabPage: React.FC<NewTabPageProps> = ({
  isIncognito,
  settings,
  bookmarks,
  adblockStatus,
  sleepingTabsCount,
  totalTabsCount,
  onNavigate,
  onChangeSearchEngine,
  onOpenSidePanel,
  onSaveShortcut,
}) => {
  const [query, setQuery] = useState('');
  const [addingShortcut, setAddingShortcut] = useState(false);
  const [scTitle, setScTitle] = useState('');
  const [scUrl, setScUrl] = useState('');

  const quickLinks = bookmarks.slice(0, 8);

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!query.trim()) return;
    onNavigate(query.trim());
  };

  return (
    <div className="flex flex-col items-center justify-center min-h-full w-full bg-[#F4F6FA] dark:bg-[#0B0D11] px-6 py-12 overflow-y-auto">
      <div className="w-full max-w-[740px] space-y-10">
        {/* Заголовок стартової сторінки */}
        <div className="text-center space-y-3">
          <div className="inline-flex items-center justify-center gap-3">
            {isIncognito ? (
              <EyeOff className="w-8 h-8 text-amber-600 dark:text-amber-400" />
            ) : (
              <svg className="w-9 h-9 text-blue-600 dark:text-blue-400" viewBox="0 0 24 24" fill="none">
                <path
                  d="M12 3C7.03 3 3 7.03 3 12c0 3.2 1.67 6.01 4.19 7.6-.66-1.78-.52-3.84.56-5.58 1.48-2.38 4.46-3.44 7.12-2.66 1.54.45 2.81 1.45 3.61 2.75A8.96 8.96 0 0 0 21 12c0-4.97-4.03-9-9-9Z"
                  fill="currentColor"
                />
                <circle cx="12" cy="12" r="2.75" className="fill-[#F4F6FA] dark:fill-[#0B0D11]" />
              </svg>
            )}
            <h1 className="text-[28px] font-semibold tracking-tight text-slate-900 dark:text-white">
              {isIncognito ? 'Приватна вкладка Vortex' : 'Vortex Browser'}
            </h1>
          </div>
          <p className="text-[13px] text-slate-500 dark:text-[#8F98A8]">
            {isIncognito
              ? 'Історія відвідувань, пошукові запити та дані сайтів не зберігаються після закриття цієї вкладки.'
              : `Пошукова система: ${ENGINE_LABELS[settings.searchEngine]} · AdBlock: ${settings.adblockEnabled ? 'Активний' : 'Вимкнено'} · Memory Saver: ${MEMORY_MODE_LABELS[settings.memorySaverMode]}`}
          </p>
        </div>

        {/* Пошуковий рядок */}
        <div className="space-y-3">
          <form
            onSubmit={handleSubmit}
            className="flex items-center h-[50px] w-full rounded-xl bg-white dark:bg-[#151922] border border-slate-200 dark:border-white/[0.1] focus-within:border-blue-500/70 px-4 shadow-md dark:shadow-xl transition-colors"
          >
            <Search className="w-5 h-5 text-slate-400 dark:text-[#64748B] mr-3 shrink-0" />
            <input
              type="text"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder={`Пошук у ${ENGINE_LABELS[settings.searchEngine]} або введення веб-адреси...`}
              autoFocus
              className="flex-1 bg-transparent text-[15px] text-slate-900 dark:text-white placeholder:text-slate-400 dark:placeholder:text-[#64748B] focus:outline-none"
            />
            <button
              type="submit"
              className="ml-2 px-4 py-1.5 rounded-lg bg-blue-600 hover:bg-blue-500 text-white text-[13px] font-medium transition-colors whitespace-nowrap shrink-0"
            >
              Перейти
            </button>
          </form>

          {/* Вибір пошукової системи */}
          <div className="flex items-center justify-center gap-1 text-[12px]">
            {(Object.keys(ENGINE_LABELS) as SearchEngineId[]).map((id) => (
              <button
                key={id}
                type="button"
                onClick={() => onChangeSearchEngine(id)}
                className={`px-3 py-1 rounded-md transition-colors whitespace-nowrap ${
                  settings.searchEngine === id
                    ? 'bg-white text-slate-900 border border-slate-200 shadow-2xs dark:border-transparent dark:bg-[#1A202C] dark:text-white font-medium'
                    : 'text-slate-500 hover:text-slate-900 dark:text-[#8F98A8] dark:hover:text-white'
                }`}
              >
                {ENGINE_LABELS[id]}
              </button>
            ))}
          </div>
        </div>

        {/* Швидкі закладки */}
        <div className="space-y-3">
          <div className="flex items-center justify-between text-[12px] text-slate-500 dark:text-[#8F98A8]">
            <span className="font-medium text-slate-700 dark:text-[#CBD5E1]">Швидкий доступ</span>
            <button
              type="button"
              onClick={() => setAddingShortcut((v) => !v)}
              className="flex items-center gap-1 hover:text-slate-900 dark:hover:text-white transition-colors"
            >
              <Plus className="w-3.5 h-3.5" />
              <span>Додати сайт</span>
            </button>
          </div>

          {addingShortcut && (
            <form
              onSubmit={async (e) => {
                e.preventDefault();
                if (!scUrl.trim()) return;
                await onSaveShortcut(scTitle.trim() || scUrl.trim(), scUrl.trim());
                setScTitle('');
                setScUrl('');
                setAddingShortcut(false);
              }}
              className="flex items-center gap-2 p-3 rounded-xl bg-white dark:bg-[#151922] border border-slate-200 dark:border-white/[0.08]"
            >
              <input
                type="text"
                value={scTitle}
                onChange={(e) => setScTitle(e.target.value)}
                placeholder="Назва"
                className="w-40 rounded-lg bg-slate-50 dark:bg-[#0B0D11] border border-slate-200 dark:border-white/[0.08] px-2.5 py-1.5 text-[12px] text-slate-900 dark:text-white"
              />
              <input
                type="text"
                value={scUrl}
                onChange={(e) => setScUrl(e.target.value)}
                placeholder="https://example.com"
                className="flex-1 rounded-lg bg-slate-50 dark:bg-[#0B0D11] border border-slate-200 dark:border-white/[0.08] px-2.5 py-1.5 text-[12px] font-mono text-slate-900 dark:text-white"
              />
              <button
                type="submit"
                className="px-3 py-1.5 rounded-lg bg-blue-600 hover:bg-blue-500 text-white text-[12px] font-medium"
              >
                Додати
              </button>
            </form>
          )}

          <div className="grid grid-cols-2 sm:grid-cols-4 gap-3">
            {quickLinks.map((bm) => {
              let host = bm.url;
              try {
                host = new URL(bm.url).hostname.replace(/^www\./, '');
              } catch {
                // keep raw
              }
              return (
                <button
                  key={bm.id}
                  type="button"
                  onClick={() => onNavigate(bm.url)}
                  className="flex items-center gap-3 p-3 rounded-xl bg-white hover:bg-slate-50 dark:bg-[#12161E] dark:hover:bg-[#181D27] border border-slate-200 dark:border-white/[0.07] transition-colors text-left"
                >
                  <div className="flex items-center justify-center w-8 h-8 rounded-lg bg-slate-100 dark:bg-[#1A202C] text-slate-500 dark:text-[#94A3B8] shrink-0">
                    <Globe className="w-4 h-4" />
                  </div>
                  <div className="min-w-0">
                    <div className="truncate text-[13px] font-medium text-slate-900 dark:text-[#E6EAF2]">
                      {bm.title}
                    </div>
                    <div className="truncate text-[11px] font-mono text-slate-400 dark:text-[#64748B]">
                      {host}
                    </div>
                  </div>
                </button>
              );
            })}
          </div>
        </div>

        {/* Нижній рядок стану та швидких розділів */}
        <div className="flex flex-wrap items-center justify-between gap-4 pt-6 border-t border-slate-200 dark:border-white/[0.07] text-[12px] text-slate-500 dark:text-[#8F98A8]">
          <div className="flex items-center gap-4 tabular-nums">
            <span>
              Заблоковано трекерів і реклами:{' '}
              <strong className="text-slate-900 dark:text-white font-mono">
                {adblockStatus?.totalBlocked ?? 0}
              </strong>
            </span>
            <span>·</span>
            <span>
              Вкладок:{' '}
              <strong className="text-slate-900 dark:text-white font-mono">{totalTabsCount}</strong>
              {sleepingTabsCount > 0 ? ` (${sleepingTabsCount} у режимі сну)` : ''}
            </span>
          </div>
          <div className="flex items-center gap-4">
            <button
              type="button"
              onClick={() => onOpenSidePanel('passwords')}
              className="flex items-center gap-1.5 hover:text-slate-900 dark:hover:text-white transition-colors"
            >
              <KeyRound className="w-3.5 h-3.5" />
              <span>Паролі</span>
            </button>
            <button
              type="button"
              onClick={() => onNavigate('vortex://benchmarks')}
              className="flex items-center gap-1.5 hover:text-slate-900 dark:hover:text-white transition-colors"
            >
              <Activity className="w-3.5 h-3.5" />
              <span>Тестування</span>
            </button>
            <button
              type="button"
              onClick={() => onNavigate('vortex://settings')}
              className="flex items-center gap-1.5 hover:text-slate-900 dark:hover:text-white transition-colors"
            >
              <Settings className="w-3.5 h-3.5" />
              <span>Налаштування</span>
            </button>
          </div>
        </div>
      </div>
    </div>
  );
};
