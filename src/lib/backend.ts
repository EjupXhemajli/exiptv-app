/**
 * Backend-Bridge.
 *
 * In der Tauri-Shell laufen alle Aufrufe über `invoke` gegen das
 * Rust-Backend. Läuft die App im reinen Browser (Vite-Dev ohne Tauri,
 * UI-Entwicklung), springt ein In-Memory-Mock ein, damit die Oberfläche
 * ohne native Schicht entwickelt und geprüft werden kann.
 * Der Mock persistiert nichts und ist klar als solcher gekennzeichnet.
 */
import type {
  Channel, ChannelGroup, Diagnostics, Episode, HistoryEntry, ImportReport, Movie, MpvSetup, NewsItem,
  PlaybackStatus, Provider, ProviderKind, Season, Series,
} from "./types";

export const isTauri = "__TAURI_INTERNALS__" in window;

type Listener = (p: unknown) => void;

export interface PlayArgs {
  url: string;
  userAgent?: string;
  referer?: string;
  title?: string;
}

interface Backend {
  listProviders(): Promise<Provider[]>;
  addProvider(input: { name: string; kind: ProviderKind; source: string; username?: string; password?: string }): Promise<number>;
  deleteProvider(id: number): Promise<void>;
  importM3uFromUrl(providerId: number, url: string): Promise<ImportReport>;
  importM3uFromFile(providerId: number, path: string): Promise<ImportReport>;
  listGroups(providerId: number): Promise<ChannelGroup[]>;
  listChannels(providerId: number, groupId: number | null, limit: number, offset: number, sort?: string): Promise<Channel[]>;
  searchChannels(query: string): Promise<Channel[]>;
  renameProvider(id: number, name: string): Promise<void>;
  setProviderEnabled(id: number, enabled: boolean): Promise<void>;
  countChannels(providerId: number): Promise<number>;
  getSetting(key: string): Promise<string | null>;
  setSetting(key: string, value: string): Promise<void>;
  appDiagnostics(): Promise<Diagnostics>;
  onImportProgress(cb: Listener): Promise<() => void>;
  pickM3uFile(): Promise<string | null>;

  // Wiedergabe (Phase 4)
  ensurePlaybackReady(): Promise<void>;
  playbackLoad(args: PlayArgs): Promise<void>;
  playbackTogglePause(): Promise<void>;
  playbackStop(): Promise<void>;
  playbackSeek(seconds: number): Promise<void>;
  playbackSeekRelative(delta: number): Promise<void>;
  playbackSetVolume(volume: number): Promise<void>;
  playbackSelectAudio(id: number): Promise<void>;
  playbackSelectSubtitle(id: number | null): Promise<void>;
  playbackSetAspect(ratio: string | null): Promise<void>;
  playbackSetDeinterlace(on: boolean): Promise<void>;
  playbackSetBounds(x: number, y: number, width: number, height: number): Promise<void>;
  playbackShowVideo(visible: boolean): Promise<void>;
  playbackRequestTracks(): Promise<void>;
  onPlaybackStatus(cb: (s: PlaybackStatus) => void): Promise<() => void>;
  onPlaybackError(cb: (message: string) => void): Promise<() => void>;
  onMpvSetup(cb: (s: MpvSetup) => void): Promise<() => void>;

  // VOD
  movieCategories(providerId: number): Promise<string[]>;
  listMovies(providerId: number, category: string | null, limit: number, offset: number, sort?: string): Promise<Movie[]>;
  seriesCategories(providerId: number): Promise<string[]>;
  listSeries(providerId: number, category: string | null, limit: number, offset: number, sort?: string): Promise<Series[]>;
  listSeasons(seriesId: number): Promise<Season[]>;
  listEpisodes(seasonId: number): Promise<Episode[]>;
  countMovies(providerId: number): Promise<number>;
  countSeries(providerId: number): Promise<number>;
  importXtream(providerId: number): Promise<ImportReport>;
  loadSeriesSeasons(providerId: number, seriesDbId: number, seriesExternalId: number): Promise<void>;

  // Favoriten & Verlauf
  addFavorite(itemType: string, itemId: number): Promise<void>;
  removeFavorite(itemType: string, itemId: number): Promise<void>;
  isFavorite(itemType: string, itemId: number): Promise<boolean>;
  favoriteChannels(): Promise<Channel[]>;
  favoriteChannelIds(): Promise<number[]>;
  addHistory(itemType: string, itemId: number, name: string, url: string, logo: string | null): Promise<void>;
  listHistory(): Promise<HistoryEntry[]>;
  clearHistory(): Promise<void>;
  readLog(): Promise<string>;
  clearImageCache(): Promise<void>;
  cacheImage(url: string): Promise<string>;
  quitApp(): Promise<void>;
  fetchNews(perFeed?: number): Promise<NewsItem[]>;
  fetchArticleImage(url: string): Promise<string | null>;
}

function tauriBackend(): Backend {
  // Dynamische Importe, damit der Browser-Build ohne Tauri-Runtime lädt.
  const inv = async <T,>(cmd: string, args?: Record<string, unknown>) => {
    const { invoke } = await import("@tauri-apps/api/core");
    return invoke<T>(cmd, args);
  };
  return {
    listProviders: () => inv("list_providers"),
    addProvider: (input) => inv("add_provider", { input }),
    deleteProvider: (id) => inv("delete_provider", { id }),
    importM3uFromUrl: (providerId, url) => inv("import_m3u_from_url", { providerId, url }),
    importM3uFromFile: (providerId, path) => inv("import_m3u_from_file", { providerId, path }),
    listGroups: (providerId) => inv("list_groups", { providerId }),
    listChannels: (providerId, groupId, limit, offset, sort) =>
      inv("list_channels", { providerId, groupId, limit, offset, sort }),
    searchChannels: (query) => inv("search_channels", { query }),
    renameProvider: (id, name) => inv("rename_provider", { id, name }),
    setProviderEnabled: (id, enabled) => inv("set_provider_enabled", { id, enabled }),
    countChannels: (providerId) => inv("count_channels", { providerId }),
    getSetting: (key) => inv("get_setting", { key }),
    setSetting: (key, value) => inv("set_setting", { key, value }),
    appDiagnostics: () => inv("app_diagnostics"),
    onImportProgress: async (cb) => {
      const { listen } = await import("@tauri-apps/api/event");
      const un = await listen("import-progress", (e) => cb(e.payload));
      return () => un();
    },
    pickM3uFile: async () => {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const sel = await open({
        multiple: false,
        filters: [{ name: "Playlisten", extensions: ["m3u", "m3u8"] }],
      });
      return typeof sel === "string" ? sel : null;
    },

    ensurePlaybackReady: () => inv("ensure_playback_ready"),
    playbackLoad: (args) => inv("playback_load", { args }),
    playbackTogglePause: () => inv("playback_toggle_pause"),
    playbackStop: () => inv("playback_stop"),
    playbackSeek: (seconds) => inv("playback_seek", { seconds }),
    playbackSeekRelative: (delta) => inv("playback_seek_relative", { delta }),
    playbackSetVolume: (volume) => inv("playback_set_volume", { volume }),
    playbackSelectAudio: (id) => inv("playback_select_audio", { id }),
    playbackSelectSubtitle: (id) => inv("playback_select_subtitle", { id }),
    playbackSetAspect: (ratio) => inv("playback_set_aspect", { ratio }),
    playbackSetDeinterlace: (on) => inv("playback_set_deinterlace", { on }),
    playbackSetBounds: (x, y, width, height) => inv("playback_set_bounds", { x, y, width, height }),
    playbackShowVideo: (visible) => inv("playback_show_video", { visible }),
    playbackRequestTracks: () => inv("playback_request_tracks"),
    onPlaybackStatus: async (cb) => {
      const { listen } = await import("@tauri-apps/api/event");
      const un = await listen("playback-status", (e) => cb(e.payload as PlaybackStatus));
      return () => un();
    },
    onPlaybackError: async (cb) => {
      const { listen } = await import("@tauri-apps/api/event");
      const un = await listen("playback-error", (e) => cb((e.payload as { message: string }).message));
      return () => un();
    },
    onMpvSetup: async (cb) => {
      const { listen } = await import("@tauri-apps/api/event");
      const un = await listen("mpv-setup", (e) => cb(e.payload as MpvSetup));
      return () => un();
    },

    movieCategories: (providerId) => inv("movie_categories", { providerId }),
    listMovies: (providerId, category, limit, offset, sort) => inv("list_movies", { providerId, category, limit, offset, sort }),
    seriesCategories: (providerId) => inv("series_categories", { providerId }),
    listSeries: (providerId, category, limit, offset, sort) => inv("list_series", { providerId, category, limit, offset, sort }),
    listSeasons: (seriesId) => inv("list_seasons", { seriesId }),
    listEpisodes: (seasonId) => inv("list_episodes", { seasonId }),
    countMovies: (providerId) => inv("count_movies", { providerId }),
    countSeries: (providerId) => inv("count_series", { providerId }),
    importXtream: (providerId) => inv("import_xtream", { providerId }),
    loadSeriesSeasons: (providerId, seriesDbId, seriesExternalId) =>
      inv("load_series_seasons", { providerId, seriesDbId, seriesExternalId }),
    addFavorite: (itemType, itemId) => inv("add_favorite", { itemType, itemId }),
    removeFavorite: (itemType, itemId) => inv("remove_favorite", { itemType, itemId }),
    isFavorite: (itemType, itemId) => inv("is_favorite", { itemType, itemId }),
    favoriteChannels: () => inv("favorite_channels"),
    favoriteChannelIds: () => inv("favorite_channel_ids"),
    addHistory: (itemType, itemId, name, url, logo) => inv("add_history", { itemType, itemId, name, url, logo }),
    listHistory: () => inv("list_history"),
    clearHistory: () => inv("clear_history"),
    readLog: () => inv("read_log"),
    clearImageCache: () => inv("clear_image_cache"),
    cacheImage: async (url) => {
      const path = await inv<string>("cache_image", { url });
      // Lokalen Pfad in eine im WebView anzeigbare URL wandeln.
      const { convertFileSrc } = await import("@tauri-apps/api/core");
      return convertFileSrc(path);
    },
    quitApp: () => inv("quit_app"),
    fetchNews: (perFeed) => inv("fetch_news", { perFeed }),
    fetchArticleImage: (url) => inv("fetch_article_image", { url }),
  };
}

/** Browser-Mock für die UI-Entwicklung (nicht persistent). */
function mockBackend(): Backend {
  let nextId = 1;
  const providers: Provider[] = [];
  const groups = new Map<number, ChannelGroup[]>();
  const channels = new Map<number, Channel[]>();
  const settings = new Map<string, string>();
  const listeners: Listener[] = [];
  const statusListeners: ((s: PlaybackStatus) => void)[] = [];
  let mockPlaying = false;
  let mockPaused = false;
  let mockVolume = 100;
  let mockTitle: string | null = null;
  const mockFavorites = new Set<string>();
  const mockHistory = new Map<string, HistoryEntry>();
  const emitStatus = (s: PlaybackStatus) => statusListeners.forEach((l) => l(s));

  const emit = (p: unknown) => listeners.forEach((l) => l(p));
  const demoImport = (pid: number): ImportReport => {
    const gs: ChannelGroup[] = ["Nachrichten", "Unterhaltung", "Sport", "Doku"].map((name, i) => ({
      id: i + 1, provider_id: pid, name, sort_index: i, hidden: false,
    }));
    const cs: Channel[] = Array.from({ length: 240 }, (_, i) => ({
      id: i + 1, provider_id: pid, group_id: (i % 4) + 1,
      name: `Demo-Sender ${String(i + 1).padStart(3, "0")}`,
      url: `https://demo.invalid/stream/${i + 1}.m3u8`,
      tvg_id: `demo${i + 1}`, tvg_name: null, logo_url: null,
      channel_number: i + 1, is_radio: false, hidden: false, locked: false, sort_index: i,
    }));
    groups.set(pid, gs);
    channels.set(pid, cs);
    return { total_lines: 481, channels_parsed: 240, channels_skipped: 3, groups_found: 4, warnings: [], encoding: "UTF-8" };
  };

  return {
    listProviders: async () => [...providers],
    addProvider: async (input) => {
      const id = nextId++;
      const now = Math.floor(Date.now() / 1000);
      providers.push({
        id, name: input.name, kind: input.kind, source: input.source,
        username: input.username ?? null, secret_ref: null, enabled: true,
        auto_refresh_hours: null, epg_url: null, user_agent: null, referer: null,
        last_refresh_at: null, expires_at: null, max_connections: null,
        created_at: now, updated_at: now,
      });
      return id;
    },
    deleteProvider: async (id) => {
      const i = providers.findIndex((p) => p.id === id);
      if (i >= 0) providers.splice(i, 1);
      groups.delete(id); channels.delete(id);
    },
    importM3uFromUrl: async (pid) => {
      emit({ provider_id: pid, stage: "laden", channels: 0 });
      await new Promise((r) => setTimeout(r, 500));
      emit({ provider_id: pid, stage: "verarbeiten", channels: 0 });
      await new Promise((r) => setTimeout(r, 400));
      const rep = demoImport(pid);
      emit({ provider_id: pid, stage: "speichern", channels: rep.channels_parsed });
      await new Promise((r) => setTimeout(r, 300));
      emit({ provider_id: pid, stage: "fertig", channels: rep.channels_parsed });
      const p = providers.find((x) => x.id === pid);
      if (p) p.last_refresh_at = Math.floor(Date.now() / 1000);
      return rep;
    },
    importM3uFromFile: async function (pid) { return this.importM3uFromUrl(pid, ""); },
    listGroups: async (pid) => groups.get(pid) ?? [],
    listChannels: async (pid, gid, limit, offset, sort) => {
      let list = (channels.get(pid) ?? []).filter((c) => gid == null || c.group_id === gid);
      // Sortierung im Mock nachbilden.
      if (sort === "name_asc") list = [...list].sort((a, b) => a.name.localeCompare(b.name));
      else if (sort === "name_desc") list = [...list].sort((a, b) => b.name.localeCompare(a.name));
      else if (sort === "recently_added") list = [...list].sort((a, b) => b.id - a.id);
      else if (sort === "channel_number") list = [...list].sort((a, b) => (a.channel_number ?? 1e9) - (b.channel_number ?? 1e9));
      return list.slice(offset, offset + limit);
    },
    searchChannels: async (q) => {
      const needle = q.toLowerCase();
      return [...channels.values()].flat().filter((c) => c.name.toLowerCase().includes(needle)).slice(0, 100);
    },
    renameProvider: async (id, name) => {
      const p = providers.find((x) => x.id === id);
      if (p) p.name = name.trim();
    },
    setProviderEnabled: async (id, enabled) => {
      const p = providers.find((x) => x.id === id);
      if (p) p.enabled = enabled;
    },
    countChannels: async (pid) => (channels.get(pid) ?? []).length,
    getSetting: async (k) => settings.get(k) ?? null,
    setSetting: async (k, v) => { settings.set(k, v); },
    appDiagnostics: async () => ({
      app_version: "0.1.0 (Browser-Vorschau)", os: "browser", arch: "-", db_schema_version: 1,
    }),
    onImportProgress: async (cb) => { listeners.push(cb); return () => {
      const i = listeners.indexOf(cb); if (i >= 0) listeners.splice(i, 1);
    }; },
    pickM3uFile: async () => window.prompt("Pfad zur M3U-Datei (Browser-Vorschau):") ?? null,

    // Wiedergabe: im Browser nicht real möglich (keine native mpv-Schicht).
    // Der Mock meldet einen simulierten Statusverlauf, damit die UI-Logik
    // (Overlay, Steuerung) im Browser entwickelt werden kann.
    ensurePlaybackReady: async () => {
      statusListeners.length; // no-op
    },
    playbackLoad: async (args) => {
      mockPlaying = true;
      mockTitle = args.title ?? null;
      emitStatus({ state: "loading", position: null, duration: null, volume: mockVolume, recovering: false, recoveryStage: 0, title: mockTitle, tracks: [] });
      await new Promise((r) => setTimeout(r, 600));
      if (mockPlaying) emitStatus({ state: "playing", position: 0, duration: null, volume: mockVolume, recovering: false, recoveryStage: 0, title: mockTitle, tracks: [] });
    },
    playbackTogglePause: async () => {
      mockPaused = !mockPaused;
      emitStatus({ state: mockPaused ? "paused" : "playing", position: 0, duration: null, volume: mockVolume, recovering: false, recoveryStage: 0, title: mockTitle, tracks: [] });
    },
    playbackStop: async () => {
      mockPlaying = false; mockPaused = false;
      emitStatus({ state: "idle", position: null, duration: null, volume: mockVolume, recovering: false, recoveryStage: 0, title: null, tracks: [] });
    },
    playbackSeek: async () => {},
    playbackSeekRelative: async () => {},
    playbackSetVolume: async (v) => { mockVolume = v; },
    playbackSelectAudio: async () => {},
    playbackSelectSubtitle: async () => {},
    playbackSetAspect: async () => {},
    playbackSetDeinterlace: async () => {},
    playbackSetBounds: async () => {},
    playbackShowVideo: async () => {},
    playbackRequestTracks: async () => {},
    onPlaybackStatus: async (cb) => { statusListeners.push(cb); return () => {
      const i = statusListeners.indexOf(cb); if (i >= 0) statusListeners.splice(i, 1);
    }; },
    onPlaybackError: async () => () => {},
    onMpvSetup: async () => () => {},

    // VOD: im Mock leer (echte Daten kommen aus Xtream, Phase 5).
    movieCategories: async () => [],
    listMovies: async () => [],
    seriesCategories: async () => [],
    listSeries: async () => [],
    listSeasons: async () => [],
    listEpisodes: async () => [],
    countMovies: async () => 0,
    countSeries: async () => 0,
    importXtream: async () => ({
      total_lines: 0, channels_parsed: 0, channels_skipped: 0, groups_found: 0, warnings: [], encoding: null,
    }),
    loadSeriesSeasons: async () => {},
    addFavorite: async (itemType, itemId) => { mockFavorites.add(`${itemType}:${itemId}`); },
    removeFavorite: async (itemType, itemId) => { mockFavorites.delete(`${itemType}:${itemId}`); },
    isFavorite: async (itemType, itemId) => mockFavorites.has(`${itemType}:${itemId}`),
    favoriteChannels: async () => {
      const ids = new Set([...mockFavorites].filter((k) => k.startsWith("channel:")).map((k) => Number(k.split(":")[1])));
      return [...channels.values()].flat().filter((c) => ids.has(c.id));
    },
    favoriteChannelIds: async () =>
      [...mockFavorites].filter((k) => k.startsWith("channel:")).map((k) => Number(k.split(":")[1])),
    addHistory: async (itemType, itemId, name, url, logo) => {
      mockHistory.set(`${itemType}:${itemId}`, { item_type: itemType, item_id: itemId, name, url, logo_url: logo, watched_at: Date.now() / 1000 });
    },
    listHistory: async () => [...mockHistory.values()].sort((a, b) => b.watched_at - a.watched_at),
    clearHistory: async () => { mockHistory.clear(); },
    readLog: async () => "Mock-Log: keine Einträge im Browser-Modus.",
    clearImageCache: async () => {},
    cacheImage: async (url) => url,
    quitApp: async () => { window.close(); },
    fetchNews: async () => [],
    fetchArticleImage: async () => null,
  };
}

export const backend: Backend = isTauri ? tauriBackend() : mockBackend();
