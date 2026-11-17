import React, { useEffect, useRef, useState } from 'react';
import {
  Cpu,
  Database,
  Download,
  Globe,
  HardDrive,
  KeyRound,
  Moon,
  Plus,
  RefreshCw,
  Shield,
  Sun,
  Trash2,
  Upload,
} from 'lucide-react';
import type {
  AdBlockStatus,
  BrowserSettings,
  MemorySaverMode,
  SearchEngineId,
  SidePanelSection,
  SiteDataSummary,
} from '../types/browser';

interface SettingsPageProps {
  settings: BrowserSettings;
  adblockStatus: AdBlockStatus | null;
  siteData: SiteDataSummary[];
  onUpdateSettings: (patch: Partial<BrowserSettings>) => Promise<void>;
  onToggleAdBlockDomain: (domain: string, disableOnDomain: boolean) => Promise<void>;
  onUpdateAdBlockFilters: () => Promise<void>;
  onClearSiteData: (domain?: string) => Promise<void>;
  onExportAllData: () => Promise<Record<string, unknown>>;
  onImportAllData: (payload: Record<string, unknown>) => Promise<void>;
  onOpenSidePanel: (section: SidePanelSection) => void;
  onRefreshSiteData: () => void;
}

type SettingsTab = 'general' | 'adblock' | 'memory' | 'privacy' | 'downloads';

export const SettingsPage: React.FC<SettingsPageProps> = ({
  settings,
  adblockStatus,
  siteData,
  onUpdateSettings,
  onToggleAdBlockDomain,
  onUpdateAdBlockFilters,
  onClearSiteData,
  onExportAllData,
  onImportAllData,
  onOpenSidePanel,
  onRefreshSiteData,
}) => {
  const [activeSection, setActiveSection] = useState<SettingsTab>('general');
  const [newWhitelistDomain, setNewWhitelistDomain] = useState('');
  const [updatingFilters, setUpdatingFilters] = useState(false);
  const [statusMessage, setStatusMessage] = useState('');
  const importInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    onRefreshSiteData();
  }, [onRefreshSiteData]);

  const navItems: Array<{ id: SettingsTab; label: string; icon: React.ReactNode }> = [
    { id: 'general', label: 'Загальні та пошук', icon: <Globe className="w-4 h-4" /> },
    { id: 'adblock', label: 'AdBlock та винятки', icon: <Shield className="w-4 h-4" /> },
    { id: 'memory', label: 'Memory Saver та RAM', icon: <Cpu className="w-4 h-4" /> },
    { id: 'privacy', label: 'Приватність і дані', icon: <Database className="w-4 h-4" /> },
    { id: 'downloads', label: 'Завантаження й резервна копія', icon: <HardDrive className="w-4 h-4" /> },
  ];

  const notify = (msg: string) => {
    setStatusMessage(msg);
    setTimeout(() => setStatusMessage(''), 3000);
  };

  const toggleTheme = async () => {
    const nextTheme = settings.theme === 'dark' ? 'light' : 'dark';
    document.documentElement.classList.toggle('dark', nextTheme === 'dark');
    await onUpdateSettings({ theme: nextTheme });
    notify(nextTheme === 'dark' ? 'Увімкнено темну тему' : 'Увімкнено світлу тему');
  };

  return (
    <div className="flex h-full w-full bg-[#F4F6FA] text-slate-900 dark:bg-[#0B0D11] dark:text-[#E6EAF2] overflow-hidden">
      {/* Ліве меню налаштувань */}
      <nav className="w-[240px] shrink-0 border-r border-slate-200 dark:border-white/[0.07] bg-white dark:bg-[#0E1117] p-4 space-y-1">
        <div className="px-3 pb-3 mb-2 border-b border-slate-200 dark:border-white/[0.07]">
          <h1 className="text-[16px] font-semibold text-slate-900 dark:text-white">Налаштування</h1>
          <p className="text-[12px] text-slate-500 dark:text-[#8F98A8] mt-0.5">
            Конфігурація Vortex Browser
          </p>
        </div>

        {navItems.map((item) => (
          <button
            key={item.id}
            type="button"
            onClick={() => setActiveSection(item.id)}
            className={`flex w-full items-center gap-2.5 px-3 py-2 rounded-lg text-[13px] font-medium transition-colors text-left ${
              activeSection === item.id
                ? 'bg-slate-100 text-slate-900 dark:bg-[#181D27] dark:text-white'
                : 'text-slate-600 hover:text-slate-900 hover:bg-slate-100/70 dark:text-[#8F98A8] dark:hover:text-white dark:hover:bg-white/[0.04]'
            }`}
          >
            {item.icon}
            <span className="truncate">{item.label}</span>
          </button>
        ))}

        <div className="pt-4 mt-4 border-t border-slate-200 dark:border-white/[0.07] space-y-1">
          <button
            type="button"
            onClick={() => onOpenSidePanel('passwords')}
            className="flex w-full items-center gap-2.5 px-3 py-2 rounded-lg text-[13px] text-slate-600 hover:text-slate-900 hover:bg-slate-100/70 dark:text-[#94A3B8] dark:hover:text-white dark:hover:bg-white/[0.04]"
          >
            <KeyRound className="w-4 h-4" />
            <span>Менеджер паролів</span>
          </button>
          <button
            type="button"
            onClick={() => onOpenSidePanel('bookmarks')}
            className="flex w-full items-center gap-2.5 px-3 py-2 rounded-lg text-[13px] text-slate-600 hover:text-slate-900 hover:bg-slate-100/70 dark:text-[#94A3B8] dark:hover:text-white dark:hover:bg-white/[0.04]"
          >
            <Globe className="w-4 h-4" />
            <span>Керування закладками</span>
          </button>
        </div>
      </nav>

      {/* Вміст налаштувань */}
      <div className="flex-1 overflow-y-auto p-8">
        <div className="max-w-[760px] space-y-6">
          {statusMessage && (
            <div className="rounded-lg bg-emerald-500/15 border border-emerald-500/30 px-4 py-2.5 text-[13px] text-emerald-700 dark:text-emerald-300">
              {statusMessage}
            </div>
          )}

          {/* === ЗАГАЛЬНІ === */}
          {activeSection === 'general' && (
            <div className="space-y-6">
              <div>
                <h2 className="text-[20px] font-semibold text-slate-900 dark:text-white">
                  Загальні та пошук
                </h2>
                <p className="text-[13px] text-slate-500 dark:text-[#8F98A8] mt-1">
                  Налаштування теми оформлення, поведінки при запуску, пошукової системи та панелі закладок.
                </p>
              </div>

              <div className="rounded-xl bg-white dark:bg-[#12161E] border border-slate-200 dark:border-white/[0.07] divide-y divide-slate-200 dark:divide-white/[0.06]">
                {/* Кнопка перемикання теми (Світла / Темна) */}
                <div className="flex items-center justify-between gap-4 p-4">
                  <div>
                    <div className="text-[14px] font-medium text-slate-900 dark:text-white">
                      Тема оформлення інтерфейсу
                    </div>
                    <div className="text-[12px] text-slate-500 dark:text-[#8F98A8]">
                      Поточний режим: {settings.theme === 'dark' ? 'Темна тема' : 'Світла тема'}
                    </div>
                  </div>
                  <div className="flex items-center gap-2 shrink-0">
                    <div className="flex items-center p-1 rounded-lg bg-slate-100 dark:bg-[#0B0D11] border border-slate-200 dark:border-white/[0.08]">
                      <button
                        type="button"
                        onClick={() => {
                          if (settings.theme !== 'light') void toggleTheme();
                        }}
                        className={`flex items-center gap-1.5 px-3 py-1.5 rounded-md text-[12px] font-medium transition-colors whitespace-nowrap ${
                          settings.theme === 'light'
                            ? 'bg-white text-slate-900 shadow-xs'
                            : 'text-slate-500 hover:text-slate-900 dark:text-[#8F98A8] dark:hover:text-white'
                        }`}
                      >
                        <Sun className="w-3.5 h-3.5" />
                        <span>Світла</span>
                      </button>
                      <button
                        type="button"
                        onClick={() => {
                          if (settings.theme !== 'dark') void toggleTheme();
                        }}
                        className={`flex items-center gap-1.5 px-3 py-1.5 rounded-md text-[12px] font-medium transition-colors whitespace-nowrap ${
                          settings.theme === 'dark'
                            ? 'bg-[#1C222E] text-white shadow-xs'
                            : 'text-slate-600 hover:text-slate-900 dark:text-[#8F98A8] dark:hover:text-white'
                        }`}
                      >
                        <Moon className="w-3.5 h-3.5" />
                        <span>Темна</span>
                      </button>
                    </div>
                    <button
                      type="button"
                      onClick={() => void toggleTheme()}
                      aria-label="Перемкнути тему"
                      className="flex items-center gap-1.5 px-3 py-2 rounded-lg bg-blue-600 hover:bg-blue-500 text-white text-[12px] font-medium whitespace-nowrap transition-colors"
                    >
                      {settings.theme === 'dark' ? (
                        <>
                          <Sun className="w-3.5 h-3.5" />
                          <span>Увімкнути світлу</span>
                        </>
                      ) : (
                        <>
                          <Moon className="w-3.5 h-3.5" />
                          <span>Увімкнути темну</span>
                        </>
                      )}
                    </button>
                  </div>
                </div>

                <div className="flex items-center justify-between gap-4 p-4">
                  <div>
                    <div className="text-[14px] font-medium text-slate-900 dark:text-white">
                      Пошукова система за замовчуванням
                    </div>
                    <div className="text-[12px] text-slate-500 dark:text-[#8F98A8]">
                      Використовується при введенні пошукових запитів в адресний рядок
                    </div>
                  </div>
                  <select
                    value={settings.searchEngine}
                    onChange={(e) =>
                      onUpdateSettings({ searchEngine: e.target.value as SearchEngineId })
                    }
                    className="rounded-lg bg-slate-50 dark:bg-[#0B0D11] border border-slate-200 dark:border-white/[0.1] px-3 py-2 text-[13px] text-slate-900 dark:text-white"
                  >
                    <option value="google">Google (за замовчуванням)</option>
                    <option value="duckduckgo">DuckDuckGo</option>
                    <option value="brave">Brave Search</option>
                    <option value="bing">Bing</option>
                  </select>
                </div>

                <div className="flex items-center justify-between gap-4 p-4">
                  <div>
                    <div className="text-[14px] font-medium text-slate-900 dark:text-white">
                      Стартова сторінка
                    </div>
                    <div className="text-[12px] text-slate-500 dark:text-[#8F98A8]">
                      Сторінка, яка відкривається для нових вкладок та кнопки «Додому»
                    </div>
                  </div>
                  <input
                    type="text"
                    value={settings.startupPage}
                    onChange={(e) => onUpdateSettings({ startupPage: e.target.value })}
                    className="w-64 rounded-lg bg-slate-50 dark:bg-[#0B0D11] border border-slate-200 dark:border-white/[0.1] px-3 py-1.5 text-[13px] font-mono text-slate-900 dark:text-white"
                  />
                </div>

                <label className="flex items-center justify-between gap-4 p-4 cursor-pointer">
                  <div>
                    <div className="text-[14px] font-medium text-slate-900 dark:text-white">
                      Відновлювати вкладки після перезапуску
                    </div>
                    <div className="text-[12px] text-slate-500 dark:text-[#8F98A8]">
                      Зберігає відкриту сесію у локальній базі SQLite
                    </div>
                  </div>
                  <input
                    type="checkbox"
                    checked={settings.restoreTabsOnStartup}
                    onChange={(e) => onUpdateSettings({ restoreTabsOnStartup: e.target.checked })}
                    className="h-4 w-4 rounded border-slate-300 dark:border-white/20 bg-white dark:bg-[#0B0D11] text-blue-500"
                  />
                </label>

                <label className="flex items-center justify-between gap-4 p-4 cursor-pointer">
                  <div>
                    <div className="text-[14px] font-medium text-slate-900 dark:text-white">
                      Показувати панель швидкого доступу закладок
                    </div>
                    <div className="text-[12px] text-slate-500 dark:text-[#8F98A8]">
                      Компактна панель під адресним рядком
                    </div>
                  </div>
                  <input
                    type="checkbox"
                    checked={settings.showBookmarksBar}
                    onChange={(e) => onUpdateSettings({ showBookmarksBar: e.target.checked })}
                    className="h-4 w-4 rounded border-slate-300 dark:border-white/20 bg-white dark:bg-[#0B0D11] text-blue-500"
                  />
                </label>
              </div>
            </div>
          )}

          {/* === ADBLOCK === */}
          {activeSection === 'adblock' && (
            <div className="space-y-6">
              <div>
                <h2 className="text-[20px] font-semibold text-slate-900 dark:text-white">
                  AdBlock та білий список сайтів
                </h2>
                <p className="text-[13px] text-slate-500 dark:text-[#8F98A8] mt-1">
                  Блокування рекламних мереж, банерів і трекерів на мережевому рівні (EasyList + EasyPrivacy).
                </p>
              </div>

              <div className="rounded-xl bg-white dark:bg-[#12161E] border border-slate-200 dark:border-white/[0.07] divide-y divide-slate-200 dark:divide-white/[0.06]">
                <label className="flex items-center justify-between gap-4 p-4 cursor-pointer">
                  <div>
                    <div className="text-[14px] font-medium text-slate-900 dark:text-white">
                      Увімкнути вбудований AdBlock
                    </div>
                    <div className="text-[12px] text-slate-500 dark:text-[#8F98A8]">
                      Активних правил:{' '}
                      <span className="font-mono tabular-nums">{adblockStatus?.rulesLoaded ?? 0}</span> · Всього заблоковано:{' '}
                      <span className="font-mono tabular-nums">{adblockStatus?.totalBlocked ?? 0}</span>
                    </div>
                  </div>
                  <input
                    type="checkbox"
                    checked={settings.adblockEnabled}
                    onChange={(e) => onUpdateSettings({ adblockEnabled: e.target.checked })}
                    className="h-4 w-4 rounded border-slate-300 dark:border-white/20 bg-white dark:bg-[#0B0D11] text-blue-500"
                  />
                </label>

                <label className="flex items-center justify-between gap-4 p-4 cursor-pointer">
                  <div>
                    <div className="text-[14px] font-medium text-slate-900 dark:text-white">
                      Автоматично оновлювати списки фільтрів
                    </div>
                    <div className="text-[12px] text-slate-500 dark:text-[#8F98A8]">
                      Періодичне оновлення EasyList та EasyPrivacy у фоновому режимі
                    </div>
                  </div>
                  <input
                    type="checkbox"
                    checked={settings.adblockAutoUpdate}
                    onChange={(e) => onUpdateSettings({ adblockAutoUpdate: e.target.checked })}
                    className="h-4 w-4 rounded border-slate-300 dark:border-white/20 bg-white dark:bg-[#0B0D11] text-blue-500"
                  />
                </label>

                <div className="flex items-center justify-between gap-4 p-4">
                  <div>
                    <div className="text-[14px] font-medium text-slate-900 dark:text-white">
                      Оновлення баз фільтрів
                    </div>
                    <div className="text-[12px] text-slate-500 dark:text-[#8F98A8]">
                      {adblockStatus?.lastUpdatedAt
                        ? `Останнє оновлення: ${new Date(adblockStatus.lastUpdatedAt).toLocaleString('uk-UA')}`
                        : 'Використовується локально кешований набір правил'}
                    </div>
                  </div>
                  <button
                    type="button"
                    disabled={updatingFilters}
                    onClick={async () => {
                      setUpdatingFilters(true);
                      try {
                        await onUpdateAdBlockFilters();
                        notify('Списки фільтрів EasyList та EasyPrivacy оновлено');
                      } finally {
                        setUpdatingFilters(false);
                      }
                    }}
                    className="flex items-center gap-2 px-4 py-2 rounded-lg bg-blue-600 hover:bg-blue-500 text-white text-[13px] font-medium"
                  >
                    <RefreshCw className={`w-4 h-4 ${updatingFilters ? 'animate-spin' : ''}`} />
                    <span>{updatingFilters ? 'Оновлення...' : 'Оновити зараз'}</span>
                  </button>
                </div>
              </div>

              {/* Білий список доменів */}
              <div className="rounded-xl bg-white dark:bg-[#12161E] border border-slate-200 dark:border-white/[0.07] p-4 space-y-3">
                <div>
                  <div className="text-[14px] font-medium text-slate-900 dark:text-white">
                    Винятки AdBlock (Whitelist)
                  </div>
                  <div className="text-[12px] text-slate-500 dark:text-[#8F98A8]">
                    Для цих доменів блокувальник реклами повністю вимкнено. Інші сайти залишаються захищеними.
                  </div>
                </div>

                <form
                  onSubmit={async (e) => {
                    e.preventDefault();
                    if (!newWhitelistDomain.trim()) return;
                    await onToggleAdBlockDomain(newWhitelistDomain.trim(), true);
                    setNewWhitelistDomain('');
                    notify('Домен додано до винятків');
                  }}
                  className="flex items-center gap-2"
                >
                  <input
                    type="text"
                    value={newWhitelistDomain}
                    onChange={(e) => setNewWhitelistDomain(e.target.value)}
                    placeholder="Наприклад: pravda.com.ua"
                    className="flex-1 rounded-lg bg-slate-50 dark:bg-[#0B0D11] border border-slate-200 dark:border-white/[0.1] px-3 py-2 text-[13px] font-mono text-slate-900 dark:text-white"
                  />
                  <button
                    type="submit"
                    className="flex items-center gap-1.5 px-4 py-2 rounded-lg bg-slate-100 hover:bg-slate-200 dark:bg-[#1C222E] dark:hover:bg-[#252D3D] border border-slate-200 dark:border-white/[0.08] text-[13px] text-slate-900 dark:text-white"
                  >
                    <Plus className="w-4 h-4" />
                    <span>Додати домен</span>
                  </button>
                </form>

                <div className="divide-y divide-slate-200 dark:divide-white/[0.05]">
                  {!adblockStatus || adblockStatus.whitelist.length === 0 ? (
                    <div className="py-4 text-[12px] text-slate-400 dark:text-[#64748B]">
                      Список винятків порожній — AdBlock працює на всіх сайтах.
                    </div>
                  ) : (
                    adblockStatus.whitelist.map((domain) => (
                      <div key={domain} className="flex items-center justify-between py-2.5 text-[13px]">
                        <span className="font-mono text-slate-900 dark:text-[#E6EAF2]">{domain}</span>
                        <button
                          type="button"
                          onClick={async () => {
                            await onToggleAdBlockDomain(domain, false);
                            notify(`AdBlock знову ввімкнено для ${domain}`);
                          }}
                          className="flex items-center gap-1 text-[12px] text-red-600 dark:text-red-400 hover:text-red-500 dark:hover:text-red-300"
                        >
                          <Trash2 className="w-3.5 h-3.5" />
                          <span>Видалити з винятків</span>
                        </button>
                      </div>
                    ))
                  )}
                </div>
              </div>
            </div>
          )}

          {/* === MEMORY SAVER === */}
          {activeSection === 'memory' && (
            <div className="space-y-6">
              <div>
                <h2 className="text-[20px] font-semibold text-slate-900 dark:text-white">
                  Memory Saver та оптимізація RAM
                </h2>
                <p className="text-[13px] text-slate-500 dark:text-[#8F98A8] mt-1">
                  Керування призупиненням (`TrySuspend`) та вивантаженням неактивних вкладок WebView2 із пам’яті.
                </p>
              </div>

              <div className="grid grid-cols-1 md:grid-cols-3 gap-3">
                {(
                  [
                    {
                      id: 'off',
                      title: 'Вимкнено',
                      desc: 'Усі відкриті вкладки залишаються активними в RAM постійно.',
                    },
                    {
                      id: 'standard',
                      title: 'Стандартний режим',
                      desc: 'Призупинення через 10 хв бездіяльності, вивантаження через 30 хв.',
                    },
                    {
                      id: 'aggressive',
                      title: 'Агресивний режим',
                      desc: 'Максимальна економія RAM: призупинення через 2 хв, вивантаження через 5 хв.',
                    },
                  ] as Array<{ id: MemorySaverMode; title: string; desc: string }>
                ).map((mode) => {
                  const selected = settings.memorySaverMode === mode.id;
                  return (
                    <button
                      key={mode.id}
                      type="button"
                      onClick={() => {
                        const times =
                          mode.id === 'aggressive'
                            ? { memorySaverSuspendMins: 2, memorySaverDiscardMins: 5 }
                            : mode.id === 'standard'
                              ? { memorySaverSuspendMins: 10, memorySaverDiscardMins: 30 }
                              : {};
                        onUpdateSettings({ memorySaverMode: mode.id, ...times });
                      }}
                      className={`p-4 rounded-xl border text-left transition-colors ${
                        selected
                          ? 'bg-blue-500/10 border-blue-500/60 text-slate-900 dark:bg-blue-500/15 dark:text-white'
                          : 'bg-white dark:bg-[#12161E] border-slate-200 dark:border-white/[0.07] text-slate-700 dark:text-[#CBD5E1] hover:border-slate-300 dark:hover:border-white/[0.15]'
                      }`}
                    >
                      <div className="font-semibold text-[14px]">{mode.title}</div>
                      <p className="text-[12px] text-slate-500 dark:text-[#8F98A8] mt-1.5 leading-relaxed">
                        {mode.desc}
                      </p>
                    </button>
                  );
                })}
              </div>

              {settings.memorySaverMode !== 'off' && (
                <div className="rounded-xl bg-white dark:bg-[#12161E] border border-slate-200 dark:border-white/[0.07] p-4 grid grid-cols-1 sm:grid-cols-2 gap-4">
                  <div>
                    <label className="block text-[12px] text-slate-500 dark:text-[#8F98A8] mb-1">
                      Тайм-аут призупинення (хв)
                    </label>
                    <input
                      type="number"
                      min={1}
                      max={240}
                      value={settings.memorySaverSuspendMins}
                      onChange={(e) =>
                        onUpdateSettings({
                          memorySaverSuspendMins: Math.max(1, Number(e.target.value) || 1),
                        })
                      }
                      className="w-full rounded-lg bg-slate-50 dark:bg-[#0B0D11] border border-slate-200 dark:border-white/[0.1] px-3 py-2 text-[13px] font-mono text-slate-900 dark:text-white"
                    />
                  </div>
                  <div>
                    <label className="block text-[12px] text-slate-500 dark:text-[#8F98A8] mb-1">
                      Тайм-аут повного вивантаження (хв)
                    </label>
                    <input
                      type="number"
                      min={2}
                      max={480}
                      value={settings.memorySaverDiscardMins}
                      onChange={(e) =>
                        onUpdateSettings({
                          memorySaverDiscardMins: Math.max(2, Number(e.target.value) || 2),
                        })
                      }
                      className="w-full rounded-lg bg-slate-50 dark:bg-[#0B0D11] border border-slate-200 dark:border-white/[0.1] px-3 py-2 text-[13px] font-mono text-slate-900 dark:text-white"
                    />
                  </div>
                </div>
              )}

              <div className="rounded-xl bg-white dark:bg-[#12161E] border border-slate-200 dark:border-white/[0.07] p-4 text-[13px] text-slate-600 dark:text-[#94A3B8] space-y-1.5">
                <div className="font-medium text-slate-900 dark:text-white">
                  Захищені вкладки (ніколи не вивантажуються)
                </div>
                <p>
                  Вкладки, які відтворюють аудіо, мають активне завантаження файлів, закріплені користувачем або активні в цей момент, захищені від призупинення та вивантаження.
                </p>
              </div>
            </div>
          )}

          {/* === ПРИВАТНІСТЬ === */}
          {activeSection === 'privacy' && (
            <div className="space-y-6">
              <div>
                <h2 className="text-[20px] font-semibold text-slate-900 dark:text-white">
                  Приватність, Cookies та дані сайтів
                </h2>
                <p className="text-[13px] text-slate-500 dark:text-[#8F98A8] mt-1">
                  Керування заголовками відстеження, сторонніми cookies та локально збереженими даними сайтів.
                </p>
              </div>

              <div className="rounded-xl bg-white dark:bg-[#12161E] border border-slate-200 dark:border-white/[0.07] divide-y divide-slate-200 dark:divide-white/[0.06]">
                <label className="flex items-center justify-between gap-4 p-4 cursor-pointer">
                  <div>
                    <div className="text-[14px] font-medium text-slate-900 dark:text-white">
                      Надсилати запит Do Not Track (DNT: 1)
                    </div>
                    <div className="text-[12px] text-slate-500 dark:text-[#8F98A8]">
                      Повідомляє вебсайтам про заборону збору поведінкової аналітики
                    </div>
                  </div>
                  <input
                    type="checkbox"
                    checked={settings.doNotTrack}
                    onChange={(e) => onUpdateSettings({ doNotTrack: e.target.checked })}
                    className="h-4 w-4 rounded border-slate-300 dark:border-white/20 bg-white dark:bg-[#0B0D11] text-blue-500"
                  />
                </label>

                <label className="flex items-center justify-between gap-4 p-4 cursor-pointer">
                  <div>
                    <div className="text-[14px] font-medium text-slate-900 dark:text-white">
                      Блокувати сторонні Cookies
                    </div>
                    <div className="text-[12px] text-slate-500 dark:text-[#8F98A8]">
                      Обмежує міжсайтове стеження через сторонні рекламні та аналітичні домени
                    </div>
                  </div>
                  <input
                    type="checkbox"
                    checked={settings.blockThirdPartyCookies}
                    onChange={(e) =>
                      onUpdateSettings({ blockThirdPartyCookies: e.target.checked })
                    }
                    className="h-4 w-4 rounded border-slate-300 dark:border-white/20 bg-white dark:bg-[#0B0D11] text-blue-500"
                  />
                </label>
              </div>

              <div className="rounded-xl bg-white dark:bg-[#12161E] border border-slate-200 dark:border-white/[0.07] p-4 space-y-3">
                <div className="flex items-center justify-between">
                  <div>
                    <div className="text-[14px] font-medium text-slate-900 dark:text-white">
                      Збережені Cookies і дані сайтів
                    </div>
                    <div className="text-[12px] text-slate-500 dark:text-[#8F98A8]">
                      Домени, які зберігали дані під час роботи в браузері
                    </div>
                  </div>
                  <button
                    type="button"
                    onClick={async () => {
                      await onClearSiteData();
                      notify('Усі збережені дані сайтів очищено');
                    }}
                    className="px-3 py-1.5 rounded-lg bg-red-500/15 hover:bg-red-500/25 text-red-600 dark:text-red-300 text-[12px] font-medium"
                  >
                    Очистити все
                  </button>
                </div>

                <div className="divide-y divide-slate-200 dark:divide-white/[0.05]">
                  {siteData.length === 0 ? (
                    <div className="py-4 text-[12px] text-slate-400 dark:text-[#64748B]">
                      Немає збережених даних сайтів
                    </div>
                  ) : (
                    siteData.map((sd) => (
                      <div key={sd.domain} className="flex items-center justify-between py-2 text-[13px]">
                        <div>
                          <span className="font-mono text-slate-900 dark:text-white">{sd.domain}</span>
                          <span className="text-[11px] text-slate-500 dark:text-[#8F98A8] ml-2 font-mono tabular-nums">
                            {sd.cookieCount} запис(ів) · {sd.totalBytes} Б
                          </span>
                        </div>
                        <button
                          type="button"
                          onClick={async () => {
                            await onClearSiteData(sd.domain);
                            notify(`Дані для ${sd.domain} видалено`);
                          }}
                          className="text-[12px] text-slate-500 hover:text-red-500 dark:text-[#8F98A8] dark:hover:text-red-400"
                        >
                          Видалити
                        </button>
                      </div>
                    ))
                  )}
                </div>
              </div>
            </div>
          )}

          {/* === ЗАВАНТАЖЕННЯ ТА ЕКСПОРТ === */}
          {activeSection === 'downloads' && (
            <div className="space-y-6">
              <div>
                <h2 className="text-[20px] font-semibold text-slate-900 dark:text-white">
                  Завантаження та резервне копіювання
                </h2>
                <p className="text-[13px] text-slate-500 dark:text-[#8F98A8] mt-1">
                  Налаштування каталогу завантажень та повний експорт/імпорт даних Vortex Browser.
                </p>
              </div>

              <div className="rounded-xl bg-white dark:bg-[#12161E] border border-slate-200 dark:border-white/[0.07] divide-y divide-slate-200 dark:divide-white/[0.06]">
                <div className="flex items-center justify-between gap-4 p-4">
                  <div>
                    <div className="text-[14px] font-medium text-slate-900 dark:text-white">
                      Папка завантажень за замовчуванням
                    </div>
                    <div className="text-[12px] text-slate-500 dark:text-[#8F98A8]">
                      Шлях на локальному диску
                    </div>
                  </div>
                  <input
                    type="text"
                    value={settings.downloadDir}
                    onChange={(e) => onUpdateSettings({ downloadDir: e.target.value })}
                    className="w-64 rounded-lg bg-slate-50 dark:bg-[#0B0D11] border border-slate-200 dark:border-white/[0.1] px-3 py-1.5 text-[13px] font-mono text-slate-900 dark:text-white"
                  />
                </div>

                <label className="flex items-center justify-between gap-4 p-4 cursor-pointer">
                  <div>
                    <div className="text-[14px] font-medium text-slate-900 dark:text-white">
                      Завжди запитувати місце збереження
                    </div>
                    <div className="text-[12px] text-slate-500 dark:text-[#8F98A8]">
                      Показувати діалог вибору файлу перед початком завантаження
                    </div>
                  </div>
                  <input
                    type="checkbox"
                    checked={settings.askDownloadLocation}
                    onChange={(e) => onUpdateSettings({ askDownloadLocation: e.target.checked })}
                    className="h-4 w-4 rounded border-slate-300 dark:border-white/20 bg-white dark:bg-[#0B0D11] text-blue-500"
                  />
                </label>
              </div>

              <div className="rounded-xl bg-white dark:bg-[#12161E] border border-slate-200 dark:border-white/[0.07] p-4 flex flex-wrap items-center justify-between gap-4">
                <div>
                  <div className="text-[14px] font-medium text-slate-900 dark:text-white">
                    Експорт та імпорт даних браузера
                  </div>
                  <div className="text-[12px] text-slate-500 dark:text-[#8F98A8]">
                    Збереження або перенесення налаштувань, закладок та білого списку AdBlock у JSON
                  </div>
                </div>
                <div className="flex items-center gap-2">
                  <button
                    type="button"
                    onClick={async () => {
                      const data = await onExportAllData();
                      const blob = new Blob([JSON.stringify(data, null, 2)], {
                        type: 'application/json',
                      });
                      const a = document.createElement('a');
                      a.href = URL.createObjectURL(blob);
                      a.download = `vortex-backup-${new Date().toISOString().slice(0, 10)}.json`;
                      a.click();
                      URL.revokeObjectURL(a.href);
                    }}
                    className="flex items-center gap-2 px-4 py-2 rounded-lg bg-slate-100 hover:bg-slate-200 dark:bg-[#1C222E] dark:hover:bg-[#252D3D] border border-slate-200 dark:border-white/[0.08] text-[13px] text-slate-900 dark:text-white"
                  >
                    <Download className="w-4 h-4" />
                    <span>Експорт JSON</span>
                  </button>
                  <button
                    type="button"
                    onClick={() => importInputRef.current?.click()}
                    className="flex items-center gap-2 px-4 py-2 rounded-lg bg-blue-600 hover:bg-blue-500 text-white text-[13px] font-medium"
                  >
                    <Upload className="w-4 h-4" />
                    <span>Імпорт JSON</span>
                  </button>
                  <input
                    ref={importInputRef}
                    type="file"
                    accept="application/json,.json"
                    className="hidden"
                    onChange={async (e) => {
                      const file = e.target.files?.[0];
                      if (!file) return;
                      const text = await file.text();
                      await onImportAllData(JSON.parse(text));
                      notify('Дані браузера успішно імпортовано');
                      e.target.value = '';
                    }}
                  />
                </div>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
};
