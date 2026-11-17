export type SearchEngineId = 'google' | 'duckduckgo' | 'brave' | 'bing';
export type MemorySaverMode = 'off' | 'standard' | 'aggressive';
export type SidePanelSection = 'bookmarks' | 'history' | 'downloads' | 'passwords';

export interface BrowserTab {
  id: string;
  webviewLabel: string;
  url: string;
  title: string;
  favicon: string | null;
  isLoading: boolean;
  isHttps: boolean;
  isPinned: boolean;
  isIncognito: boolean;
  isPlayingAudio: boolean;
  isMuted: boolean;
  isSuspended: boolean;
  isDiscarded: boolean;
  hasActiveDownload: boolean;
  blockedCount: number;
  blockedDomains?: string[];
  loadTimeMs?: number;
  rawBytes?: number;
  filteredBytes?: number;
  lastActiveAt: number;
  historyStack: string[];
  historyIndex: number;
  renderedHtml?: string;
  errorMessage?: string;
}

export interface BrowserSettings {
  theme: 'dark' | 'light';
  startupPage: string;
  searchEngine: SearchEngineId;
  showBookmarksBar: boolean;
  adblockEnabled: boolean;
  adblockAutoUpdate: boolean;
  memorySaverMode: MemorySaverMode;
  memorySaverSuspendMins: number;
  memorySaverDiscardMins: number;
  downloadDir: string;
  askDownloadLocation: boolean;
  doNotTrack: boolean;
  blockThirdPartyCookies: boolean;
  restoreTabsOnStartup: boolean;
}

export interface AdBlockStatus {
  domain: string;
  globalEnabled: boolean;
  isWhitelisted: boolean;
  isActiveOnSite: boolean;
  blockedOnTab: number;
  totalBlocked: number;
  whitelist: string[];
  rulesLoaded: number;
  lastUpdatedAt: number | null;
}

export interface BookmarkFolder {
  id: string;
  name: string;
  parentId: string | null;
  sortOrder: number;
}

export interface BookmarkItem {
  id: string;
  title: string;
  url: string;
  folderId: string | null;
  favicon: string | null;
  onBar: boolean;
  createdAt: number;
}

export interface HistoryItem {
  id: string;
  title: string;
  url: string;
  favicon: string | null;
  visitedAt: number;
  visitCount: number;
}

export interface DownloadRecord {
  id: string;
  filename: string;
  url: string;
  savePath: string;
  totalBytes: number;
  receivedBytes: number;
  speedBps?: number;
  status: 'downloading' | 'paused' | 'completed' | 'failed' | 'canceled';
  mimeType: string | null;
  supportsResume: boolean;
  startedAt: number;
  completedAt: number | null;
}

export interface VaultStatus {
  isConfigured: boolean;
  isUnlocked: boolean;
  entryCount: number;
}

export interface PasswordCredential {
  id: string;
  origin: string;
  title: string;
  username: string;
  password: string;
  notes: string | null;
  createdAt: number;
  updatedAt: number;
}

export interface SiteDataSummary {
  domain: string;
  cookieCount: number;
  totalBytes: number;
  updatedAt: number;
}

export interface BrowseResponse {
  finalUrl: string;
  title: string;
  favicon: string;
  isHttps: boolean;
  status: number;
  loadTimeMs: number;
  blockedCount: number;
  blockedDomains: string[];
  rawBytes: number;
  filteredBytes: number;
  contentType: string;
  html: string;
}

export interface LiveBenchItem {
  url: string;
  status: number;
  fetchMs: number;
  rawBytes: number;
  filteredBytes: number;
  blockedRequests: number;
  savedPercent: number;
}

export interface WindowsBenchRow {
  browser: string;
  tabs: number;
  startupAndLoadS: number;
  workingSetMb: number;
  privateBytesMb: number;
  cpuSeconds: number;
  processCount: number;
}

export interface BenchmarkRunRecord {
  id: string;
  createdAt: number;
  payload: {
    type?: string;
    generatedAt?: string;
    os?: string;
    cpu?: string;
    results?: Array<LiveBenchItem | WindowsBenchRow>;
  };
}
