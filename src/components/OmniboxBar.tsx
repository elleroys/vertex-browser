import React, { useEffect, useMemo, useRef, useState } from 'react';
import {
  Activity,
  ArrowLeft,
  ArrowRight,
  Bookmark,
  Download,
  History,
  Home,
  KeyRound,
  Lock,
  LockOpen,
  PanelRight,
  RefreshCw,
  RotateCw,
  Search,
  Settings,
  Shield,
  ShieldAlert,
  ShieldCheck,
  Star,
  X,
} from 'lucide-react';
import type {
  AdBlockStatus,
  BookmarkFolder,
  BookmarkItem,
  BrowserSettings,
  BrowserTab,
  HistoryItem,
  SidePanelSection,
} from '../types/browser';

interface OmniboxBarProps {
  activeTab: BrowserTab;
  settings: BrowserSettings;
  adblockStatus: AdBlockStatus | null;
  bookmarks: BookmarkItem[];
  bookmarkFolders: BookmarkFolder[];
  history: HistoryItem[];
  activeDownloadsCount: number;
  sidePanelOpen: boolean;
  sidePanelSection: SidePanelSection;
  vaultUnlocked: boolean;
  hasMatchingPasswords: boolean;
  omniboxFocusSignal: number;
  onNavigate: (urlOrQuery: string) => void;
  onBack: () => void;
  onForward: () => void;
  onReloadOrStop: () => void;
  onHome: () => void;
  onToggleAdBlockDomain: (domain: string, disableOnDomain: boolean) => Promise<void>;
  onUpdateAdBlockFilters: () => Promise<void>;
  onToggleBookmark: (title: string, url: string, folderId: string | null, onBar: boolean) => Promise<void>;
  onDeleteBookmark: (id: string) => Promise<void>;
  onToggleSidePanel: (section?: SidePanelSection) => void;
  onOpenInternalPage: (url: string) => void;
  onOverlayVisibilityChange: (open: boolean) => void;
}

export const OmniboxBar: React.FC<OmniboxBarProps> = ({
  activeTab,
  settings,
  adblockStatus,
  bookmarks,
  bookmarkFolders,
  history,
  activeDownloadsCount,
  sidePanelOpen,
  sidePanelSection,
  vaultUnlocked,
  hasMatchingPasswords,
  omniboxFocusSignal,
  onNavigate,
  onBack,
  onForward,
  onReloadOrStop,
  onHome,
  onToggleAdBlockDomain,
  onUpdateAdBlockFilters,
  onToggleBookmark,
  onDeleteBookmark,
  onToggleSidePanel,
  onOpenInternalPage,
  onOverlayVisibilityChange,
}) => {
  const [inputVal, setInputVal] = useState(activeTab.url);
  const [isFocused, setIsFocused] = useState(false);
  const [selectedSuggestionIdx, setSelectedSuggestionIdx] = useState(-1);
  const [showAdblockMenu, setShowAdblockMenu] = useState(false);
  const [showBookmarkPopover, setShowBookmarkPopover] = useState(false);
  const [updatingFilters, setUpdatingFilters] = useState(false);

  const existingBookmark = useMemo(
    () => bookmarks.find((b) => b.url === activeTab.url),
    [bookmarks, activeTab.url]
  );

  const [bmTitle, setBmTitle] = useState(activeTab.title);
  const [bmFolderId, setBmFolderId] = useState<string>('quick-bar');
  const [bmOnBar, setBmOnBar] = useState(true);

  const inputRef = useRef<HTMLInputElement>(null);
  const barRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!isFocused) {
      setInputVal(activeTab.url === 'vortex://newtab' ? '' : activeTab.url);
    }
  }, [activeTab.id, activeTab.url, isFocused]);

  useEffect(() => {
    if (omniboxFocusSignal > 0) {
      inputRef.current?.focus();
      inputRef.current?.select();
    }
  }, [omniboxFocusSignal]);

  useEffect(() => {
    onOverlayVisibilityChange(
      showAdblockMenu || showBookmarkPopover || (isFocused && inputVal.trim().length > 0)
    );
  }, [showAdblockMenu, showBookmarkPopover, isFocused, inputVal, onOverlayVisibilityChange]);

  useEffect(() => {
    const closeMenus = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        setShowAdblockMenu(false);
        setShowBookmarkPopover(false);
      }
    };
    const handleOutsideMouseDown = (e: MouseEvent) => {
      if (barRef.current && !barRef.current.contains(e.target as Node)) {
        setShowAdblockMenu(false);
        setShowBookmarkPopover(false);
      }
    };
    window.addEventListener('keydown', closeMenus);
    window.addEventListener('mousedown', handleOutsideMouseDown);
    return () => {
      window.removeEventListener('keydown', closeMenus);
      window.removeEventListener('mousedown', handleOutsideMouseDown);
    };
  }, []);

  const suggestions = useMemo(() => {
    const q = inputVal.trim().toLowerCase();
    if (!q || !isFocused) return [];

    const bmMatches = bookmarks
      .filter((b) => b.title.toLowerCase().includes(q) || b.url.toLowerCase().includes(q))
      .slice(0, 3)
      .map((b) => ({ kind: 'bookmark' as const, title: b.title, url: b.url }));

    const histMatches = history
      .filter(
        (h) =>
          (h.title.toLowerCase().includes(q) || h.url.toLowerCase().includes(q)) &&
          !bmMatches.some((b) => b.url === h.url)
      )
      .slice(0, 4)
      .map((h) => ({ kind: 'history' as const, title: h.title, url: h.url }));

    const engineLabel: Record<string, string> = {
      google: 'Google',
      duckduckgo: 'DuckDuckGo',
      brave: 'Brave Search',
      bing: 'Bing',
    };

    return [
      {
        kind: 'search' as const,
        title: `Пошук у ${engineLabel[settings.searchEngine] || 'Google'}: «${inputVal.trim()}»`,
        url: inputVal.trim(),
      },
      ...bmMatches,
      ...histMatches,
    ];
  }, [inputVal, isFocused, bookmarks, history, settings.searchEngine]);

  const canGoBack = activeTab.historyIndex > 0;
  const canGoForward = activeTab.historyIndex < activeTab.historyStack.length - 1;
  const isInternal = activeTab.url.startsWith('vortex://');
  const currentDomain = useMemo(() => {
    try {
      if (isInternal) return '';
      return new URL(activeTab.url).hostname.replace(/^www\./, '');
    } catch {
      return '';
    }
  }, [activeTab.url, isInternal]);

  const adblockActiveHere = adblockStatus ? adblockStatus.isActiveOnSite : settings.adblockEnabled;

  const handleOpenBookmarkPopover = async () => {
    if (isInternal) return;
    if (!existingBookmark) {
      await onToggleBookmark(activeTab.title || activeTab.url, activeTab.url, 'quick-bar', true);
      setBmTitle(activeTab.title || activeTab.url);
      setBmFolderId('quick-bar');
      setBmOnBar(true);
    } else {
      setBmTitle(existingBookmark.title);
      setBmFolderId(existingBookmark.folderId || 'quick-bar');
      setBmOnBar(existingBookmark.onBar);
    }
    setShowAdblockMenu(false);
    setShowBookmarkPopover((v) => !v);
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (selectedSuggestionIdx >= 0 && suggestions[selectedSuggestionIdx]) {
      onNavigate(suggestions[selectedSuggestionIdx].url);
    } else if (inputVal.trim()) {
      onNavigate(inputVal.trim());
    }
    setIsFocused(false);
    inputRef.current?.blur();
  };

  return (
    <div
      ref={barRef}
      className="relative flex h-[44px] w-full items-center gap-2 bg-white dark:bg-[#0F1218] px-2.5 border-b border-slate-200 dark:border-white/[0.07]"
    >
      {/* Кнопки навігації */}
      <div className="flex items-center gap-0.5 shrink-0">
        <button
          type="button"
          onClick={onBack}
          disabled={!canGoBack}
          title="Назад (Alt+←)"
          className="flex items-center justify-center w-8 h-8 rounded-lg text-slate-700 hover:bg-slate-900/[0.06] dark:text-[#CBD5E1] dark:hover:bg-white/[0.07] disabled:opacity-30 disabled:pointer-events-none transition-colors"
        >
          <ArrowLeft className="w-4 h-4" />
        </button>
        <button
          type="button"
          onClick={onForward}
          disabled={!canGoForward}
          title="Вперед (Alt+→)"
          className="flex items-center justify-center w-8 h-8 rounded-lg text-slate-700 hover:bg-slate-900/[0.06] dark:text-[#CBD5E1] dark:hover:bg-white/[0.07] disabled:opacity-30 disabled:pointer-events-none transition-colors"
        >
          <ArrowRight className="w-4 h-4" />
        </button>
        <button
          type="button"
          onClick={onReloadOrStop}
          title={activeTab.isLoading ? 'Зупинити завантаження (Esc)' : 'Оновити сторінку (F5)'}
          className="flex items-center justify-center w-8 h-8 rounded-lg text-slate-700 hover:bg-slate-900/[0.06] dark:text-[#CBD5E1] dark:hover:bg-white/[0.07] transition-colors"
        >
          {activeTab.isLoading ? <X className="w-4 h-4" /> : <RotateCw className="w-4 h-4" />}
        </button>
        <button
          type="button"
          onClick={onHome}
          title="Стартова сторінка"
          className="flex items-center justify-center w-8 h-8 rounded-lg text-slate-700 hover:bg-slate-900/[0.06] dark:text-[#CBD5E1] dark:hover:bg-white/[0.07] transition-colors"
        >
          <Home className="w-4 h-4" />
        </button>
      </div>

      {/* Адресний рядок (Omnibox) */}
      <div className="relative flex-1 min-w-0">
        <form
          onSubmit={handleSubmit}
          className={`flex items-center h-[32px] w-full rounded-lg bg-[#F1F5F9] dark:bg-[#151922] px-2.5 border transition-colors ${
            isFocused
              ? 'border-blue-500/70 bg-white dark:bg-[#181D27]'
              : 'border-slate-200 hover:border-slate-300 dark:border-white/[0.08] dark:hover:border-white/[0.15]'
          }`}
        >
          <div className="flex items-center mr-2 shrink-0">
            {isInternal ? (
              <Search className="w-3.5 h-3.5 text-slate-500 dark:text-[#8F98A8]" />
            ) : activeTab.isHttps ? (
              <span title="Захищене HTTPS-з’єднання" className="flex items-center text-emerald-600 dark:text-emerald-400">
                <Lock className="w-3.5 h-3.5" />
              </span>
            ) : (
              <span title="Незахищене HTTP-з’єднання" className="flex items-center text-amber-600 dark:text-amber-400">
                <LockOpen className="w-3.5 h-3.5" />
              </span>
            )}
          </div>

          <input
            ref={inputRef}
            type="text"
            value={inputVal}
            onFocus={() => {
              setIsFocused(true);
              setSelectedSuggestionIdx(-1);
              setTimeout(() => inputRef.current?.select(), 0);
            }}
            onBlur={() => {
              setTimeout(() => setIsFocused(false), 160);
            }}
            onChange={(e) => {
              setInputVal(e.target.value);
              setSelectedSuggestionIdx(-1);
            }}
            onKeyDown={(e) => {
              if (e.key === 'ArrowDown' && suggestions.length > 0) {
                e.preventDefault();
                setSelectedSuggestionIdx((prev) => (prev + 1) % suggestions.length);
              } else if (e.key === 'ArrowUp' && suggestions.length > 0) {
                e.preventDefault();
                setSelectedSuggestionIdx((prev) => (prev <= 0 ? suggestions.length - 1 : prev - 1));
              } else if (e.key === 'Escape') {
                setIsFocused(false);
                inputRef.current?.blur();
              }
            }}
            placeholder="Введіть URL або пошуковий запит (Ctrl+L)..."
            aria-label="Адресний рядок"
            className="flex-1 min-w-0 bg-transparent text-[13px] font-mono text-slate-900 dark:text-[#E6EAF2] placeholder:font-sans placeholder:text-slate-400 dark:placeholder:text-[#64748B] focus:outline-none"
          />

          {!isInternal && (
            <button
              type="button"
              onClick={handleOpenBookmarkPopover}
              title={existingBookmark ? 'Редагувати закладку' : 'Додати до закладок'}
              className={`flex items-center justify-center w-6 h-6 rounded-md transition-colors shrink-0 ${
                existingBookmark
                  ? 'text-blue-600 dark:text-blue-400 hover:bg-slate-900/[0.06] dark:hover:bg-white/[0.08]'
                  : 'text-slate-500 hover:text-slate-900 hover:bg-slate-900/[0.06] dark:text-[#8F98A8] dark:hover:text-white dark:hover:bg-white/[0.08]'
              }`}
            >
              <Star className="w-3.5 h-3.5" fill={existingBookmark ? 'currentColor' : 'none'} />
            </button>
          )}
        </form>

        {/* Підказки адресного рядка */}
        {isFocused && suggestions.length > 0 && (
          <div className="absolute left-0 right-0 top-[36px] z-50 rounded-lg bg-white dark:bg-[#151922] border border-slate-200 dark:border-white/[0.1] shadow-2xl py-1 overflow-hidden">
            {suggestions.map((s, idx) => (
              <button
                key={`${s.kind}-${s.url}-${idx}`}
                type="button"
                onMouseDown={(e) => {
                  e.preventDefault();
                  onNavigate(s.url);
                  setIsFocused(false);
                }}
                className={`flex w-full items-center gap-3 px-3 py-2 text-left text-[13px] transition-colors ${
                  idx === selectedSuggestionIdx
                    ? 'bg-blue-500/15 text-blue-700 dark:bg-blue-500/20 dark:text-white'
                    : 'hover:bg-slate-100 text-slate-700 dark:hover:bg-white/[0.06] dark:text-[#CBD5E1]'
                }`}
              >
                {s.kind === 'search' ? (
                  <Search className="w-3.5 h-3.5 text-blue-600 dark:text-blue-400 shrink-0" />
                ) : s.kind === 'bookmark' ? (
                  <Bookmark className="w-3.5 h-3.5 text-amber-600 dark:text-amber-400 shrink-0" />
                ) : (
                  <History className="w-3.5 h-3.5 text-slate-400 dark:text-[#8F98A8] shrink-0" />
                )}
                <span className="truncate font-medium">{s.title}</span>
                {s.kind !== 'search' && (
                  <span className="ml-auto truncate max-w-[40%] font-mono text-[11px] text-slate-400 dark:text-[#64748B]">
                    {s.url}
                  </span>
                )}
              </button>
            ))}
          </div>
        )}

        {/* Popover редагування закладки */}
        {showBookmarkPopover && (
          <div className="absolute right-0 top-[38px] z-50 w-80 rounded-xl bg-white dark:bg-[#151922] border border-slate-200 dark:border-white/[0.1] shadow-2xl p-4 text-[13px]">
            <div className="flex items-center justify-between mb-3">
              <span className="font-semibold text-slate-900 dark:text-white">Закладку збережено</span>
              <button
                type="button"
                onClick={() => setShowBookmarkPopover(false)}
                className="text-slate-400 hover:text-slate-900 dark:text-[#8F98A8] dark:hover:text-white"
              >
                <X className="w-4 h-4" />
              </button>
            </div>
            <div className="space-y-3">
              <div>
                <label className="block text-[11px] text-slate-500 dark:text-[#8F98A8] mb-1">Назва</label>
                <input
                  type="text"
                  value={bmTitle}
                  onChange={(e) => setBmTitle(e.target.value)}
                  className="w-full rounded-lg bg-slate-50 dark:bg-[#0F1218] border border-slate-200 dark:border-white/[0.1] px-2.5 py-1.5 text-[13px] text-slate-900 dark:text-white focus:outline-none focus:border-blue-500"
                />
              </div>
              <div>
                <label className="block text-[11px] text-slate-500 dark:text-[#8F98A8] mb-1">Папка</label>
                <select
                  value={bmFolderId}
                  onChange={(e) => setBmFolderId(e.target.value)}
                  className="w-full rounded-lg bg-slate-50 dark:bg-[#0F1218] border border-slate-200 dark:border-white/[0.1] px-2.5 py-1.5 text-[13px] text-slate-900 dark:text-white focus:outline-none focus:border-blue-500"
                >
                  {bookmarkFolders.map((f) => (
                    <option key={f.id} value={f.id}>
                      {f.name}
                    </option>
                  ))}
                </select>
              </div>
              <label className="flex items-center gap-2 text-[12px] text-slate-700 dark:text-[#CBD5E1] cursor-pointer">
                <input
                  type="checkbox"
                  checked={bmOnBar}
                  onChange={(e) => setBmOnBar(e.target.checked)}
                  className="rounded border-slate-300 dark:border-white/20 bg-white dark:bg-[#0F1218] text-blue-500"
                />
                <span>Показувати на панелі швидкого доступу</span>
              </label>
              <div className="flex items-center justify-between pt-2 border-t border-slate-200 dark:border-white/[0.07]">
                {existingBookmark ? (
                  <button
                    type="button"
                    onClick={async () => {
                      await onDeleteBookmark(existingBookmark.id);
                      setShowBookmarkPopover(false);
                    }}
                    className="px-3 py-1.5 rounded-lg text-[12px] text-red-600 dark:text-red-400 hover:bg-red-500/10"
                  >
                    Видалити
                  </button>
                ) : (
                  <span />
                )}
                <button
                  type="button"
                  onClick={async () => {
                    await onToggleBookmark(bmTitle, activeTab.url, bmFolderId, bmOnBar);
                    setShowBookmarkPopover(false);
                  }}
                  className="px-4 py-1.5 rounded-lg bg-blue-600 hover:bg-blue-500 text-white text-[12px] font-medium"
                >
                  Зберегти
                </button>
              </div>
            </div>
          </div>
        )}
      </div>

      {/* Зручний перемикач AdBlock біля адресного рядка */}
      <div className="relative shrink-0">
        <button
          type="button"
          onClick={() => {
            setShowBookmarkPopover(false);
            setShowAdblockMenu((v) => !v);
          }}
          title={
            isInternal
              ? 'AdBlock налаштування'
              : adblockActiveHere
                ? `AdBlock активний (${activeTab.blockedCount} заблоковано)`
                : `AdBlock вимкнено для ${currentDomain}`
          }
          className={`flex items-center gap-1.5 h-[32px] px-2.5 rounded-lg border text-[12px] font-medium transition-colors whitespace-nowrap shrink-0 ${
            !settings.adblockEnabled || (currentDomain && !adblockActiveHere)
              ? 'bg-amber-50 border-amber-300 text-amber-700 hover:bg-amber-100 dark:bg-[#151922] dark:border-amber-500/35 dark:text-amber-300 dark:hover:bg-[#1B212D]'
              : 'bg-slate-100 border-slate-200 text-slate-800 hover:bg-slate-200/70 dark:bg-[#151922] dark:border-white/[0.08] dark:text-[#E6EAF2] dark:hover:bg-[#1B212D]'
          }`}
        >
          {adblockActiveHere ? (
            <ShieldCheck className="w-4 h-4 text-emerald-600 dark:text-emerald-400 shrink-0" />
          ) : (
            <ShieldAlert className="w-4 h-4 text-amber-600 dark:text-amber-400 shrink-0" />
          )}
          <span className="font-mono tabular-nums text-[12px]">
            {activeTab.blockedCount > 0 ? activeTab.blockedCount : 'AdBlock'}
          </span>
        </button>

        {showAdblockMenu && (
          <div className="absolute right-0 top-[38px] z-50 w-[330px] rounded-xl bg-white dark:bg-[#151922] border border-slate-200 dark:border-white/[0.1] shadow-2xl p-4 text-[13px]">
            <div className="flex items-start justify-between gap-3 pb-3 border-b border-slate-200 dark:border-white/[0.07]">
              <div className="flex items-center gap-2.5 min-w-0">
                {adblockActiveHere ? (
                  <ShieldCheck className="w-5 h-5 text-emerald-600 dark:text-emerald-400 shrink-0" />
                ) : (
                  <Shield className="w-5 h-5 text-amber-600 dark:text-amber-400 shrink-0" />
                )}
                <div className="min-w-0">
                  <div className="font-semibold text-slate-900 dark:text-white truncate">Vortex AdBlock</div>
                  <div className="text-[12px] text-slate-500 dark:text-[#8F98A8] truncate">
                    {currentDomain ? currentDomain : 'Внутрішня сторінка браузера'}
                  </div>
                </div>
              </div>
              <button
                type="button"
                onClick={() => setShowAdblockMenu(false)}
                className="text-slate-400 hover:text-slate-900 dark:text-[#8F98A8] dark:hover:text-white"
              >
                <X className="w-4 h-4" />
              </button>
            </div>

            {currentDomain ? (
              <div className="py-3.5 border-b border-slate-200 dark:border-white/[0.07]">
                <div className="flex items-center justify-between gap-3 mb-2">
                  <div>
                    <div className="font-medium text-slate-900 dark:text-white">
                      {adblockActiveHere ? 'Захист активний на сайті' : 'Сайт у винятках (Whitelist)'}
                    </div>
                    <div className="text-[11px] text-slate-500 dark:text-[#8F98A8]">
                      {adblockActiveHere
                        ? 'Реклама й трекери блокуються на рівні мережі'
                        : 'Реклама і всі запити на цьому домені дозволені'}
                    </div>
                  </div>
                  <button
                    type="button"
                    role="switch"
                    aria-checked={adblockActiveHere}
                    onClick={async () => {
                      await onToggleAdBlockDomain(currentDomain, adblockActiveHere);
                    }}
                    className={`relative inline-flex h-6 w-11 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ${
                      adblockActiveHere ? 'bg-emerald-500' : 'bg-slate-300 dark:bg-[#2A3242]'
                    }`}
                  >
                    <span
                      className={`pointer-events-none inline-block h-5 w-5 transform rounded-full bg-white shadow transition duration-200 ${
                        adblockActiveHere ? 'translate-x-5' : 'translate-x-0'
                      }`}
                    />
                  </button>
                </div>
                <button
                  type="button"
                  onClick={async () => {
                    await onToggleAdBlockDomain(currentDomain, adblockActiveHere);
                  }}
                  className={`mt-1 w-full py-2 px-3 rounded-lg text-[12px] font-medium transition-colors ${
                    adblockActiveHere
                      ? 'bg-amber-500/15 text-amber-700 hover:bg-amber-500/25 dark:text-amber-300'
                      : 'bg-emerald-500/15 text-emerald-700 hover:bg-emerald-500/25 dark:text-emerald-300'
                  }`}
                >
                  {adblockActiveHere
                    ? `Вимкнути AdBlock для ${currentDomain}`
                    : `Знову ввімкнути AdBlock для ${currentDomain}`}
                </button>
              </div>
            ) : (
              <div className="py-3 text-[12px] text-slate-500 dark:text-[#8F98A8] border-b border-slate-200 dark:border-white/[0.07]">
                Відкрийте зовнішній вебсайт, щоб керувати винятками для конкретного домену.
              </div>
            )}

            <div className="py-3 grid grid-cols-2 gap-3 text-[12px] border-b border-slate-200 dark:border-white/[0.07]">
              <div>
                <div className="text-slate-500 dark:text-[#8F98A8]">На цій вкладці</div>
                <div className="text-[16px] font-semibold font-mono tabular-nums text-slate-900 dark:text-white mt-0.5">
                  {activeTab.blockedCount}
                </div>
              </div>
              <div>
                <div className="text-slate-500 dark:text-[#8F98A8]">Загалом заблоковано</div>
                <div className="text-[16px] font-semibold font-mono tabular-nums text-slate-900 dark:text-white mt-0.5">
                  {adblockStatus?.totalBlocked ?? 0}
                </div>
              </div>
            </div>

            <div className="pt-3 flex items-center justify-between gap-2 text-[12px]">
              <span className="text-slate-500 dark:text-[#8F98A8] truncate">
                EasyList + EasyPrivacy ·{' '}
                <span className="font-mono tabular-nums">{adblockStatus?.rulesLoaded ?? 0}</span> правил
              </span>
              <button
                type="button"
                disabled={updatingFilters}
                onClick={async () => {
                  setUpdatingFilters(true);
                  try {
                    await onUpdateAdBlockFilters();
                  } finally {
                    setUpdatingFilters(false);
                  }
                }}
                className="flex items-center gap-1.5 text-blue-600 hover:text-blue-500 dark:text-blue-400 dark:hover:text-blue-300 whitespace-nowrap shrink-0"
              >
                <RefreshCw className={`w-3.5 h-3.5 ${updatingFilters ? 'animate-spin' : ''}`} />
                <span>{updatingFilters ? 'Оновлення...' : 'Оновити'}</span>
              </button>
            </div>
          </div>
        )}
      </div>

      {/* Інструменти праворуч */}
      <div className="flex items-center gap-1 shrink-0">
        <button
          type="button"
          onClick={() => onToggleSidePanel('passwords')}
          title={
            hasMatchingPasswords
              ? 'Знайдено збережений пароль для цього сайту'
              : vaultUnlocked
                ? 'Менеджер паролів (розблоковано)'
                : 'Менеджер паролів (заблоковано)'
          }
          className={`relative flex items-center justify-center w-8 h-8 rounded-lg transition-colors ${
            sidePanelOpen && sidePanelSection === 'passwords'
              ? 'bg-blue-500/15 text-blue-600 dark:bg-blue-500/20 dark:text-blue-400'
              : hasMatchingPasswords
                ? 'text-emerald-600 bg-emerald-500/10 hover:bg-emerald-500/20 dark:text-emerald-400'
                : 'text-slate-700 hover:bg-slate-900/[0.06] dark:text-[#CBD5E1] dark:hover:bg-white/[0.07]'
          }`}
        >
          <KeyRound className="w-4 h-4" />
        </button>

        <button
          type="button"
          onClick={() => onToggleSidePanel('downloads')}
          title="Менеджер завантажень"
          className={`relative flex items-center justify-center w-8 h-8 rounded-lg transition-colors ${
            sidePanelOpen && sidePanelSection === 'downloads'
              ? 'bg-blue-500/15 text-blue-600 dark:bg-blue-500/20 dark:text-blue-400'
              : 'text-slate-700 hover:bg-slate-900/[0.06] dark:text-[#CBD5E1] dark:hover:bg-white/[0.07]'
          }`}
        >
          <Download className="w-4 h-4" />
          {activeDownloadsCount > 0 && (
            <span className="absolute -top-0.5 -right-0.5 min-w-[15px] h-[15px] px-1 rounded-full bg-blue-500 text-[10px] font-mono font-semibold text-white flex items-center justify-center">
              {activeDownloadsCount}
            </span>
          )}
        </button>

        <button
          type="button"
          onClick={() => onToggleSidePanel()}
          title="Бічна панель (Закладки, Історія, Завантаження, Паролі)"
          className={`flex items-center justify-center w-8 h-8 rounded-lg transition-colors ${
            sidePanelOpen
              ? 'bg-blue-500/15 text-blue-600 dark:bg-blue-500/20 dark:text-blue-400'
              : 'text-slate-700 hover:bg-slate-900/[0.06] dark:text-[#CBD5E1] dark:hover:bg-white/[0.07]'
          }`}
        >
          <PanelRight className="w-4 h-4" />
        </button>

        <button
          type="button"
          onClick={() => onOpenInternalPage('vortex://benchmarks')}
          title="Тестування продуктивності: Vortex vs Chrome"
          className={`flex items-center justify-center w-8 h-8 rounded-lg transition-colors ${
            activeTab.url === 'vortex://benchmarks'
              ? 'bg-blue-500/15 text-blue-600 dark:bg-blue-500/20 dark:text-blue-400'
              : 'text-slate-700 hover:bg-slate-900/[0.06] dark:text-[#CBD5E1] dark:hover:bg-white/[0.07]'
          }`}
        >
          <Activity className="w-4 h-4" />
        </button>

        <button
          type="button"
          onClick={() => onOpenInternalPage('vortex://settings')}
          title="Налаштування Vortex Browser"
          className={`flex items-center justify-center w-8 h-8 rounded-lg transition-colors ${
            activeTab.url === 'vortex://settings'
              ? 'bg-blue-500/15 text-blue-600 dark:bg-blue-500/20 dark:text-blue-400'
              : 'text-slate-700 hover:bg-slate-900/[0.06] dark:text-[#CBD5E1] dark:hover:bg-white/[0.07]'
          }`}
        >
          <Settings className="w-4 h-4" />
        </button>
      </div>

      {/* Смужка прогресу завантаження сторінки */}
      {activeTab.isLoading && (
        <div className="absolute bottom-0 left-0 right-0 h-[2px] bg-blue-500/15 overflow-hidden">
          <div className="h-full w-1/2 bg-blue-500 animate-[pulse_1s_ease-in-out_infinite]" />
        </div>
      )}
    </div>
  );
};
