//! Versionierte Schema-Migrationen.
//!
//! Jede Migration läuft in einer eigenen Transaktion und wird in
//! `schema_migrations` verbucht. Bereits angewendete Versionen werden
//! übersprungen (idempotent).

use crate::error::{CoreError, Result};
use rusqlite::Connection;

struct Migration {
    version: i64,
    name: &'static str,
    sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[Migration {
    version: 1,
    name: "grundschema",
    sql: r#"
CREATE TABLE user_profiles (
    id          INTEGER PRIMARY KEY,
    name        TEXT NOT NULL UNIQUE,
    avatar      TEXT,
    is_kids     INTEGER NOT NULL DEFAULT 0,
    pin_hash    TEXT,
    created_at  INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE TABLE providers (
    id                 INTEGER PRIMARY KEY,
    name               TEXT NOT NULL,
    kind               TEXT NOT NULL CHECK (kind IN ('m3u_url','m3u_file','xtream','direct')),
    source             TEXT NOT NULL,
    username           TEXT,
    secret_ref         TEXT,
    enabled            INTEGER NOT NULL DEFAULT 1,
    auto_refresh_hours INTEGER,
    epg_url            TEXT,
    user_agent         TEXT,
    referer            TEXT,
    last_refresh_at    INTEGER,
    expires_at         INTEGER,
    max_connections    INTEGER,
    created_at         INTEGER NOT NULL DEFAULT (unixepoch()),
    updated_at         INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE TABLE channel_groups (
    id          INTEGER PRIMARY KEY,
    provider_id INTEGER NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    sort_index  INTEGER NOT NULL DEFAULT 0,
    hidden      INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX idx_groups_provider ON channel_groups(provider_id);

CREATE TABLE channels (
    id              INTEGER PRIMARY KEY,
    provider_id     INTEGER NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
    group_id        INTEGER REFERENCES channel_groups(id) ON DELETE SET NULL,
    name            TEXT NOT NULL,
    search_name     TEXT NOT NULL,
    url             TEXT NOT NULL,
    tvg_id          TEXT,
    tvg_name        TEXT,
    logo_url        TEXT,
    channel_number  INTEGER,
    is_radio        INTEGER NOT NULL DEFAULT 0,
    catchup         TEXT,
    catchup_days    INTEGER,
    catchup_source  TEXT,
    timeshift       TEXT,
    user_agent      TEXT,
    referer         TEXT,
    hidden          INTEGER NOT NULL DEFAULT 0,
    locked          INTEGER NOT NULL DEFAULT 0,
    sort_index      INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX idx_channels_provider       ON channels(provider_id);
CREATE INDEX idx_channels_group          ON channels(provider_id, group_id);
CREATE INDEX idx_channels_search         ON channels(search_name);
CREATE INDEX idx_channels_tvg            ON channels(tvg_id);

CREATE TABLE epg_sources (
    id          INTEGER PRIMARY KEY,
    provider_id INTEGER REFERENCES providers(id) ON DELETE CASCADE,
    url         TEXT NOT NULL,
    kind        TEXT NOT NULL DEFAULT 'xmltv',
    time_offset_minutes INTEGER NOT NULL DEFAULT 0,
    last_refresh_at INTEGER
);

CREATE TABLE epg_channel_mappings (
    id            INTEGER PRIMARY KEY,
    channel_id    INTEGER NOT NULL REFERENCES channels(id) ON DELETE CASCADE,
    epg_source_id INTEGER NOT NULL REFERENCES epg_sources(id) ON DELETE CASCADE,
    epg_channel_id TEXT NOT NULL,
    manual        INTEGER NOT NULL DEFAULT 0,
    UNIQUE(channel_id, epg_source_id)
);

CREATE TABLE epg_programs (
    id             INTEGER PRIMARY KEY,
    epg_source_id  INTEGER NOT NULL REFERENCES epg_sources(id) ON DELETE CASCADE,
    epg_channel_id TEXT NOT NULL,
    start_at       INTEGER NOT NULL,
    end_at         INTEGER NOT NULL,
    title          TEXT NOT NULL,
    subtitle       TEXT,
    description    TEXT,
    category       TEXT,
    season         INTEGER,
    episode        INTEGER,
    rating         TEXT,
    icon_url       TEXT
);
CREATE INDEX idx_epg_lookup ON epg_programs(epg_source_id, epg_channel_id, start_at);
CREATE INDEX idx_epg_time   ON epg_programs(start_at, end_at);

CREATE TABLE movies (
    id           INTEGER PRIMARY KEY,
    provider_id  INTEGER NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
    external_id  TEXT,
    name         TEXT NOT NULL,
    search_name  TEXT NOT NULL,
    url          TEXT NOT NULL,
    category     TEXT,
    poster_url   TEXT,
    backdrop_url TEXT,
    plot         TEXT,
    year         INTEGER,
    genre        TEXT,
    duration_s   INTEGER,
    rating       REAL,
    age_rating   TEXT,
    director     TEXT,
    cast         TEXT,
    trailer_url  TEXT,
    added_at     INTEGER
);
CREATE INDEX idx_movies_provider ON movies(provider_id);
CREATE INDEX idx_movies_search   ON movies(search_name);

CREATE TABLE series (
    id           INTEGER PRIMARY KEY,
    provider_id  INTEGER NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
    external_id  TEXT,
    name         TEXT NOT NULL,
    search_name  TEXT NOT NULL,
    category     TEXT,
    poster_url   TEXT,
    backdrop_url TEXT,
    plot         TEXT,
    year         INTEGER,
    genre        TEXT,
    rating       REAL,
    age_rating   TEXT,
    added_at     INTEGER
);
CREATE INDEX idx_series_provider ON series(provider_id);
CREATE INDEX idx_series_search   ON series(search_name);

CREATE TABLE seasons (
    id        INTEGER PRIMARY KEY,
    series_id INTEGER NOT NULL REFERENCES series(id) ON DELETE CASCADE,
    number    INTEGER NOT NULL,
    name      TEXT,
    UNIQUE(series_id, number)
);

CREATE TABLE episodes (
    id         INTEGER PRIMARY KEY,
    season_id  INTEGER NOT NULL REFERENCES seasons(id) ON DELETE CASCADE,
    number     INTEGER NOT NULL,
    name       TEXT,
    url        TEXT NOT NULL,
    plot       TEXT,
    duration_s INTEGER,
    poster_url TEXT,
    UNIQUE(season_id, number)
);

CREATE TABLE favorites (
    id         INTEGER PRIMARY KEY,
    profile_id INTEGER NOT NULL REFERENCES user_profiles(id) ON DELETE CASCADE,
    item_type  TEXT NOT NULL CHECK (item_type IN ('channel','movie','series')),
    item_id    INTEGER NOT NULL,
    created_at INTEGER NOT NULL DEFAULT (unixepoch()),
    UNIQUE(profile_id, item_type, item_id)
);

CREATE TABLE watch_history (
    id         INTEGER PRIMARY KEY,
    profile_id INTEGER NOT NULL REFERENCES user_profiles(id) ON DELETE CASCADE,
    item_type  TEXT NOT NULL,
    item_id    INTEGER NOT NULL,
    watched_at INTEGER NOT NULL DEFAULT (unixepoch())
);
CREATE INDEX idx_history_profile ON watch_history(profile_id, watched_at DESC);

CREATE TABLE playback_progress (
    id          INTEGER PRIMARY KEY,
    profile_id  INTEGER NOT NULL REFERENCES user_profiles(id) ON DELETE CASCADE,
    item_type   TEXT NOT NULL,
    item_id     INTEGER NOT NULL,
    position_s  REAL NOT NULL,
    duration_s  REAL,
    finished    INTEGER NOT NULL DEFAULT 0,
    updated_at  INTEGER NOT NULL DEFAULT (unixepoch()),
    UNIQUE(profile_id, item_type, item_id)
);

CREATE TABLE recordings (
    id          INTEGER PRIMARY KEY,
    profile_id  INTEGER REFERENCES user_profiles(id) ON DELETE SET NULL,
    channel_id  INTEGER REFERENCES channels(id) ON DELETE SET NULL,
    title       TEXT NOT NULL,
    file_path   TEXT,
    start_at    INTEGER,
    end_at      INTEGER,
    status      TEXT NOT NULL DEFAULT 'geplant'
);

CREATE TABLE reminders (
    id          INTEGER PRIMARY KEY,
    profile_id  INTEGER NOT NULL REFERENCES user_profiles(id) ON DELETE CASCADE,
    channel_id  INTEGER NOT NULL REFERENCES channels(id) ON DELETE CASCADE,
    program_title TEXT NOT NULL,
    remind_at   INTEGER NOT NULL
);

CREATE TABLE parental_restrictions (
    id          INTEGER PRIMARY KEY,
    profile_id  INTEGER NOT NULL REFERENCES user_profiles(id) ON DELETE CASCADE,
    item_type   TEXT NOT NULL,
    item_ref    TEXT NOT NULL,
    UNIQUE(profile_id, item_type, item_ref)
);

CREATE TABLE app_settings (
    key        TEXT PRIMARY KEY,
    value      TEXT NOT NULL,
    updated_at INTEGER NOT NULL DEFAULT (unixepoch())
);

CREATE TABLE image_cache_entries (
    id          INTEGER PRIMARY KEY,
    url         TEXT NOT NULL UNIQUE,
    file_name   TEXT NOT NULL,
    content_type TEXT,
    size_bytes  INTEGER NOT NULL DEFAULT 0,
    fetched_at  INTEGER NOT NULL DEFAULT (unixepoch()),
    last_used_at INTEGER NOT NULL DEFAULT (unixepoch())
);
CREATE INDEX idx_imagecache_lru ON image_cache_entries(last_used_at);

CREATE TABLE import_jobs (
    id           INTEGER PRIMARY KEY,
    provider_id  INTEGER NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
    started_at   INTEGER NOT NULL,
    finished_at  INTEGER,
    status       TEXT NOT NULL,
    channels     INTEGER NOT NULL DEFAULT 0,
    skipped      INTEGER NOT NULL DEFAULT 0,
    warnings_json TEXT
);

CREATE TABLE playback_sessions (
    id          INTEGER PRIMARY KEY,
    profile_id  INTEGER REFERENCES user_profiles(id) ON DELETE SET NULL,
    item_type   TEXT,
    item_id     INTEGER,
    started_at  INTEGER NOT NULL DEFAULT (unixepoch()),
    ended_at    INTEGER,
    reconnects  INTEGER NOT NULL DEFAULT 0,
    last_error  TEXT
);

CREATE TABLE diagnostic_events (
    id         INTEGER PRIMARY KEY,
    level      TEXT NOT NULL,
    source     TEXT NOT NULL,
    message    TEXT NOT NULL,
    created_at INTEGER NOT NULL DEFAULT (unixepoch())
);
CREATE INDEX idx_diag_time ON diagnostic_events(created_at);

-- Standardprofil, damit die App sofort nutzbar ist.
INSERT INTO user_profiles (name) VALUES ('Standard');
"#,
}, Migration {
    version: 2,
    name: "verlauf_erweitern",
    sql: r#"
-- watch_history um Anzeigedaten und Eindeutigkeit erweitern.
-- Alte Verlaufsdaten (nur IDs) sind ohne die neuen Felder nutzlos, daher
-- wird die Tabelle neu aufgebaut.
DROP TABLE IF EXISTS watch_history;
CREATE TABLE watch_history (
    id         INTEGER PRIMARY KEY,
    profile_id INTEGER NOT NULL REFERENCES user_profiles(id) ON DELETE CASCADE,
    item_type  TEXT NOT NULL,
    item_id    INTEGER NOT NULL,
    name       TEXT NOT NULL DEFAULT '',
    url        TEXT NOT NULL DEFAULT '',
    logo_url   TEXT,
    watched_at INTEGER NOT NULL DEFAULT (unixepoch()),
    UNIQUE(profile_id, item_type, item_id)
);
CREATE INDEX idx_history_profile ON watch_history(profile_id, watched_at DESC);
"#,
}];

pub fn run(conn: &mut Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version    INTEGER PRIMARY KEY,
            name       TEXT NOT NULL,
            applied_at INTEGER NOT NULL DEFAULT (unixepoch())
        )",
        [],
    )?;

    for m in MIGRATIONS {
        let applied: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version = ?1)",
            [m.version],
            |r| r.get(0),
        )?;
        if applied {
            continue;
        }
        let tx = conn.transaction()?;
        tx.execute_batch(m.sql).map_err(|e| CoreError::Migration {
            version: m.version,
            message: e.to_string(),
        })?;
        tx.execute(
            "INSERT INTO schema_migrations (version, name) VALUES (?1, ?2)",
            rusqlite::params![m.version, m.name],
        )?;
        tx.commit()?;
        tracing::info!(version = m.version, name = m.name, "Migration angewendet");
    }
    Ok(())
}
