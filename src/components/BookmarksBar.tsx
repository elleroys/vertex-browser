import React, { useState } from 'react';
import { Folder, Globe } from 'lucide-react';
import type { BookmarkFolder, BookmarkItem } from '../types/browser';

interface BookmarksBarProps {
  folders: BookmarkFolder[];
  bookmarks: BookmarkItem[];
  onOpenUrl: (url: string, newTab?: boolean) => void;
  onOverlayVisibilityChange?: (open: boolean) => void;
}

export const BookmarksBar: React.FC<BookmarksBarProps> = ({
  folders,
  bookmarks,
  onOpenUrl,
  onOverlayVisibilityChange,
}) => {
  const [openFolderId, setOpenFolderId] = useState<string | null>(null);

  React.useEffect(() => {
    onOverlayVisibilityChange?.(openFolderId !== null);
  }, [openFolderId, onOverlayVisibilityChange]);

  const quickItems = bookmarks.filter((b) => b.onBar && (!b.folderId || b.folderId === 'quick-bar'));
  const secondaryFolders = folders.filter((f) => f.id !== 'quick-bar');

  return (
    <div className="relative flex h-[30px] w-full items-center gap-1 bg-[#F8FAFC] dark:bg-[#0D1016] px-3 border-b border-slate-200 dark:border-white/[0.06] overflow-x-auto no-scrollbar text-[12px]">
      {secondaryFolders.map((folder) => {
        const folderBookmarks = bookmarks.filter((b) => b.folderId === folder.id);
        const isOpen = openFolderId === folder.id;
        return (
          <div key={folder.id} className="relative shrink-0">
            <button
              type="button"
              onClick={() => setOpenFolderId(isOpen ? null : folder.id)}
              className="flex items-center gap-1.5 px-2 py-1 rounded-md text-slate-600 hover:text-slate-900 hover:bg-slate-900/[0.06] dark:text-[#94A3B8] dark:hover:text-white dark:hover:bg-white/[0.06] transition-colors whitespace-nowrap shrink-0"
            >
              <Folder className="w-3.5 h-3.5 text-blue-600/80 dark:text-blue-400/80" />
              <span>{folder.name}</span>
            </button>
            {isOpen && (
              <div
                className="fixed mt-1 z-50 w-60 rounded-lg bg-white dark:bg-[#151922] border border-slate-200 dark:border-white/[0.1] shadow-2xl py-1"
                onMouseLeave={() => setOpenFolderId(null)}
              >
                {folderBookmarks.length === 0 ? (
                  <div className="px-3 py-2 text-[11px] text-slate-400 dark:text-[#64748B]">
                    Папка порожня
                  </div>
                ) : (
                  folderBookmarks.map((b) => (
                    <button
                      key={b.id}
                      type="button"
                      onClick={() => {
                        onOpenUrl(b.url);
                        setOpenFolderId(null);
                      }}
                      className="flex w-full items-center gap-2 px-3 py-1.5 text-left text-slate-700 hover:bg-slate-100 hover:text-slate-900 dark:text-[#CBD5E1] dark:hover:bg-white/[0.06] dark:hover:text-white"
                    >
                      <Globe className="w-3.5 h-3.5 text-slate-400 dark:text-[#64748B] shrink-0" />
                      <span className="truncate">{b.title}</span>
                    </button>
                  ))
                )}
              </div>
            )}
          </div>
        );
      })}

      {quickItems.map((b) => (
        <button
          key={b.id}
          type="button"
          onClick={(e) => onOpenUrl(b.url, e.ctrlKey || e.metaKey)}
          title={b.url}
          className="flex items-center gap-1.5 px-2 py-1 rounded-md text-slate-700 hover:text-slate-900 hover:bg-slate-900/[0.06] dark:text-[#B8C1D1] dark:hover:text-white dark:hover:bg-white/[0.06] transition-colors max-w-[170px] whitespace-nowrap shrink-0"
        >
          <Globe className="w-3 h-3 text-slate-400 dark:text-[#64748B] shrink-0" />
          <span className="truncate">{b.title}</span>
        </button>
      ))}

      {quickItems.length === 0 && secondaryFolders.length === 0 && (
        <span className="text-[11px] text-slate-400 dark:text-[#64748B]">
          Натисніть зірочку в адресному рядку, щоб закріпити сайти тут
        </span>
      )}
    </div>
  );
};
