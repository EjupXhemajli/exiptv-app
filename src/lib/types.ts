// Spiegel der Rust-Modelle (serde snake_case).
export type ProviderKind = "m3u_url" | "m3u_file" | "xtream" | "direct";

export interface Provider {
  id: number;
  name: string;
  kind: ProviderKind;
  source: string;
  username: string | null;
  secret_ref: string | null;
  enabled: boolean;
  auto_refresh_hours: number | null;
  epg_url: string | null;
  user_agent: string | null;
  referer: string | null;
  last_refresh_at: number | null;
  expires_at: number | null;
  max_connections: number | null;
  created_at: number;
  updated_at: number;
}

export interface ChannelGroup {
  id: number;
  provider_id: number;
  name: string;
  sort_index: number;
  hidden: boolean;
}

export interface Channel {
  id: number;
  provider_id: number;
  group_id: number | null;
  name: string;
  url: string;
  tvg_id: string | null;
  tvg_name: string | null;
  logo_url: string | null;
  channel_number: number | null;
  is_radio: boolean;
  hidden: boolean;
  locked: boolean;
  sort_index: number;
}

export interface ImportReport {
  total_lines: number;
  channels_parsed: number;
  channels_skipped: number;
  groups_found: number;
  movies_parsed?: number;
  series_parsed?: number;
  warnings: string[];
  encoding: string | null;
}

export interface ImportProgress {
  provider_id: number;
  stage: "laden" | "verarbeiten" | "speichern" | "fertig";
  channels: number;
}

export interface Diagnostics {
  app_version: string;
  os: string;
  arch: string;
  db_schema_version: number;
}

export type PlaybackState =
  | "idle" | "loading" | "playing" | "paused" | "buffering" | "ended" | "error";

export interface TrackInfo {
  id: number;
  kind: "video" | "audio" | "subtitle";
  language: string | null;
  title: string | null;
  selected: boolean;
}

export interface PlaybackStatus {
  state: PlaybackState;
  position: number | null;
  duration: number | null;
  volume: number;
  recovering: boolean;
  recoveryStage: number;
  title: string | null;
  tracks: TrackInfo[];
}

export interface HistoryEntry {
  item_type: string;
  item_id: number;
  name: string;
  url: string;
  logo_url: string | null;
  watched_at: number;
}

export interface MpvSetup {
  stage: string; // vorhanden|laden|entpacken|fertig|bereit|fehler
  message: string;
}

export interface Movie {
  id: number;
  provider_id: number;
  name: string;
  url: string;
  category: string | null;
  poster_url: string | null;
  backdrop_url: string | null;
  plot: string | null;
  year: number | null;
  genre: string | null;
  duration_s: number | null;
  rating: number | null;
  age_rating: string | null;
  director: string | null;
  cast: string | null;
  trailer_url: string | null;
}

export interface Series {
  id: number;
  provider_id: number;
  external_id: string | null;
  name: string;
  category: string | null;
  poster_url: string | null;
  backdrop_url: string | null;
  plot: string | null;
  year: number | null;
  genre: string | null;
  rating: number | null;
  age_rating: string | null;
}

export interface Season {
  id: number;
  series_id: number;
  number: number;
  name: string | null;
  episode_count: number;
}

export interface Episode {
  id: number;
  season_id: number;
  number: number;
  name: string | null;
  url: string;
  plot: string | null;
  duration_s: number | null;
  poster_url: string | null;
}
