import React, { useEffect, useRef, useState } from 'react';
import { Activity, Play, ShieldCheck, Upload } from 'lucide-react';
import type { BenchmarkRunRecord, LiveBenchItem, WindowsBenchRow } from '../types/browser';

interface BenchmarksPageProps {
  runs: BenchmarkRunRecord[];
  onRefreshRuns: () => void;
  onRunLiveBenchmark: (urls: string[]) => Promise<void>;
  onImportReport: (payload: Record<string, unknown>) => Promise<void>;
}

export const BenchmarksPage: React.FC<BenchmarksPageProps> = ({
  runs,
  onRefreshRuns,
  onRunLiveBenchmark,
  onImportReport,
}) => {
  const [urlsText, setUrlsText] = useState(
    'https://example.com\nhttps://uk.wikipedia.org\nhttps://news.ycombinator.com\nhttps://www.rust-lang.org'
  );
  const [running, setRunning] = useState(false);
  const [error, setError] = useState('');
  const fileInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    onRefreshRuns();
  }, [onRefreshRuns]);

  const latestLiveRun = runs.find((r) => r.payload?.type === 'live_adblock_network_bench');
  const latestWindowsRun = runs.find(
    (r) =>
      Array.isArray(r.payload?.results) &&
      r.payload.results.length > 0 &&
      'workingSetMb' in r.payload.results[0]
  );

  const liveItems = (latestLiveRun?.payload?.results || []) as LiveBenchItem[];
  const winItems = (latestWindowsRun?.payload?.results || []) as WindowsBenchRow[];

  return (
    <div className="h-full w-full overflow-y-auto bg-[#F4F6FA] dark:bg-[#0B0D11] p-8 text-slate-900 dark:text-[#E6EAF2]">
      <div className="max-w-[980px] mx-auto space-y-8">
        {/* Заголовок */}
        <div className="flex flex-wrap items-start justify-between gap-4 border-b border-slate-200 dark:border-white/[0.07] pb-6">
          <div>
            <h1 className="text-[24px] font-semibold text-slate-900 dark:text-white">
              Тестування продуктивності та порівняння з Google Chrome
            </h1>
            <p className="text-[13px] text-slate-500 dark:text-[#8F98A8] mt-1 max-w-2xl leading-relaxed">
              Vortex Browser не використовує вигаданих оцінок продуктивності. Тут відображаються лише реальні вимірювання мережевого AdBlock та імпортовані звіти Windows 11.
            </p>
          </div>
          <div className="flex items-center gap-2 shrink-0">
            <button
              type="button"
              onClick={() => fileInputRef.current?.click()}
              className="flex items-center gap-2 px-4 py-2 rounded-lg bg-white hover:bg-slate-50 dark:bg-[#151922] dark:hover:bg-[#1C222E] border border-slate-200 dark:border-white/[0.1] text-[13px] font-medium text-slate-900 dark:text-white whitespace-nowrap"
            >
              <Upload className="w-4 h-4" />
              <span>Імпортувати звіт Windows 11 (.json)</span>
            </button>
            <input
              ref={fileInputRef}
              type="file"
              accept="application/json,.json"
              className="hidden"
              onChange={async (e) => {
                const file = e.target.files?.[0];
                if (!file) return;
                try {
                  const json = JSON.parse(await file.text());
                  await onImportReport(json);
                } catch {
                  setError('Некоректний JSON-файл звіту');
                }
                e.target.value = '';
              }}
            />
          </div>
        </div>

        {error && (
          <div className="rounded-lg bg-red-500/15 border border-red-500/30 px-4 py-2.5 text-[13px] text-red-700 dark:text-red-300">
            {error}
          </div>
        )}

        {/* Секція 1 */}
        <section className="rounded-xl bg-white dark:bg-[#12161E] border border-slate-200 dark:border-white/[0.07] p-5 space-y-4">
          <div className="flex flex-wrap items-center justify-between gap-4">
            <div className="flex items-center gap-2.5">
              <ShieldCheck className="w-5 h-5 text-emerald-600 dark:text-emerald-400" />
              <div>
                <h2 className="text-[16px] font-semibold text-slate-900 dark:text-white">
                  Живе вимірювання завантаження сторінок та впливу AdBlock
                </h2>
                <p className="text-[12px] text-slate-500 dark:text-[#8F98A8]">
                  Виконує реальні HTTPS-запити та вимірює затримку, заблоковані ресурси і економію трафіку.
                </p>
              </div>
            </div>
            <button
              type="button"
              disabled={running}
              onClick={async () => {
                setError('');
                setRunning(true);
                try {
                  const urls = urlsText
                    .split(/\r?\n/)
                    .map((u) => u.trim())
                    .filter(Boolean);
                  await onRunLiveBenchmark(urls);
                } catch (err: unknown) {
                  setError(err instanceof Error ? err.message : 'Помилка виконання тесту');
                } finally {
                  setRunning(false);
                }
              }}
              className="flex items-center gap-2 px-4 py-2 rounded-lg bg-blue-600 hover:bg-blue-500 disabled:opacity-50 text-white text-[13px] font-medium whitespace-nowrap"
            >
              <Play className="w-4 h-4" />
              <span>{running ? 'Вимірювання...' : 'Запустити живий тест'}</span>
            </button>
          </div>

          <textarea
            rows={3}
            value={urlsText}
            onChange={(e) => setUrlsText(e.target.value)}
            className="w-full rounded-lg bg-slate-50 dark:bg-[#0B0D11] border border-slate-200 dark:border-white/[0.08] p-3 text-[12px] font-mono text-slate-800 dark:text-[#CBD5E1] focus:outline-none focus:border-blue-500"
          />

          {liveItems.length > 0 && (
            <div className="overflow-x-auto">
              <table className="w-full text-left border-collapse text-[13px]">
                <thead>
                  <tr className="border-b border-slate-200 dark:border-white/[0.08] text-[12px] text-slate-500 dark:text-[#8F98A8]">
                    <th className="py-2 pr-4 font-medium">URL</th>
                    <th className="py-2 px-3 font-medium text-right">HTTP</th>
                    <th className="py-2 px-3 font-medium text-right">Час завантаження</th>
                    <th className="py-2 px-3 font-medium text-right">Заблоковано</th>
                    <th className="py-2 px-3 font-medium text-right">До / Після фільтру</th>
                    <th className="py-2 pl-3 font-medium text-right">Економія</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-slate-200 dark:divide-white/[0.05] font-mono tabular-nums">
                  {liveItems.map((item) => (
                    <tr key={item.url} className="hover:bg-slate-50 dark:hover:bg-white/[0.02]">
                      <td className="py-2.5 pr-4 text-slate-900 dark:text-[#E6EAF2] truncate max-w-[260px]">
                        {item.url}
                      </td>
                      <td className="py-2.5 px-3 text-right text-slate-500 dark:text-[#94A3B8]">
                        {item.status}
                      </td>
                      <td className="py-2.5 px-3 text-right text-slate-900 dark:text-white">
                        {item.fetchMs} мс
                      </td>
                      <td className="py-2.5 px-3 text-right text-emerald-600 dark:text-emerald-400">
                        {item.blockedRequests}
                      </td>
                      <td className="py-2.5 px-3 text-right text-slate-500 dark:text-[#94A3B8]">
                        {(item.rawBytes / 1024).toFixed(1)} / {(item.filteredBytes / 1024).toFixed(1)} КБ
                      </td>
                      <td className="py-2.5 pl-3 text-right text-blue-600 dark:text-blue-400">
                        {item.savedPercent}%
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </section>

        {/* Секція 2 */}
        <section className="rounded-xl bg-white dark:bg-[#12161E] border border-slate-200 dark:border-white/[0.07] p-5 space-y-4">
          <div className="flex items-center gap-2.5">
            <Activity className="w-5 h-5 text-blue-600 dark:text-blue-400" />
            <div>
              <h2 className="text-[16px] font-semibold text-slate-900 dark:text-white">
                Порівняльний звіт Windows 11: Vortex Browser vs Google Chrome (1, 5, 10, 20, 30 вкладок)
              </h2>
              <p className="text-[12px] text-slate-500 dark:text-[#8F98A8]">
                Генерується скриптом <span className="font-mono">benchmarks/compare_vortex_vs_chrome.ps1</span> на реальній Windows 11 системі.
              </p>
            </div>
          </div>

          {winItems.length === 0 ? (
            <div className="rounded-lg bg-slate-50 dark:bg-[#0B0D11] border border-slate-200 dark:border-white/[0.06] p-4 space-y-2 text-[13px] text-slate-600 dark:text-[#94A3B8]">
              <div className="font-medium text-slate-900 dark:text-white">
                Звіт Windows 11 ще не завантажено
              </div>
              <p>
                Щоб отримати реальні цифри RAM (Working Set / Private Bytes), CPU, часу запуску та ефекту Memory Saver на Windows 11, виконайте у PowerShell:
              </p>
              <pre className="rounded-md bg-slate-200/70 dark:bg-[#151922] p-3 font-mono text-[12px] text-blue-700 dark:text-blue-300 overflow-x-auto">
                powershell -ExecutionPolicy Bypass -File .\benchmarks\compare_vortex_vs_chrome.ps1
              </pre>
              <p className="text-[12px] text-slate-500 dark:text-[#64748B]">
                Після завершення натисніть «Імпортувати звіт Windows 11 (.json)» вище та оберіть файл <span className="font-mono">benchmarks/vortex_vs_chrome_report.json</span>.
              </p>
            </div>
          ) : (
            <div className="space-y-3">
              {latestWindowsRun?.payload?.os && (
                <div className="text-[12px] text-slate-500 dark:text-[#8F98A8]">
                  Система: <span className="text-slate-900 dark:text-white">{latestWindowsRun.payload.os}</span> · CPU:{' '}
                  <span className="text-slate-900 dark:text-white">{latestWindowsRun.payload.cpu}</span>
                </div>
              )}
              <div className="overflow-x-auto">
                <table className="w-full text-left border-collapse text-[13px]">
                  <thead>
                    <tr className="border-b border-slate-200 dark:border-white/[0.08] text-[12px] text-slate-500 dark:text-[#8F98A8]">
                      <th className="py-2 pr-4 font-medium">Браузер</th>
                      <th className="py-2 px-3 font-medium text-right">Вкладок</th>
                      <th className="py-2 px-3 font-medium text-right">Working Set RAM</th>
                      <th className="py-2 px-3 font-medium text-right">Private Bytes</th>
                      <th className="py-2 px-3 font-medium text-right">CPU час</th>
                      <th className="py-2 pl-3 font-medium text-right">Запуск + Завантаження</th>
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-slate-200 dark:divide-white/[0.05] font-mono tabular-nums">
                    {winItems.map((row, idx) => (
                      <tr key={`${row.browser}-${row.tabs}-${idx}`} className="hover:bg-slate-50 dark:hover:bg-white/[0.02]">
                        <td className="py-2.5 pr-4 font-sans font-medium text-slate-900 dark:text-white">
                          {row.browser}
                        </td>
                        <td className="py-2.5 px-3 text-right text-slate-700 dark:text-[#CBD5E1]">
                          {row.tabs}
                        </td>
                        <td className="py-2.5 px-3 text-right text-blue-600 dark:text-blue-400">
                          {row.workingSetMb} МБ
                        </td>
                        <td className="py-2.5 px-3 text-right text-slate-700 dark:text-[#CBD5E1]">
                          {row.privateBytesMb} МБ
                        </td>
                        <td className="py-2.5 px-3 text-right text-slate-500 dark:text-[#94A3B8]">
                          {row.cpuSeconds} с
                        </td>
                        <td className="py-2.5 pl-3 text-right text-slate-700 dark:text-[#CBD5E1]">
                          {row.startupAndLoadS} с
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </div>
          )}
        </section>

        {/* Секція 3 */}
        <section className="rounded-xl bg-white dark:bg-[#12161E] border border-slate-200 dark:border-white/[0.07] p-5 grid grid-cols-1 md:grid-cols-3 gap-5 text-[13px]">
          <div className="space-y-1.5">
            <div className="font-semibold text-slate-900 dark:text-white">
              1. Спільний WebView2 Runtime
            </div>
            <p className="text-slate-500 dark:text-[#8F98A8] leading-relaxed">
              На Windows 11 WebView2 використовує системний runtime Edge без дублювання бінарників у пам’яті та важких розширень Chrome, а Rust UI споживає &lt; 25 МБ RAM.
            </p>
          </div>
          <div className="space-y-1.5">
            <div className="font-semibold text-slate-900 dark:text-white">
              2. Мережевий AdBlock до рендерингу
            </div>
            <p className="text-slate-500 dark:text-[#8F98A8] leading-relaxed">
              Запити реклами й трекерів перехоплюються в Rust через <span className="font-mono">WebResourceRequested</span> до завантаження JS-бандлів у процеси рендерингу.
            </p>
          </div>
          <div className="space-y-1.5">
            <div className="font-semibold text-slate-900 dark:text-white">
              3. TrySuspend + Tab Discarding
            </div>
            <p className="text-slate-500 dark:text-[#8F98A8] leading-relaxed">
              Неактивні вкладки заморожуються (<span className="font-mono">ICoreWebView2_13::TrySuspend</span>) або повністю вивантажуються з автоматичним відновленням при кліку.
            </p>
          </div>
        </section>
      </div>
    </div>
  );
};
