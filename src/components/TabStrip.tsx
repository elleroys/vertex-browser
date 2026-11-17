import React, { useEffect, useState } from 'react';
import {
  Copy,
  EyeOff,
  Globe,
  Moon,
  Pin,
  PinOff,
  Plus,
  RotateCcw,
  RotateCw,
  Volume2,
  VolumeX,
  X,
} from 'lucide-react';
import type { BrowserTab } from '../types/browser';

interface TabStripProps {
  tabs: BrowserTab[];
  activeTabId: string;
  canReopenClosed: boolean;
  onSelectTab: (id: string) => void;
  onCreateTab: (url?: string, incognito?: boolean) => void;
  onCloseTab: (id: string) => void;
  onDuplicateTab: (id: string) => void;
  onTogglePin: (id: string) => void;
  onToggleMute: (id: string) => void;
  onSleepTab: (id: string) => void;
  onReloadTab: (id: string) => void;
  onCloseOtherTabs: (id: string) => void;
  onReopenClosedTab: () => void;
  onOverlayVisibilityChange?: (open: boolean) => void;
}

export const TabStrip: React.FC<TabStripProps> = ({
  tabs,
  activeTabId,
  canReopenClosed,
  onSelectTab,
  onCreateTab,
  onCloseTab,
  onDuplicateTab,
  onTogglePin,
  onToggleMute,
  onSleepTab,
  onReloadTab,
  onCloseOtherTabs,
  onReopenClosedTab,
  onOverlayVisibilityChange,
}) => {
  const [contextMenu, setContextMenu] = useState<{
    tabId: string;
    x: number;
    y: number;
  } | null>(null);

  useEffect(() => {
    onOverlayVisibilityChange?.(contextMenu !== null);
  }, [contextMenu, onOverlayVisibilityChange]);

  useEffect(() => {
    if (!contextMenu) return;
    const handleClose = () => setContextMenu(null);
    window.addEventListener('click', handleClose);
    window.addEventListener('blur', handleClose);
    return () => {
      window.removeEventListener('click', handleClose);
      window.removeEventListener('blur', handleClose);
    };
  }, [contextMenu]);

  const contextTab = contextMenu ? tabs.find((t) => t.id === contextMenu.tabId) : null;

  return (
    <div className="relative flex h-[38px] w-full items-center gap-1.5 bg-[#E6EAF2] dark:bg-[#090B0F] px-2.5 select-none border-b border-slate-200 dark:border-white/[0.06]">
      {/* Логотип Vortex */}
      <button
        type="button"
        onClick={() => onCreateTab('vortex://newtab', false)}
        title="Vortex Browser — Нова вкладка (Ctrl+T)"
        className="flex items-center gap-2 pr-2 pl-1 py-1 rounded-md text-slate-900 dark:text-[#E6EAF2] hover:bg-slate-900/[0.06] dark:hover:bg-white/[0.06] transition-colors shrink-0 focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-blue-500"
      >
        <svg className="w-4 h-4 text-blue-600 dark:text-blue-400 shrink-0" viewBox="0 0 24 24" fill="none">
          <path
            d="M12 3C7.03 3 3 7.03 3 12c0 3.2 1.67 6.01 4.19 7.6-.66-1.78-.52-3.84.56-5.58 1.48-2.38 4.46-3.44 7.12-2.66 1.54.45 2.81 1.45 3.61 2.75A8.96 8.96 0 0 0 21 12c0-4.97-4.03-9-9-9Z"
            fill="currentColor"
            opacity="0.9"
          />
          <circle cx="12" cy="12" r="2.75" className="fill-[#E6EAF2] dark:fill-[#090B0F]" />
        </svg>
        <span className="text-[12px] font-semibold tracking-tight whitespace-nowrap">Vortex</span>
      </button>

      {/* Смуга вкладок */}
      <div className="flex flex-1 items-center gap-1 overflow-x-auto no-scrollbar min-w-0">
        {tabs.map((tab) => {
          const isActive = tab.id === activeTabId;
          const isSleeping = tab.isSuspended || tab.isDiscarded;

          return (
            <div
              key={tab.id}
              role="tab"
              aria-selected={isActive}
              tabIndex={0}
              onClick={() => onSelectTab(tab.id)}
              onContextMenu={(e) => {
                e.preventDefault();
                setContextMenu({ tabId: tab.id, x: e.clientX, y: e.clientY });
              }}
              onKeyDown={(e) => {
                if (e.key === 'Enter' || e.key === ' ') {
                  e.preventDefault();
                  onSelectTab(tab.id);
                }
              }}
              title={`${tab.title}${isSleeping ? ' · Спить (Memory Saver)' : ''}`}
              className={`group relative flex items-center gap-2 h-[30px] rounded-lg px-2.5 text-[12px] transition-colors cursor-pointer shrink-0 ${
                tab.isPinned ? 'max-w-[44px] px-2 justify-center' : 'min-w-[130px] max-w-[220px] flex-1'
              } ${
                isActive
                  ? 'bg-white text-slate-900 border border-slate-200 shadow-2xs dark:bg-[#151922] dark:text-[#F1F5F9] dark:border-white/[0.09] dark:shadow-none'
                  : 'bg-transparent text-slate-600 hover:bg-slate-900/[0.05] hover:text-slate-900 dark:text-[#8F98A8] dark:hover:bg-white/[0.045] dark:hover:text-[#CBD5E1] border border-transparent'
              }`}
            >
              {/* Favicon / Іконка стану */}
              <div className="relative flex items-center justify-center w-4 h-4 shrink-0">
                {tab.isLoading ? (
                  <span className="w-3.5 h-3.5 rounded-full border-[1.5px] border-blue-500/30 border-t-blue-500 dark:border-blue-400/30 dark:border-t-blue-400 animate-spin" />
                ) : tab.isIncognito ? (
                  <EyeOff className="w-3.5 h-3.5 text-amber-600 dark:text-amber-400" />
                ) : isSleeping ? (
                  <Moon className="w-3.5 h-3.5 text-slate-400" />
                ) : tab.favicon ? (
                  <img
                    src={tab.favicon}
                    alt=""
                    referrerPolicy="no-referrer"
                    className="w-3.5 h-3.5 rounded-xs object-contain"
                    onError={(e) => {
                      (e.currentTarget as HTMLImageElement).style.display = 'none';
                    }}
                  />
                ) : (
                  <Globe className="w-3.5 h-3.5 text-slate-400 dark:text-[#64748B]" />
                )}
              </div>

              {!tab.isPinned && (
                <span
                  className={`truncate flex-1 text-left font-medium ${
                    isSleeping ? 'opacity-65' : ''
                  }`}
                >
                  {tab.title || 'Нова вкладка'}
                </span>
              )}

              {/* Індикатор звуку */}
              {!tab.isPinned && tab.isPlayingAudio && (
                <button
                  type="button"
                  onClick={(e) => {
                    e.stopPropagation();
                    onToggleMute(tab.id);
                  }}
                  title={tab.isMuted ? 'Увімкнути звук' : 'Вимкнути звук'}
                  className="p-0.5 rounded hover:bg-slate-900/10 dark:hover:bg-white/10 text-blue-600 dark:text-blue-400 shrink-0"
                >
                  {tab.isMuted ? <VolumeX className="w-3 h-3" /> : <Volume2 className="w-3 h-3" />}
                </button>
              )}

              {/* Кнопка закриття вкладки */}
              {!tab.isPinned && (
                <button
                  type="button"
                  onClick={(e) => {
                    e.stopPropagation();
                    onCloseTab(tab.id);
                  }}
                  title="Закрити вкладку (Ctrl+W)"
                  aria-label="Закрити вкладку"
                  className={`p-0.5 rounded hover:bg-slate-900/10 dark:hover:bg-white/10 transition-opacity shrink-0 ${
                    isActive ? 'opacity-80 hover:opacity-100' : 'opacity-0 group-hover:opacity-80'
                  }`}
                >
                  <X className="w-3 h-3" />
                </button>
              )}
            </div>
          );
        })}

        {/* Нова вкладка */}
        <button
          type="button"
          onClick={() => onCreateTab('vortex://newtab', false)}
          title="Нова вкладка (Ctrl+T)"
          aria-label="Нова вкладка"
          className="flex items-center justify-center w-7 h-7 rounded-md text-slate-600 hover:text-slate-900 hover:bg-slate-900/[0.07] dark:text-[#8F98A8] dark:hover:text-white dark:hover:bg-white/[0.07] transition-colors shrink-0"
        >
          <Plus className="w-4 h-4" />
        </button>
      </div>

      {/* Праві швидкі дії панелі вкладок */}
      <div className="flex items-center gap-1 shrink-0 pl-1 border-l border-slate-300 dark:border-white/[0.06]">
        {canReopenClosed && (
          <button
            type="button"
            onClick={onReopenClosedTab}
            title="Відновити закриту вкладку"
            className="flex items-center gap-1 px-2 py-1 rounded-md text-[11px] text-slate-600 hover:text-slate-900 hover:bg-slate-900/[0.06] dark:text-[#8F98A8] dark:hover:text-white dark:hover:bg-white/[0.06] transition-colors whitespace-nowrap shrink-0"
          >
            <RotateCcw className="w-3.5 h-3.5" />
          </button>
        )}
        <button
          type="button"
          onClick={() => onCreateTab('vortex://newtab', true)}
          title="Нова приватна вкладка (без збереження історії)"
          className="flex items-center gap-1.5 px-2.5 py-1 rounded-md text-[11px] font-medium text-slate-600 hover:text-amber-600 hover:bg-slate-900/[0.06] dark:text-[#94A3B8] dark:hover:text-amber-300 dark:hover:bg-white/[0.06] transition-colors whitespace-nowrap shrink-0"
        >
          <EyeOff className="w-3.5 h-3.5" />
          <span>Приватно</span>
        </button>
      </div>

      {/* Контекстне меню вкладки */}
      {contextMenu && contextTab && (
        <div
          style={{ left: Math.min(contextMenu.x, window.innerWidth - 230), top: contextMenu.y + 4 }}
          className="fixed z-50 w-56 rounded-lg bg-white dark:bg-[#151922] border border-slate-200 dark:border-white/[0.1] shadow-2xl py-1.5 text-[12px] text-slate-800 dark:text-[#E6EAF2]"
          onClick={(e) => e.stopPropagation()}
        >
          <button
            type="button"
            onClick={() => {
              onReloadTab(contextTab.id);
              setContextMenu(null);
            }}
            className="flex w-full items-center gap-2.5 px-3 py-1.5 hover:bg-slate-100 dark:hover:bg-white/[0.07] text-left"
          >
            <RotateCw className="w-3.5 h-3.5 text-slate-500 dark:text-[#94A3B8]" />
            <span>Оновити вкладку</span>
          </button>
          <button
            type="button"
            onClick={() => {
              onDuplicateTab(contextTab.id);
              setContextMenu(null);
            }}
            className="flex w-full items-center gap-2.5 px-3 py-1.5 hover:bg-slate-100 dark:hover:bg-white/[0.07] text-left"
          >
            <Copy className="w-3.5 h-3.5 text-slate-500 dark:text-[#94A3B8]" />
            <span>Дублювати вкладку</span>
          </button>
          <button
            type="button"
            onClick={() => {
              onTogglePin(contextTab.id);
              setContextMenu(null);
            }}
            className="flex w-full items-center gap-2.5 px-3 py-1.5 hover:bg-slate-100 dark:hover:bg-white/[0.07] text-left"
          >
            {contextTab.isPinned ? (
              <>
                <PinOff className="w-3.5 h-3.5 text-slate-500 dark:text-[#94A3B8]" />
                <span>Відкріпити вкладку</span>
              </>
            ) : (
              <>
                <Pin className="w-3.5 h-3.5 text-slate-500 dark:text-[#94A3B8]" />
                <span>Закріпити вкладку</span>
              </>
            )}
          </button>
          <button
            type="button"
            onClick={() => {
              onToggleMute(contextTab.id);
              setContextMenu(null);
            }}
            className="flex w-full items-center gap-2.5 px-3 py-1.5 hover:bg-slate-100 dark:hover:bg-white/[0.07] text-left"
          >
            {contextTab.isMuted ? (
              <>
                <Volume2 className="w-3.5 h-3.5 text-slate-500 dark:text-[#94A3B8]" />
                <span>Увімкнути звук</span>
              </>
            ) : (
              <>
                <VolumeX className="w-3.5 h-3.5 text-slate-500 dark:text-[#94A3B8]" />
                <span>Вимкнути звук</span>
              </>
            )}
          </button>
          {contextTab.id !== activeTabId && (
            <button
              type="button"
              onClick={() => {
                onSleepTab(contextTab.id);
                setContextMenu(null);
              }}
              className="flex w-full items-center gap-2.5 px-3 py-1.5 hover:bg-slate-100 dark:hover:bg-white/[0.07] text-left"
            >
              <Moon className="w-3.5 h-3.5 text-blue-600 dark:text-blue-400" />
              <span>Приспати (Memory Saver)</span>
            </button>
          )}
          <div className="my-1 border-t border-slate-200 dark:border-white/[0.07]" />
          <button
            type="button"
            onClick={() => {
              onCloseOtherTabs(contextTab.id);
              setContextMenu(null);
            }}
            className="flex w-full items-center gap-2.5 px-3 py-1.5 hover:bg-slate-100 dark:hover:bg-white/[0.07] text-left"
          >
            <X className="w-3.5 h-3.5 text-slate-500 dark:text-[#94A3B8]" />
            <span>Закрити інші вкладки</span>
          </button>
          <button
            type="button"
            onClick={() => {
              onCloseTab(contextTab.id);
              setContextMenu(null);
            }}
            className="flex w-full items-center gap-2.5 px-3 py-1.5 hover:bg-red-500/15 text-red-600 dark:text-red-400 text-left"
          >
            <X className="w-3.5 h-3.5" />
            <span>Закрити вкладку</span>
          </button>
        </div>
      )}
    </div>
  );
};
