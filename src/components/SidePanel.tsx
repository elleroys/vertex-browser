import React, { useMemo, useRef, useState } from 'react';
import {
  Bookmark,
  Check,
  Copy,
  Download,
  Edit3,
  Eye,
  EyeOff,
  FolderPlus,
  Globe,
  History,
  KeyRound,
  Lock,
  Pause,
  Play,
  Plus,
  RotateCw,
  Search,
  Trash2,
  Upload,
  X,
} from 'lucide-react';
import type {
  BookmarkFolder,
  BookmarkItem,
  DownloadRecord,
  HistoryItem,
  PasswordCredential,
  SidePanelSection,
  VaultStatus,
} from '../types/browser';

interface SidePanelProps {
  section: SidePanelSection;
  onChangeSection: (s: SidePanelSection) => void;
  onClose: () => void;
  onOpenUrl: (url: string, newTab?: boolean) => void;
  bookmarks: BookmarkItem[];
  bookmarkFolders: BookmarkFolder[];
  onSaveBookmark: (item: Partial<BookmarkItem>) => Promise<void>;
  onDeleteBookmark: (id: string) => Promise<void>;
  onCreateFolder: (name: string) => Promise<void>;
  onImportBookmarksHtml: (content: string) => Promise<number>;
  history: HistoryItem[];
  onSearchHistory: (q: string) => void;
  onDeleteHistoryItem: (id: string) => Promise<void>;
  onClearHistory: (hoursBack: number | null) => Promise<void>;
  downloads: DownloadRecord[];
  onStartDownload: (url: string) => Promise<void>;
  onPauseDownload: (id: string) => Promise<void>;
  onResumeDownload: (id: string) => Promise<void>;
  onRetryDownload: (id: string) => Promise<void>;
  onDeleteDownload: (id: string) => Promise<void>;
  vaultStatus: VaultStatus;
  passwords: PasswordCredential[];
  onVaultSetup: (masterPassword: string) => Promise<void>;
  onVaultUnlock: (masterPassword: string) => Promise<void>;
  onVaultLock: () => Promise<void>;
  onSavePassword: (entry: {
    id?: string;
    origin: string;
    title: string;
    username: string;
    password: string;
    notes?: string | null;
  }) => Promise<void>;
  onDeletePassword: (id: string) => Promise<void>;
  onImportPasswordsCsv: (csv: string) => Promise<number>;
}

function formatBytes(bytes: number): string {
  if (!bytes || bytes <= 0) return '0 Б';
  const units = ['Б', 'КБ', 'МБ', 'ГБ'];
  let val = bytes;
  let idx = 0;
  while (val >= 1024 && idx < units.length - 1) {
    val /= 1024;
    idx++;
  }
  return `${val < 10 && idx > 0 ? val.toFixed(1) : Math.round(val)} ${units[idx]}`;
}

export const SidePanel: React.FC<SidePanelProps> = ({
  section,
  onChangeSection,
  onClose,
  onOpenUrl,
  bookmarks,
  bookmarkFolders,
  onSaveBookmark,
  onDeleteBookmark,
  onCreateFolder,
  onImportBookmarksHtml,
  history,
  onSearchHistory,
  onDeleteHistoryItem,
  onClearHistory,
  downloads,
  onStartDownload,
  onPauseDownload,
  onResumeDownload,
  onRetryDownload,
  onDeleteDownload,
  vaultStatus,
  passwords,
  onVaultSetup,
  onVaultUnlock,
  onVaultLock,
  onSavePassword,
  onDeletePassword,
  onImportPasswordsCsv,
}) => {
  const [bmSearch, setBmSearch] = useState('');
  const [bmFolderFilter, setBmFolderFilter] = useState<string>('all');
  const [newFolderName, setNewFolderName] = useState('');
  const [editingBm, setEditingBm] = useState<Partial<BookmarkItem> | null>(null);
  const bmFileInputRef = useRef<HTMLInputElement>(null);

  const [histQuery, setHistQuery] = useState('');
  const [clearHours, setClearHours] = useState<string>('24');

  const [dlUrlInput, setDlUrlInput] = useState('');
  const [dlError, setDlError] = useState('');

  const [masterPassInput, setMasterPassInput] = useState('');
  const [masterPassConfirm, setMasterPassConfirm] = useState('');
  const [vaultError, setVaultError] = useState('');
  const [pwdSearch, setPwdSearch] = useState('');
  const [revealedIds, setRevealedIds] = useState<Record<string, boolean>>({});
  const [copiedId, setCopiedId] = useState<string | null>(null);
  const [editingPwd, setEditingPwd] = useState<{
    id?: string;
    origin: string;
    title: string;
    username: string;
    password: string;
    notes: string;
  } | null>(null);
  const [importNotice, setImportNotice] = useState('');
  const csvFileInputRef = useRef<HTMLInputElement>(null);

  const filteredBookmarks = useMemo(() => {
    const q = bmSearch.trim().toLowerCase();
    return bookmarks.filter((b) => {
      if (bmFolderFilter !== 'all' && b.folderId !== bmFolderFilter) return false;
      if (q && !b.title.toLowerCase().includes(q) && !b.url.toLowerCase().includes(q)) return false;
      return true;
    });
  }, [bookmarks, bmSearch, bmFolderFilter]);

  const filteredPasswords = useMemo(() => {
    const q = pwdSearch.trim().toLowerCase();
    if (!q) return passwords;
    return passwords.filter(
      (p) =>
        p.origin.toLowerCase().includes(q) ||
        p.title.toLowerCase().includes(q) ||
        p.username.toLowerCase().includes(q)
    );
  }, [passwords, pwdSearch]);

  const tabs: Array<{ id: SidePanelSection; label: string; icon: React.ReactNode }> = [
    { id: 'bookmarks', label: 'Закладки', icon: <Bookmark className="w-3.5 h-3.5" /> },
    { id: 'history', label: 'Історія', icon: <History className="w-3.5 h-3.5" /> },
    { id: 'downloads', label: 'Завантаження', icon: <Download className="w-3.5 h-3.5" /> },
    { id: 'passwords', label: 'Паролі', icon: <KeyRound className="w-3.5 h-3.5" /> },
  ];

  const handleCopy = async (id: string, text: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setCopiedId(id);
      setTimeout(() => setCopiedId(null), 1500);
    } catch {
      // ignore clipboard denial
    }
  };

  return (
    <aside className="flex flex-col w-[360px] max-w-full h-full bg-white dark:bg-[#11151D] border-l border-slate-200 dark:border-white/[0.08] shrink-0 select-none">
      {/* Шапка панелі */}
      <div className="flex items-center justify-between px-3 py-2.5 border-b border-slate-200 dark:border-white/[0.07]">
        <div className="flex items-center gap-1 bg-slate-100 dark:bg-[#0B0D11] p-1 rounded-lg border border-slate-200 dark:border-white/[0.06] overflow-x-auto no-scrollbar">
          {tabs.map((t) => (
            <button
              key={t.id}
              type="button"
              onClick={() => onChangeSection(t.id)}
              className={`flex items-center gap-1.5 px-2.5 py-1 rounded-md text-[11px] font-medium transition-colors whitespace-nowrap shrink-0 ${
                section === t.id
                  ? 'bg-white text-slate-900 shadow-2xs dark:bg-[#1A202C] dark:text-white'
                  : 'text-slate-600 hover:text-slate-900 dark:text-[#8F98A8] dark:hover:text-[#CBD5E1]'
              }`}
            >
              {t.icon}
              <span>{t.label}</span>
            </button>
          ))}
        </div>
        <button
          type="button"
          onClick={onClose}
          title="Закрити панель"
          className="p-1.5 rounded-lg text-slate-500 hover:text-slate-900 hover:bg-slate-100 dark:text-[#8F98A8] dark:hover:text-white dark:hover:bg-white/[0.07] shrink-0 ml-1"
        >
          <X className="w-4 h-4" />
        </button>
      </div>

      {/* Вміст активного розділу */}
      <div className="flex-1 overflow-y-auto p-3 space-y-3 text-[13px]">
        {/* === ЗАКЛАДКИ === */}
        {section === 'bookmarks' && (
          <>
            <div className="flex items-center gap-2">
              <div className="relative flex-1">
                <Search className="w-3.5 h-3.5 text-slate-400 dark:text-[#64748B] absolute left-2.5 top-2.5" />
                <input
                  type="text"
                  value={bmSearch}
                  onChange={(e) => setBmSearch(e.target.value)}
                  placeholder="Пошук закладок..."
                  className="w-full rounded-lg bg-slate-50 dark:bg-[#0B0D11] border border-slate-200 dark:border-white/[0.08] pl-8 pr-2.5 py-1.5 text-[12px] text-slate-900 dark:text-white focus:outline-none focus:border-blue-500"
                />
              </div>
              <button
                type="button"
                onClick={() =>
                  setEditingBm({ title: '', url: 'https://', folderId: 'quick-bar', onBar: true })
                }
                title="Додати закладку"
                className="p-1.5 rounded-lg bg-blue-600 hover:bg-blue-500 text-white shrink-0"
              >
                <Plus className="w-4 h-4" />
              </button>
            </div>

            <div className="flex items-center gap-1.5 flex-wrap">
              <select
                value={bmFolderFilter}
                onChange={(e) => setBmFolderFilter(e.target.value)}
                className="flex-1 rounded-lg bg-slate-50 dark:bg-[#0B0D11] border border-slate-200 dark:border-white/[0.08] px-2.5 py-1.5 text-[12px] text-slate-700 dark:text-[#CBD5E1] focus:outline-none"
              >
                <option value="all">Усі папки ({bookmarks.length})</option>
                {bookmarkFolders.map((f) => (
                  <option key={f.id} value={f.id}>
                    {f.name}
                  </option>
                ))}
              </select>
              <a
                href="/api/bookmarks/export"
                download="vortex-bookmarks.html"
                className="px-2.5 py-1.5 rounded-lg bg-slate-100 hover:bg-slate-200 dark:bg-[#181D27] dark:hover:bg-[#1F2633] border border-slate-200 dark:border-white/[0.07] text-[11px] text-slate-700 dark:text-[#CBD5E1] whitespace-nowrap"
              >
                Експорт
              </a>
              <button
                type="button"
                onClick={() => bmFileInputRef.current?.click()}
                className="px-2.5 py-1.5 rounded-lg bg-slate-100 hover:bg-slate-200 dark:bg-[#181D27] dark:hover:bg-[#1F2633] border border-slate-200 dark:border-white/[0.07] text-[11px] text-slate-700 dark:text-[#CBD5E1] whitespace-nowrap"
              >
                Імпорт HTML
              </button>
              <input
                ref={bmFileInputRef}
                type="file"
                accept=".html,.htm"
                className="hidden"
                onChange={async (e) => {
                  const file = e.target.files?.[0];
                  if (!file) return;
                  const text = await file.text();
                  const count = await onImportBookmarksHtml(text);
                  setImportNotice(`Імпортовано закладок: ${count}`);
                  e.target.value = '';
                }}
              />
            </div>

            {importNotice && (
              <div className="text-[11px] text-emerald-700 dark:text-emerald-400 bg-emerald-500/10 px-2.5 py-1.5 rounded-lg">
                {importNotice}
              </div>
            )}

            <form
              onSubmit={async (e) => {
                e.preventDefault();
                if (!newFolderName.trim()) return;
                await onCreateFolder(newFolderName.trim());
                setNewFolderName('');
              }}
              className="flex items-center gap-1.5"
            >
              <input
                type="text"
                value={newFolderName}
                onChange={(e) => setNewFolderName(e.target.value)}
                placeholder="Нова папка..."
                className="flex-1 rounded-lg bg-slate-50 dark:bg-[#0B0D11] border border-slate-200 dark:border-white/[0.07] px-2.5 py-1 text-[12px] text-slate-900 dark:text-white focus:outline-none focus:border-blue-500"
              />
              <button
                type="submit"
                className="flex items-center gap-1 px-2.5 py-1 rounded-lg bg-slate-100 hover:bg-slate-200 dark:bg-[#181D27] dark:hover:bg-[#1F2633] text-[11px] text-slate-700 dark:text-[#CBD5E1] border border-slate-200 dark:border-white/[0.07]"
              >
                <FolderPlus className="w-3.5 h-3.5" />
                <span>Створити</span>
              </button>
            </form>

            {editingBm && (
              <div className="p-3 rounded-xl bg-slate-50 dark:bg-[#0B0D11] border border-blue-500/30 space-y-2">
                <div className="text-[12px] font-semibold text-slate-900 dark:text-white">
                  {editingBm.id ? 'Редагувати закладку' : 'Нова закладка'}
                </div>
                <input
                  type="text"
                  value={editingBm.title || ''}
                  onChange={(e) => setEditingBm({ ...editingBm, title: e.target.value })}
                  placeholder="Назва сайту"
                  className="w-full rounded-lg bg-white dark:bg-[#151922] border border-slate-200 dark:border-white/[0.08] px-2.5 py-1.5 text-[12px] text-slate-900 dark:text-white"
                />
                <input
                  type="url"
                  value={editingBm.url || ''}
                  onChange={(e) => setEditingBm({ ...editingBm, url: e.target.value })}
                  placeholder="https://example.com"
                  className="w-full rounded-lg bg-white dark:bg-[#151922] border border-slate-200 dark:border-white/[0.08] px-2.5 py-1.5 text-[12px] font-mono text-slate-900 dark:text-white"
                />
                <select
                  value={editingBm.folderId || 'quick-bar'}
                  onChange={(e) => setEditingBm({ ...editingBm, folderId: e.target.value })}
                  className="w-full rounded-lg bg-white dark:bg-[#151922] border border-slate-200 dark:border-white/[0.08] px-2.5 py-1.5 text-[12px] text-slate-900 dark:text-white"
                >
                  {bookmarkFolders.map((f) => (
                    <option key={f.id} value={f.id}>
                      {f.name}
                    </option>
                  ))}
                </select>
                <div className="flex justify-end gap-2 pt-1">
                  <button
                    type="button"
                    onClick={() => setEditingBm(null)}
                    className="px-2.5 py-1 text-[11px] text-slate-500 hover:text-slate-900 dark:text-[#8F98A8] dark:hover:text-white"
                  >
                    Скасувати
                  </button>
                  <button
                    type="button"
                    onClick={async () => {
                      if (!editingBm.url?.trim()) return;
                      await onSaveBookmark(editingBm);
                      setEditingBm(null);
                    }}
                    className="px-3 py-1 rounded-md bg-blue-600 hover:bg-blue-500 text-white text-[11px] font-medium"
                  >
                    Зберегти
                  </button>
                </div>
              </div>
            )}

            <div className="divide-y divide-slate-200 dark:divide-white/[0.05]">
              {filteredBookmarks.length === 0 ? (
                <div className="py-8 text-center text-[12px] text-slate-400 dark:text-[#64748B]">
                  Закладок за вашим запитом не знайдено
                </div>
              ) : (
                filteredBookmarks.map((b) => (
                  <div
                    key={b.id}
                    className="group flex items-center gap-2.5 py-2 px-1.5 rounded-lg hover:bg-slate-100 dark:hover:bg-white/[0.04]"
                  >
                    <Globe className="w-3.5 h-3.5 text-slate-400 dark:text-[#64748B] shrink-0" />
                    <button
                      type="button"
                      onClick={(e) => onOpenUrl(b.url, e.ctrlKey || e.metaKey)}
                      className="flex-1 min-w-0 text-left"
                    >
                      <div className="truncate text-[12px] font-medium text-slate-900 dark:text-[#E6EAF2]">
                        {b.title}
                      </div>
                      <div className="truncate text-[11px] font-mono text-slate-400 dark:text-[#64748B]">
                        {b.url}
                      </div>
                    </button>
                    <div className="flex items-center gap-0.5 opacity-0 group-hover:opacity-100 transition-opacity shrink-0">
                      <button
                        type="button"
                        onClick={() => setEditingBm(b)}
                        title="Редагувати"
                        className="p-1 text-slate-500 hover:text-slate-900 dark:text-[#8F98A8] dark:hover:text-white"
                      >
                        <Edit3 className="w-3.5 h-3.5" />
                      </button>
                      <button
                        type="button"
                        onClick={() => onDeleteBookmark(b.id)}
                        title="Видалити"
                        className="p-1 text-slate-500 hover:text-red-500 dark:text-[#8F98A8] dark:hover:text-red-400"
                      >
                        <Trash2 className="w-3.5 h-3.5" />
                      </button>
                    </div>
                  </div>
                ))
              )}
            </div>
          </>
        )}

        {/* === ІСТОРІЯ === */}
        {section === 'history' && (
          <>
            <div className="relative">
              <Search className="w-3.5 h-3.5 text-slate-400 dark:text-[#64748B] absolute left-2.5 top-2.5" />
              <input
                type="text"
                value={histQuery}
                onChange={(e) => {
                  setHistQuery(e.target.value);
                  onSearchHistory(e.target.value);
                }}
                placeholder="Пошук в історії за назвою або URL..."
                className="w-full rounded-lg bg-slate-50 dark:bg-[#0B0D11] border border-slate-200 dark:border-white/[0.08] pl-8 pr-2.5 py-1.5 text-[12px] text-slate-900 dark:text-white focus:outline-none focus:border-blue-500"
              />
            </div>

            <div className="flex items-center gap-2">
              <select
                value={clearHours}
                onChange={(e) => setClearHours(e.target.value)}
                className="flex-1 rounded-lg bg-slate-50 dark:bg-[#0B0D11] border border-slate-200 dark:border-white/[0.08] px-2.5 py-1.5 text-[12px] text-slate-700 dark:text-[#CBD5E1]"
              >
                <option value="1">За останню годину</option>
                <option value="24">За останні 24 години</option>
                <option value="168">За останні 7 днів</option>
                <option value="all">Уся історія</option>
              </select>
              <button
                type="button"
                onClick={() => onClearHistory(clearHours === 'all' ? null : Number(clearHours))}
                className="px-3 py-1.5 rounded-lg bg-red-500/15 hover:bg-red-500/25 text-red-600 dark:text-red-300 text-[12px] font-medium whitespace-nowrap"
              >
                Очистити
              </button>
            </div>

            <div className="divide-y divide-slate-200 dark:divide-white/[0.05]">
              {history.length === 0 ? (
                <div className="py-10 text-center text-[12px] text-slate-400 dark:text-[#64748B]">
                  Історія відвідувань порожня. Приватні вкладки не зберігаються.
                </div>
              ) : (
                history.map((h) => (
                  <div
                    key={h.id}
                    className="group flex items-center gap-2.5 py-2 px-1.5 rounded-lg hover:bg-slate-100 dark:hover:bg-white/[0.04]"
                  >
                    <Globe className="w-3.5 h-3.5 text-slate-400 dark:text-[#64748B] shrink-0" />
                    <button
                      type="button"
                      onClick={(e) => onOpenUrl(h.url, e.ctrlKey || e.metaKey)}
                      className="flex-1 min-w-0 text-left"
                    >
                      <div className="truncate text-[12px] font-medium text-slate-900 dark:text-[#E6EAF2]">
                        {h.title}
                      </div>
                      <div className="flex items-center gap-1.5 text-[11px] text-slate-400 dark:text-[#64748B]">
                        <span className="truncate font-mono">{h.url}</span>
                        <span>·</span>
                        <span className="shrink-0 tabular-nums">
                          {new Date(h.visitedAt).toLocaleTimeString('uk-UA', {
                            hour: '2-digit',
                            minute: '2-digit',
                          })}
                        </span>
                      </div>
                    </button>
                    <button
                      type="button"
                      onClick={() => onDeleteHistoryItem(h.id)}
                      title="Видалити запис"
                      className="p-1 opacity-0 group-hover:opacity-100 text-slate-500 hover:text-red-500 dark:text-[#8F98A8] dark:hover:text-red-400"
                    >
                      <Trash2 className="w-3.5 h-3.5" />
                    </button>
                  </div>
                ))
              )}
            </div>
          </>
        )}

        {/* === ЗАВАНТАЖЕННЯ === */}
        {section === 'downloads' && (
          <>
            <form
              onSubmit={async (e) => {
                e.preventDefault();
                setDlError('');
                if (!dlUrlInput.trim()) return;
                try {
                  await onStartDownload(dlUrlInput.trim());
                  setDlUrlInput('');
                } catch (err: unknown) {
                  setDlError(err instanceof Error ? err.message : 'Помилка завантаження');
                }
              }}
              className="space-y-1.5"
            >
              <div className="flex items-center gap-1.5">
                <input
                  type="url"
                  value={dlUrlInput}
                  onChange={(e) => setDlUrlInput(e.target.value)}
                  placeholder="https://... (посилання на файл)"
                  className="flex-1 rounded-lg bg-slate-50 dark:bg-[#0B0D11] border border-slate-200 dark:border-white/[0.08] px-2.5 py-1.5 text-[12px] font-mono text-slate-900 dark:text-white focus:outline-none focus:border-blue-500"
                />
                <button
                  type="submit"
                  className="px-3 py-1.5 rounded-lg bg-blue-600 hover:bg-blue-500 text-white text-[12px] font-medium whitespace-nowrap"
                >
                  Завантажити
                </button>
              </div>
              {dlError && <div className="text-[11px] text-red-600 dark:text-red-400">{dlError}</div>}
            </form>

            <div className="space-y-2">
              {downloads.length === 0 ? (
                <div className="py-10 text-center text-[12px] text-slate-400 dark:text-[#64748B]">
                  Завантажень поки немає
                </div>
              ) : (
                downloads.map((dl) => {
                  const pct =
                    dl.totalBytes > 0
                      ? Math.min(100, Math.round((dl.receivedBytes / dl.totalBytes) * 100))
                      : dl.status === 'completed'
                        ? 100
                        : 0;

                  return (
                    <div
                      key={dl.id}
                      className="rounded-xl bg-slate-50 dark:bg-[#0B0D11] border border-slate-200 dark:border-white/[0.07] p-3 space-y-2"
                    >
                      <div className="flex items-start justify-between gap-2">
                        <div className="min-w-0">
                          <div className="truncate font-medium text-slate-900 dark:text-white text-[12px]">
                            {dl.filename}
                          </div>
                          <div className="truncate font-mono text-[10px] text-slate-400 dark:text-[#64748B]">
                            {dl.url}
                          </div>
                        </div>
                        <button
                          type="button"
                          onClick={() => onDeleteDownload(dl.id)}
                          title="Видалити"
                          className="text-slate-400 hover:text-red-500 dark:text-[#64748B] dark:hover:text-red-400 shrink-0"
                        >
                          <Trash2 className="w-3.5 h-3.5" />
                        </button>
                      </div>

                      {dl.status === 'downloading' && (
                        <div className="h-1.5 w-full rounded-full bg-slate-200 dark:bg-white/[0.06] overflow-hidden">
                          <div
                            className="h-full bg-blue-500 transition-all"
                            style={{ width: `${pct}%` }}
                          />
                        </div>
                      )}

                      <div className="flex items-center justify-between text-[11px] text-slate-500 dark:text-[#8F98A8] font-mono tabular-nums">
                        <span>
                          {formatBytes(dl.receivedBytes)}
                          {dl.totalBytes > 0 ? ` / ${formatBytes(dl.totalBytes)}` : ''}
                          {dl.status === 'downloading' && dl.speedBps
                            ? ` · ${formatBytes(dl.speedBps)}/с`
                            : ''}
                          {dl.status === 'paused' ? ' · Призупинено' : ''}
                          {dl.status === 'failed' ? ' · Помилка' : ''}
                          {dl.status === 'completed' ? ' · Готово' : ''}
                        </span>
                        <div className="flex items-center gap-1.5 font-sans">
                          {dl.status === 'downloading' && (
                            <button
                              type="button"
                              onClick={() => onPauseDownload(dl.id)}
                              title="Призупинити"
                              className="p-1 rounded bg-slate-200 hover:bg-slate-300 text-slate-800 dark:bg-white/[0.06] dark:hover:bg-white/[0.12] dark:text-white"
                            >
                              <Pause className="w-3 h-3" />
                            </button>
                          )}
                          {dl.status === 'paused' && (
                            <button
                              type="button"
                              onClick={() => onResumeDownload(dl.id)}
                              title="Відновити"
                              className="p-1 rounded bg-blue-600/80 hover:bg-blue-500 text-white"
                            >
                              <Play className="w-3 h-3" />
                            </button>
                          )}
                          {dl.status === 'failed' && (
                            <button
                              type="button"
                              onClick={() => onRetryDownload(dl.id)}
                              title="Повторити"
                              className="p-1 rounded bg-slate-200 hover:bg-slate-300 text-slate-800 dark:bg-white/[0.06] dark:hover:bg-white/[0.12] dark:text-white"
                            >
                              <RotateCw className="w-3 h-3" />
                            </button>
                          )}
                          {dl.status === 'completed' && (
                            <a
                              href={`/api/downloads/${encodeURIComponent(dl.id)}/file`}
                              download={dl.filename}
                              className="px-2 py-0.5 rounded bg-emerald-500/15 hover:bg-emerald-500/25 text-emerald-700 dark:text-emerald-300 text-[11px]"
                            >
                              Зберегти файл
                            </a>
                          )}
                        </div>
                      </div>
                    </div>
                  );
                })
              )}
            </div>
          </>
        )}

        {/* === МЕНЕДЖЕР ПАРОЛІВ === */}
        {section === 'passwords' && (
          <>
            {!vaultStatus.isConfigured ? (
              <form
                onSubmit={async (e) => {
                  e.preventDefault();
                  setVaultError('');
                  if (masterPassInput.length < 8) {
                    setVaultError('Майстер-пароль має містити щонайменше 8 символів');
                    return;
                  }
                  if (masterPassInput !== masterPassConfirm) {
                    setVaultError('Паролі не збігаються');
                    return;
                  }
                  try {
                    await onVaultSetup(masterPassInput);
                    setMasterPassInput('');
                    setMasterPassConfirm('');
                  } catch (err: unknown) {
                    setVaultError(err instanceof Error ? err.message : 'Помилка налаштування');
                  }
                }}
                className="rounded-xl bg-slate-50 dark:bg-[#0B0D11] border border-slate-200 dark:border-white/[0.08] p-4 space-y-3"
              >
                <div className="flex items-center gap-2 text-slate-900 dark:text-white font-semibold">
                  <Lock className="w-4 h-4 text-blue-600 dark:text-blue-400" />
                  <span>Створення сховища паролів</span>
                </div>
                <p className="text-[12px] text-slate-500 dark:text-[#8F98A8] leading-relaxed">
                  Усі паролі шифруються локально (Argon2id + XChaCha20-Poly1305 / AES-256-GCM) і ніколи не передаються назовні.
                </p>
                <input
                  type="password"
                  value={masterPassInput}
                  onChange={(e) => setMasterPassInput(e.target.value)}
                  placeholder="Майстер-пароль (мін. 8 символів)"
                  className="w-full rounded-lg bg-white dark:bg-[#151922] border border-slate-200 dark:border-white/[0.1] px-3 py-2 text-[12px] text-slate-900 dark:text-white"
                />
                <input
                  type="password"
                  value={masterPassConfirm}
                  onChange={(e) => setMasterPassConfirm(e.target.value)}
                  placeholder="Підтвердіть майстер-пароль"
                  className="w-full rounded-lg bg-white dark:bg-[#151922] border border-slate-200 dark:border-white/[0.1] px-3 py-2 text-[12px] text-slate-900 dark:text-white"
                />
                {vaultError && <div className="text-[11px] text-red-600 dark:text-red-400">{vaultError}</div>}
                <button
                  type="submit"
                  className="w-full py-2 rounded-lg bg-blue-600 hover:bg-blue-500 text-white text-[12px] font-medium"
                >
                  Ініціалізувати сховище
                </button>
              </form>
            ) : !vaultStatus.isUnlocked ? (
              <form
                onSubmit={async (e) => {
                  e.preventDefault();
                  setVaultError('');
                  try {
                    await onVaultUnlock(masterPassInput);
                    setMasterPassInput('');
                  } catch (err: unknown) {
                    setVaultError(err instanceof Error ? err.message : 'Невірний майстер-пароль');
                  }
                }}
                className="rounded-xl bg-slate-50 dark:bg-[#0B0D11] border border-slate-200 dark:border-white/[0.08] p-4 space-y-3"
              >
                <div className="flex items-center gap-2 text-slate-900 dark:text-white font-semibold">
                  <Lock className="w-4 h-4 text-amber-600 dark:text-amber-400" />
                  <span>Сховище заблоковано</span>
                </div>
                <p className="text-[12px] text-slate-500 dark:text-[#8F98A8]">
                  Збережено записів: <span className="font-mono tabular-nums">{vaultStatus.entryCount}</span>. Введіть майстер-пароль для розшифрування.
                </p>
                <input
                  type="password"
                  value={masterPassInput}
                  onChange={(e) => setMasterPassInput(e.target.value)}
                  placeholder="Майстер-пароль..."
                  className="w-full rounded-lg bg-white dark:bg-[#151922] border border-slate-200 dark:border-white/[0.1] px-3 py-2 text-[12px] text-slate-900 dark:text-white"
                />
                {vaultError && <div className="text-[11px] text-red-600 dark:text-red-400">{vaultError}</div>}
                <button
                  type="submit"
                  className="w-full py-2 rounded-lg bg-blue-600 hover:bg-blue-500 text-white text-[12px] font-medium"
                >
                  Розблокувати
                </button>
              </form>
            ) : (
              <>
                <div className="flex items-center gap-1.5">
                  <div className="relative flex-1">
                    <Search className="w-3.5 h-3.5 text-slate-400 dark:text-[#64748B] absolute left-2.5 top-2.5" />
                    <input
                      type="text"
                      value={pwdSearch}
                      onChange={(e) => setPwdSearch(e.target.value)}
                      placeholder="Пошук сайту або логіну..."
                      className="w-full rounded-lg bg-slate-50 dark:bg-[#0B0D11] border border-slate-200 dark:border-white/[0.08] pl-8 pr-2.5 py-1.5 text-[12px] text-slate-900 dark:text-white"
                    />
                  </div>
                  <button
                    type="button"
                    onClick={() =>
                      setEditingPwd({ origin: '', title: '', username: '', password: '', notes: '' })
                    }
                    title="Додати обліковий запис"
                    className="p-1.5 rounded-lg bg-blue-600 hover:bg-blue-500 text-white shrink-0"
                  >
                    <Plus className="w-4 h-4" />
                  </button>
                  <button
                    type="button"
                    onClick={() => onVaultLock()}
                    title="Заблокувати сховище"
                    className="p-1.5 rounded-lg bg-slate-100 hover:bg-slate-200 text-slate-700 dark:bg-[#181D27] dark:hover:bg-[#1F2633] dark:text-[#CBD5E1] border border-slate-200 dark:border-white/[0.07] shrink-0"
                  >
                    <Lock className="w-4 h-4" />
                  </button>
                </div>

                <div className="flex items-center justify-between gap-2">
                  <span className="text-[11px] text-slate-500 dark:text-[#8F98A8] font-mono tabular-nums">
                    Записів: {passwords.length}
                  </span>
                  <button
                    type="button"
                    onClick={() => csvFileInputRef.current?.click()}
                    className="flex items-center gap-1.5 px-2.5 py-1 rounded-lg bg-slate-100 hover:bg-slate-200 text-slate-700 dark:bg-[#181D27] dark:hover:bg-[#1F2633] border border-slate-200 dark:border-white/[0.07] text-[11px] dark:text-[#CBD5E1]"
                  >
                    <Upload className="w-3.5 h-3.5" />
                    <span>Імпорт CSV (Google)</span>
                  </button>
                  <input
                    ref={csvFileInputRef}
                    type="file"
                    accept=".csv,text/csv"
                    className="hidden"
                    onChange={async (e) => {
                      const file = e.target.files?.[0];
                      if (!file) return;
                      const text = await file.text();
                      try {
                        const imported = await onImportPasswordsCsv(text);
                        setImportNotice(`Імпортовано паролів із CSV: ${imported}`);
                      } catch (err: unknown) {
                        setVaultError(err instanceof Error ? err.message : 'Помилка імпорту CSV');
                      }
                      e.target.value = '';
                    }}
                  />
                </div>

                {importNotice && (
                  <div className="text-[11px] text-emerald-700 dark:text-emerald-400 bg-emerald-500/10 px-2.5 py-1.5 rounded-lg">
                    {importNotice}
                  </div>
                )}

                {editingPwd && (
                  <div className="rounded-xl bg-slate-50 dark:bg-[#0B0D11] border border-blue-500/30 p-3 space-y-2">
                    <div className="text-[12px] font-semibold text-slate-900 dark:text-white">
                      {editingPwd.id ? 'Редагувати запис' : 'Новий пароль'}
                    </div>
                    <input
                      type="text"
                      value={editingPwd.origin}
                      onChange={(e) => setEditingPwd({ ...editingPwd, origin: e.target.value })}
                      placeholder="Домен (напр. github.com)"
                      className="w-full rounded-lg bg-white dark:bg-[#151922] border border-slate-200 dark:border-white/[0.08] px-2.5 py-1.5 text-[12px] font-mono text-slate-900 dark:text-white"
                    />
                    <input
                      type="text"
                      value={editingPwd.title}
                      onChange={(e) => setEditingPwd({ ...editingPwd, title: e.target.value })}
                      placeholder="Назва сервісу"
                      className="w-full rounded-lg bg-white dark:bg-[#151922] border border-slate-200 dark:border-white/[0.08] px-2.5 py-1.5 text-[12px] text-slate-900 dark:text-white"
                    />
                    <input
                      type="text"
                      value={editingPwd.username}
                      onChange={(e) => setEditingPwd({ ...editingPwd, username: e.target.value })}
                      placeholder="Логін або email"
                      className="w-full rounded-lg bg-white dark:bg-[#151922] border border-slate-200 dark:border-white/[0.08] px-2.5 py-1.5 text-[12px] text-slate-900 dark:text-white"
                    />
                    <input
                      type="text"
                      value={editingPwd.password}
                      onChange={(e) => setEditingPwd({ ...editingPwd, password: e.target.value })}
                      placeholder="Пароль"
                      className="w-full rounded-lg bg-white dark:bg-[#151922] border border-slate-200 dark:border-white/[0.08] px-2.5 py-1.5 text-[12px] font-mono text-slate-900 dark:text-white"
                    />
                    <div className="flex justify-end gap-2 pt-1">
                      <button
                        type="button"
                        onClick={() => setEditingPwd(null)}
                        className="px-2.5 py-1 text-[11px] text-slate-500 hover:text-slate-900 dark:text-[#8F98A8] dark:hover:text-white"
                      >
                        Скасувати
                      </button>
                      <button
                        type="button"
                        onClick={async () => {
                          if (!editingPwd.origin.trim() || !editingPwd.password) return;
                          await onSavePassword({
                            id: editingPwd.id,
                            origin: editingPwd.origin,
                            title: editingPwd.title || editingPwd.origin,
                            username: editingPwd.username,
                            password: editingPwd.password,
                            notes: editingPwd.notes || null,
                          });
                          setEditingPwd(null);
                        }}
                        className="px-3 py-1 rounded-md bg-blue-600 hover:bg-blue-500 text-white text-[11px] font-medium"
                      >
                        Зберегти
                      </button>
                    </div>
                  </div>
                )}

                <div className="space-y-2">
                  {filteredPasswords.length === 0 ? (
                    <div className="py-8 text-center text-[12px] text-slate-400 dark:text-[#64748B]">
                      Збережених паролів не знайдено
                    </div>
                  ) : (
                    filteredPasswords.map((p) => {
                      const shown = !!revealedIds[p.id];
                      return (
                        <div
                          key={p.id}
                          className="rounded-xl bg-slate-50 dark:bg-[#0B0D11] border border-slate-200 dark:border-white/[0.07] p-3 space-y-1.5"
                        >
                          <div className="flex items-center justify-between gap-2">
                            <div className="min-w-0">
                              <div className="truncate font-semibold text-slate-900 dark:text-white text-[12px]">
                                {p.title}
                              </div>
                              <div className="truncate font-mono text-[11px] text-slate-400 dark:text-[#64748B]">
                                {p.origin}
                              </div>
                            </div>
                            <div className="flex items-center gap-1 shrink-0">
                              <button
                                type="button"
                                onClick={() =>
                                  setEditingPwd({
                                    id: p.id,
                                    origin: p.origin,
                                    title: p.title,
                                    username: p.username,
                                    password: p.password,
                                    notes: p.notes || '',
                                  })
                                }
                                className="p-1 text-slate-500 hover:text-slate-900 dark:text-[#8F98A8] dark:hover:text-white"
                                title="Редагувати"
                              >
                                <Edit3 className="w-3.5 h-3.5" />
                              </button>
                              <button
                                type="button"
                                onClick={() => onDeletePassword(p.id)}
                                className="p-1 text-slate-500 hover:text-red-500 dark:text-[#8F98A8] dark:hover:text-red-400"
                                title="Видалити"
                              >
                                <Trash2 className="w-3.5 h-3.5" />
                              </button>
                            </div>
                          </div>

                          <div className="flex items-center justify-between gap-2 text-[12px] bg-white dark:bg-[#151922] border border-slate-200 dark:border-transparent rounded-lg px-2.5 py-1.5">
                            <span className="truncate text-slate-700 dark:text-[#CBD5E1]">
                              {p.username || '—'}
                            </span>
                            <button
                              type="button"
                              onClick={() => handleCopy(`${p.id}-u`, p.username)}
                              className="text-slate-400 hover:text-slate-900 dark:text-[#8F98A8] dark:hover:text-white shrink-0"
                              title="Копіювати логін"
                            >
                              {copiedId === `${p.id}-u` ? (
                                <Check className="w-3.5 h-3.5 text-emerald-500 dark:text-emerald-400" />
                              ) : (
                                <Copy className="w-3.5 h-3.5" />
                              )}
                            </button>
                          </div>

                          <div className="flex items-center justify-between gap-2 text-[12px] bg-white dark:bg-[#151922] border border-slate-200 dark:border-transparent rounded-lg px-2.5 py-1.5">
                            <span className="truncate font-mono text-slate-700 dark:text-[#CBD5E1]">
                              {shown ? p.password : '••••••••••••'}
                            </span>
                            <div className="flex items-center gap-1.5 shrink-0">
                              <button
                                type="button"
                                onClick={() =>
                                  setRevealedIds((prev) => ({ ...prev, [p.id]: !shown }))
                                }
                                className="text-slate-400 hover:text-slate-900 dark:text-[#8F98A8] dark:hover:text-white"
                                title={shown ? 'Приховати' : 'Показати'}
                              >
                                {shown ? (
                                  <EyeOff className="w-3.5 h-3.5" />
                                ) : (
                                  <Eye className="w-3.5 h-3.5" />
                                )}
                              </button>
                              <button
                                type="button"
                                onClick={() => handleCopy(`${p.id}-p`, p.password)}
                                className="text-slate-400 hover:text-slate-900 dark:text-[#8F98A8] dark:hover:text-white"
                                title="Копіювати пароль"
                              >
                                {copiedId === `${p.id}-p` ? (
                                  <Check className="w-3.5 h-3.5 text-emerald-500 dark:text-emerald-400" />
                                ) : (
                                  <Copy className="w-3.5 h-3.5" />
                                )}
                              </button>
                            </div>
                          </div>
                        </div>
                      );
                    })
                  )}
                </div>
              </>
            )}
          </>
        )}
      </div>
    </aside>
  );
};
