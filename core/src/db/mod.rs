//! SQLite-Zugriffsschicht.
//!
//! - WAL-Modus, Foreign Keys, sinnvolle PRAGMAs
//! - versionierte Migrationen (`migrations.rs`)
//! - alle Schreibpfade laufen in Transaktionen
//! - Playlist-Aktualisierung als Staging-Verfahren: alte Daten werden erst
//!   ersetzt, wenn der neue Import vollständig validiert in derselben
//!   Transaktion vorliegt (Anforderung Abschnitt 13)

pub mod migrations;

use crate::error::{CoreError, Result};
use crate::models::{
    Channel, ChannelGroup, ChannelSort, Episode, HistoryEntry, ImportReport, Movie, Provider,
    ProviderKind, Season, Series, VodSort,
};
use crate::parser::m3u::M3uEntry;
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;

pub struct Database {
    conn: Connection,
}

#[cfg(test)]
impl Database {
    /// Nur für Tests: direkter Zugriff auf die Connection zum Einfügen
    /// von Beispieldaten.
    pub(crate) fn conn(&self) -> &Connection {
        &self.conn
    }
}

impl Database {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        Self::init(conn)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update(None, "busy_timeout", 5_000)?;
        let mut db = Self { conn };
        migrations::run(&mut db.conn)?;
        Ok(db)
    }

    pub fn schema_version(&self) -> Result<i64> {
        Ok(self
            .conn
            .query_row("SELECT MAX(version) FROM schema_migrations", [], |r| {
                r.get::<_, Option<i64>>(0)
            })?
            .unwrap_or(0))
    }

    // ------------------------------------------------------------------
    // Einstellungen
    // ------------------------------------------------------------------

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO app_settings (key, value, updated_at) VALUES (?1, ?2, unixepoch())
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = unixepoch()",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row(
                "SELECT value FROM app_settings WHERE key = ?1",
                params![key],
                |r| r.get(0),
            )
            .optional()?)
    }

    // ------------------------------------------------------------------
    // Anbieter
    // ------------------------------------------------------------------

    pub fn insert_provider(
        &self,
        name: &str,
        kind: ProviderKind,
        source: &str,
        username: Option<&str>,
        secret_ref: Option<&str>,
    ) -> Result<i64> {
        if name.trim().is_empty() {
            return Err(CoreError::InvalidInput("Anbietername darf nicht leer sein".into()));
        }
        self.conn.execute(
            "INSERT INTO providers (name, kind, source, username, secret_ref, enabled, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 1, unixepoch(), unixepoch())",
            params![name.trim(), kind.as_str(), source, username, secret_ref],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn list_providers(&self) -> Result<Vec<Provider>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, kind, source, username, secret_ref, enabled,
                    auto_refresh_hours, epg_url, user_agent, referer,
                    last_refresh_at, expires_at, max_connections, created_at, updated_at
             FROM providers ORDER BY name COLLATE NOCASE",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(Provider {
                id: r.get(0)?,
                name: r.get(1)?,
                kind: ProviderKind::parse(&r.get::<_, String>(2)?)
                    .unwrap_or(ProviderKind::DirectStream),
                source: r.get(3)?,
                username: r.get(4)?,
                secret_ref: r.get(5)?,
                enabled: r.get::<_, i64>(6)? != 0,
                auto_refresh_hours: r.get(7)?,
                epg_url: r.get(8)?,
                user_agent: r.get(9)?,
                referer: r.get(10)?,
                last_refresh_at: r.get(11)?,
                expires_at: r.get(12)?,
                max_connections: r.get(13)?,
                created_at: r.get(14)?,
                updated_at: r.get(15)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn set_provider_enabled(&self, id: i64, enabled: bool) -> Result<()> {
        self.conn.execute(
            "UPDATE providers SET enabled = ?2, updated_at = unixepoch() WHERE id = ?1",
            params![id, enabled as i64],
        )?;
        Ok(())
    }

    /// Benennt einen Anbieter um (Bearbeiten-Funktion). Der Name muss
    /// nichtleer sein; Quelle/Zugangsdaten bleiben unangetastet.
    pub fn rename_provider(&self, id: i64, name: &str) -> Result<()> {
        if name.trim().is_empty() {
            return Err(CoreError::InvalidInput("Anbietername darf nicht leer sein".into()));
        }
        self.conn.execute(
            "UPDATE providers SET name = ?2, updated_at = unixepoch() WHERE id = ?1",
            params![id, name.trim()],
        )?;
        Ok(())
    }

    /// Löscht einen Anbieter samt aller abhängigen Daten (ON DELETE CASCADE).
    pub fn delete_provider(&self, id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM providers WHERE id = ?1", params![id])?;
        Ok(())
    }

    // ------------------------------------------------------------------
    // Kanäle & Gruppen
    // ------------------------------------------------------------------

    pub fn count_channels(&self, provider_id: i64) -> Result<i64> {
        Ok(self.conn.query_row(
            "SELECT COUNT(*) FROM channels WHERE provider_id = ?1",
            params![provider_id],
            |r| r.get(0),
        )?)
    }

    pub fn list_groups(&self, provider_id: i64) -> Result<Vec<ChannelGroup>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, provider_id, name, sort_index, hidden
             FROM channel_groups WHERE provider_id = ?1 ORDER BY sort_index, name COLLATE NOCASE",
        )?;
        let rows = stmt.query_map(params![provider_id], |r| {
            Ok(ChannelGroup {
                id: r.get(0)?,
                provider_id: r.get(1)?,
                name: r.get(2)?,
                sort_index: r.get(3)?,
                hidden: r.get::<_, i64>(4)? != 0,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn list_channels_page(
        &self,
        provider_id: i64,
        group_id: Option<i64>,
        limit: i64,
        offset: i64,
        sort: ChannelSort,
    ) -> Result<Vec<Channel>> {
        // ORDER-BY-Klausel aus festem Enum – kein String vom Nutzer, daher
        // keine Injektionsgefahr.
        let order = match sort {
            ChannelSort::Default => "sort_index, name COLLATE NOCASE",
            ChannelSort::NameAsc => "name COLLATE NOCASE ASC",
            ChannelSort::NameDesc => "name COLLATE NOCASE DESC",
            ChannelSort::RecentlyAdded => "id DESC",
            ChannelSort::ChannelNumber => "channel_number IS NULL, channel_number, name COLLATE NOCASE",
        };
        let sql = format!(
            "SELECT id, provider_id, group_id, name, url, tvg_id, tvg_name, logo_url,
                    channel_number, is_radio, catchup, catchup_days, catchup_source,
                    timeshift, user_agent, referer, hidden, locked, sort_index
             FROM channels
             WHERE provider_id = ?1 AND (?2 IS NULL OR group_id = ?2) AND hidden = 0
             ORDER BY {order}
             LIMIT ?3 OFFSET ?4"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params![provider_id, group_id, limit, offset], row_to_channel)?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn search_channels(&self, query: &str, limit: i64) -> Result<Vec<Channel>> {
        let like = format!("%{}%", normalize_search(query));
        let mut stmt = self.conn.prepare(
            "SELECT id, provider_id, group_id, name, url, tvg_id, tvg_name, logo_url,
                    channel_number, is_radio, catchup, catchup_days, catchup_source,
                    timeshift, user_agent, referer, hidden, locked, sort_index
             FROM channels
             WHERE search_name LIKE ?1 AND hidden = 0
             ORDER BY name COLLATE NOCASE LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![like, limit], row_to_channel)?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    // ------------------------------------------------------------------
    // Staging-Import (Abschnitt 13)
    // ------------------------------------------------------------------

    /// Ersetzt die Kanäle eines Anbieters atomar durch das Parse-Ergebnis.
    ///
    /// Schlägt der Import an irgendeiner Stelle fehl, bleibt der alte
    /// Datenbestand vollständig erhalten (Transaktions-Rollback).
    pub fn replace_channels_staged(
        &mut self,
        provider_id: i64,
        entries: &[M3uEntry],
        report: &ImportReport,
    ) -> Result<usize> {
        // Validierung VOR dem Ersetzen alter Daten.
        if entries.is_empty() {
            return Err(CoreError::Playlist(
                "Die neue Playlist enthält keine gültigen Kanäle. \
                 Die zuletzt funktionierende Version bleibt erhalten."
                    .into(),
            ));
        }

        let tx = self.conn.transaction()?;
        {
            tx.execute(
                "DELETE FROM channels WHERE provider_id = ?1",
                params![provider_id],
            )?;
            tx.execute(
                "DELETE FROM channel_groups WHERE provider_id = ?1",
                params![provider_id],
            )?;

            let mut group_ids: std::collections::HashMap<String, i64> =
                std::collections::HashMap::new();
            let mut insert_group = tx.prepare(
                "INSERT INTO channel_groups (provider_id, name, sort_index, hidden)
                 VALUES (?1, ?2, ?3, 0)",
            )?;
            let mut insert_channel = tx.prepare(
                "INSERT INTO channels
                 (provider_id, group_id, name, search_name, url, tvg_id, tvg_name, logo_url,
                  channel_number, is_radio, catchup, catchup_days, catchup_source, timeshift,
                  user_agent, referer, hidden, locked, sort_index)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,0,0,?17)",
            )?;

            for (idx, e) in entries.iter().enumerate() {
                let group_id = match &e.group {
                    Some(g) => {
                        let next_index = group_ids.len() as i64;
                        Some(match group_ids.get(g) {
                            Some(id) => *id,
                            None => {
                                insert_group.execute(params![provider_id, g, next_index])?;
                                let id = tx.last_insert_rowid();
                                group_ids.insert(g.clone(), id);
                                id
                            }
                        })
                    }
                    None => None,
                };
                insert_channel.execute(params![
                    provider_id,
                    group_id,
                    e.name,
                    normalize_search(&e.name),
                    e.url,
                    e.tvg_id,
                    e.tvg_name,
                    e.logo_url,
                    e.channel_number,
                    e.is_radio as i64,
                    e.catchup,
                    e.catchup_days,
                    e.catchup_source,
                    e.timeshift,
                    e.user_agent,
                    e.referer,
                    idx as i64,
                ])?;
            }

            tx.execute(
                "UPDATE providers SET last_refresh_at = unixepoch(), updated_at = unixepoch()
                 WHERE id = ?1",
                params![provider_id],
            )?;
            tx.execute(
                "INSERT INTO import_jobs (provider_id, started_at, finished_at, status, channels, skipped, warnings_json)
                 VALUES (?1, unixepoch(), unixepoch(), 'ok', ?2, ?3, ?4)",
                params![
                    provider_id,
                    entries.len() as i64,
                    report.channels_skipped as i64,
                    serde_json::to_string(&report.warnings)?
                ],
            )?;
        }
        tx.commit()?;
        Ok(entries.len())
    }

    // ------------------------------------------------------------------
    // VOD: Filme & Serien
    // ------------------------------------------------------------------

    /// Distinct-Kategorien der Filme eines Anbieters (für Filterleiste).
    pub fn movie_categories(&self, provider_id: i64) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT category FROM movies
             WHERE provider_id = ?1 AND category IS NOT NULL AND category <> ''
             ORDER BY category COLLATE NOCASE",
        )?;
        let rows = stmt.query_map(params![provider_id], |r| r.get::<_, String>(0))?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn list_movies(
        &self,
        provider_id: i64,
        category: Option<&str>,
        limit: i64,
        offset: i64,
        sort: VodSort,
    ) -> Result<Vec<Movie>> {
        // Die Sortierung stammt aus einem festen Enum (kein Nutzertext),
        // daher ist das Einsetzen ins SQL unbedenklich.
        let sql = format!(
            "SELECT id, provider_id, name, url, category, poster_url, backdrop_url, plot,
                    year, genre, duration_s, rating, age_rating, director, [cast], trailer_url
             FROM movies
             WHERE provider_id = ?1 AND (?2 IS NULL OR category = ?2)
             ORDER BY {}
             LIMIT ?3 OFFSET ?4",
            sort.order_sql()
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params![provider_id, category, limit, offset], row_to_movie)?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn search_movies(&self, query: &str, limit: i64) -> Result<Vec<Movie>> {
        let like = format!("%{}%", normalize_search(query));
        let mut stmt = self.conn.prepare(
            "SELECT id, provider_id, name, url, category, poster_url, backdrop_url, plot,
                    year, genre, duration_s, rating, age_rating, director, [cast], trailer_url
             FROM movies WHERE search_name LIKE ?1
             ORDER BY name COLLATE NOCASE LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![like, limit], row_to_movie)?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn series_categories(&self, provider_id: i64) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT category FROM series
             WHERE provider_id = ?1 AND category IS NOT NULL AND category <> ''
             ORDER BY category COLLATE NOCASE",
        )?;
        let rows = stmt.query_map(params![provider_id], |r| r.get::<_, String>(0))?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn list_series(
        &self,
        provider_id: i64,
        category: Option<&str>,
        limit: i64,
        offset: i64,
        sort: VodSort,
    ) -> Result<Vec<Series>> {
        let sql = format!(
            "SELECT id, provider_id, external_id, name, category, poster_url, backdrop_url, plot,
                    year, genre, rating, age_rating
             FROM series
             WHERE provider_id = ?1 AND (?2 IS NULL OR category = ?2)
             ORDER BY {}
             LIMIT ?3 OFFSET ?4",
            sort.order_sql()
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params![provider_id, category, limit, offset], row_to_series)?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn search_series(&self, query: &str, limit: i64) -> Result<Vec<Series>> {
        let like = format!("%{}%", normalize_search(query));
        let mut stmt = self.conn.prepare(
            "SELECT id, provider_id, external_id, name, category, poster_url, backdrop_url, plot,
                    year, genre, rating, age_rating
             FROM series WHERE search_name LIKE ?1
             ORDER BY name COLLATE NOCASE LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![like, limit], row_to_series)?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// Staffeln einer Serie inkl. Episodenzahl.
    pub fn list_seasons(&self, series_id: i64) -> Result<Vec<Season>> {
        let mut stmt = self.conn.prepare(
            "SELECT s.id, s.series_id, s.number, s.name,
                    (SELECT COUNT(*) FROM episodes e WHERE e.season_id = s.id) AS ep_count
             FROM seasons s WHERE s.series_id = ?1 ORDER BY s.number",
        )?;
        let rows = stmt.query_map(params![series_id], |r| {
            Ok(Season {
                id: r.get(0)?,
                series_id: r.get(1)?,
                number: r.get(2)?,
                name: r.get(3)?,
                episode_count: r.get(4)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn list_episodes(&self, season_id: i64) -> Result<Vec<Episode>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, season_id, number, name, url, plot, duration_s, poster_url
             FROM episodes WHERE season_id = ?1 ORDER BY number",
        )?;
        let rows = stmt.query_map(params![season_id], |r| {
            Ok(Episode {
                id: r.get(0)?,
                season_id: r.get(1)?,
                number: r.get(2)?,
                name: r.get(3)?,
                url: r.get(4)?,
                plot: r.get(5)?,
                duration_s: r.get(6)?,
                poster_url: r.get(7)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn count_movies(&self, provider_id: i64) -> Result<i64> {
        Ok(self.conn.query_row(
            "SELECT COUNT(*) FROM movies WHERE provider_id = ?1",
            params![provider_id], |r| r.get(0),
        )?)
    }

    pub fn count_series(&self, provider_id: i64) -> Result<i64> {
        Ok(self.conn.query_row(
            "SELECT COUNT(*) FROM series WHERE provider_id = ?1",
            params![provider_id], |r| r.get(0),
        )?)
    }

    // ------------------------------------------------------------------
    // Xtream-Import (Live/VOD/Serien) – Staging in einer Transaktion
    // ------------------------------------------------------------------

    /// Ersetzt alle Filme eines Anbieters atomar (alte erst nach Erfolg weg).
    pub fn replace_movies(&mut self, provider_id: i64, movies: &[NewMovie]) -> Result<usize> {
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM movies WHERE provider_id = ?1", params![provider_id])?;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO movies (provider_id, external_id, name, search_name, url, category,
                                     poster_url, plot, year, genre, duration_s, rating, age_rating,
                                     director, [cast], trailer_url, added_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16, unixepoch())",
            )?;
            for m in movies {
                stmt.execute(params![
                    provider_id, m.external_id, m.name, normalize_search(&m.name), m.url, m.category,
                    m.poster_url, m.plot, m.year, m.genre, m.duration_s, m.rating, m.age_rating,
                    m.director, m.cast, m.trailer_url
                ])?;
            }
        }
        tx.commit()?;
        Ok(movies.len())
    }

    /// Ersetzt alle Serien (inkl. Staffeln/Episoden) eines Anbieters atomar.
    pub fn replace_series(&mut self, provider_id: i64, series: &[NewSeriesFull]) -> Result<usize> {
        let tx = self.conn.transaction()?;
        // Kaskade löscht Staffeln + Episoden mit.
        tx.execute("DELETE FROM series WHERE provider_id = ?1", params![provider_id])?;
        {
            let mut s_stmt = tx.prepare(
                "INSERT INTO series (provider_id, external_id, name, search_name, category,
                                     poster_url, plot, year, genre, rating, added_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10, unixepoch())",
            )?;
            let mut se_stmt = tx.prepare(
                "INSERT OR IGNORE INTO seasons (series_id, number, name) VALUES (?1,?2,?3)",
            )?;
            let mut ep_stmt = tx.prepare(
                "INSERT OR IGNORE INTO episodes (season_id, number, name, url, plot, duration_s, poster_url)
                 VALUES (?1,?2,?3,?4,?5,?6,?7)",
            )?;
            for s in series {
                s_stmt.execute(params![
                    provider_id, s.external_id, s.name, normalize_search(&s.name), s.category,
                    s.poster_url, s.plot, s.year, s.genre, s.rating
                ])?;
                let series_id = tx.last_insert_rowid();
                for season in &s.seasons {
                    se_stmt.execute(params![series_id, season.number, season.name])?;
                    let season_id = tx.last_insert_rowid();
                    for ep in &season.episodes {
                        ep_stmt.execute(params![
                            season_id, ep.number, ep.name, ep.url, ep.plot, ep.duration_s, ep.poster_url
                        ])?;
                    }
                }
            }
        }
        tx.commit()?;
        Ok(series.len())
    }

    /// Ersetzt die Staffeln/Episoden einer bereits vorhandenen Serie
    /// (lazy Nachladen beim Öffnen der Detailseite).
    pub fn replace_series_seasons(&mut self, series_id: i64, seasons: &[NewSeason]) -> Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM seasons WHERE series_id = ?1", params![series_id])?;
        {
            let mut se_stmt = tx.prepare(
                "INSERT OR IGNORE INTO seasons (series_id, number, name) VALUES (?1,?2,?3)",
            )?;
            let mut ep_stmt = tx.prepare(
                "INSERT OR IGNORE INTO episodes (season_id, number, name, url, plot, duration_s, poster_url)
                 VALUES (?1,?2,?3,?4,?5,?6,?7)",
            )?;
            for season in seasons {
                se_stmt.execute(params![series_id, season.number, season.name])?;
                let season_id = tx.last_insert_rowid();
                for ep in &season.episodes {
                    ep_stmt.execute(params![
                        season_id, ep.number, ep.name, ep.url, ep.plot, ep.duration_s, ep.poster_url
                    ])?;
                }
            }
        }
        tx.commit()?;
        Ok(())
    }

    // ------------------------------------------------------------------
    // Favoriten (Standard-Profil = 1)
    // ------------------------------------------------------------------

    pub fn add_favorite(&self, item_type: &str, item_id: i64) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO favorites (profile_id, item_type, item_id) VALUES (1, ?1, ?2)",
            params![item_type, item_id],
        )?;
        Ok(())
    }

    pub fn remove_favorite(&self, item_type: &str, item_id: i64) -> Result<()> {
        self.conn.execute(
            "DELETE FROM favorites WHERE profile_id = 1 AND item_type = ?1 AND item_id = ?2",
            params![item_type, item_id],
        )?;
        Ok(())
    }

    pub fn is_favorite(&self, item_type: &str, item_id: i64) -> Result<bool> {
        let n: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM favorites WHERE profile_id = 1 AND item_type = ?1 AND item_id = ?2",
            params![item_type, item_id], |r| r.get(0),
        )?;
        Ok(n > 0)
    }

    /// Favorisierte Kanäle (als vollständige Channel-Objekte).
    pub fn favorite_channels(&self) -> Result<Vec<Channel>> {
        let mut stmt = self.conn.prepare(
            "SELECT c.id, c.provider_id, c.group_id, c.name, c.url, c.tvg_id, c.tvg_name,
                    c.logo_url, c.channel_number, c.is_radio, c.catchup, c.catchup_days,
                    c.catchup_source, c.timeshift, c.user_agent, c.referer, c.hidden, c.locked,
                    c.sort_index
             FROM favorites f
             JOIN channels c ON c.id = f.item_id
             WHERE f.profile_id = 1 AND f.item_type = 'channel'
             ORDER BY f.created_at DESC",
        )?;
        let rows = stmt.query_map([], row_to_channel)?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// IDs aller favorisierten Kanäle (für schnelle Markierung in Listen).
    pub fn favorite_channel_ids(&self) -> Result<Vec<i64>> {
        let mut stmt = self.conn.prepare(
            "SELECT item_id FROM favorites WHERE profile_id = 1 AND item_type = 'channel'",
        )?;
        let rows = stmt.query_map([], |r| r.get::<_, i64>(0))?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    // ------------------------------------------------------------------
    // Verlauf (Standard-Profil = 1)
    // ------------------------------------------------------------------

    /// Trägt einen Eintrag in den Wiedergabeverlauf ein (bzw. aktualisiert ihn).
    pub fn add_history(&self, item_type: &str, item_id: i64, name: &str, url: &str, logo: Option<&str>) -> Result<()> {
        self.conn.execute(
            "INSERT INTO watch_history (profile_id, item_type, item_id, name, url, logo_url, watched_at)
             VALUES (1, ?1, ?2, ?3, ?4, ?5, unixepoch())
             ON CONFLICT(profile_id, item_type, item_id)
             DO UPDATE SET watched_at = unixepoch(), name = ?3, url = ?4, logo_url = ?5",
            params![item_type, item_id, name, url, logo],
        )?;
        Ok(())
    }

    /// Wiedergabeverlauf (neueste zuerst).
    pub fn list_history(&self, limit: i64) -> Result<Vec<HistoryEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT item_type, item_id, name, url, logo_url, watched_at
             FROM watch_history WHERE profile_id = 1
             ORDER BY watched_at DESC LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit], |r| {
            Ok(HistoryEntry {
                item_type: r.get(0)?,
                item_id: r.get(1)?,
                name: r.get(2)?,
                url: r.get(3)?,
                logo_url: r.get(4)?,
                watched_at: r.get(5)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn clear_history(&self) -> Result<()> {
        self.conn.execute("DELETE FROM watch_history WHERE profile_id = 1", [])?;
        Ok(())
    }
}

/// Eingabe für den Film-Import.
#[derive(Debug, Clone, Default)]
pub struct NewMovie {
    pub external_id: Option<String>,
    pub name: String,
    pub url: String,
    pub category: Option<String>,
    pub poster_url: Option<String>,
    pub plot: Option<String>,
    pub year: Option<i64>,
    pub genre: Option<String>,
    pub duration_s: Option<i64>,
    pub rating: Option<f64>,
    pub age_rating: Option<String>,
    pub director: Option<String>,
    pub cast: Option<String>,
    pub trailer_url: Option<String>,
}

/// Eingabe für den Serien-Import (mit Staffeln/Episoden).
#[derive(Debug, Clone, Default)]
pub struct NewSeriesFull {
    pub external_id: Option<String>,
    pub name: String,
    pub category: Option<String>,
    pub poster_url: Option<String>,
    pub plot: Option<String>,
    pub year: Option<i64>,
    pub genre: Option<String>,
    pub rating: Option<f64>,
    pub seasons: Vec<NewSeason>,
}

#[derive(Debug, Clone, Default)]
pub struct NewSeason {
    pub number: i64,
    pub name: Option<String>,
    pub episodes: Vec<NewEpisode>,
}

#[derive(Debug, Clone, Default)]
pub struct NewEpisode {
    pub number: i64,
    pub name: Option<String>,
    pub url: String,
    pub plot: Option<String>,
    pub duration_s: Option<i64>,
    pub poster_url: Option<String>,
}

fn row_to_channel(r: &rusqlite::Row<'_>) -> std::result::Result<Channel, rusqlite::Error> {
    Ok(Channel {
        id: r.get(0)?,
        provider_id: r.get(1)?,
        group_id: r.get(2)?,
        name: r.get(3)?,
        url: r.get(4)?,
        tvg_id: r.get(5)?,
        tvg_name: r.get(6)?,
        logo_url: r.get(7)?,
        channel_number: r.get(8)?,
        is_radio: r.get::<_, i64>(9)? != 0,
        catchup: r.get(10)?,
        catchup_days: r.get(11)?,
        catchup_source: r.get(12)?,
        timeshift: r.get(13)?,
        user_agent: r.get(14)?,
        referer: r.get(15)?,
        hidden: r.get::<_, i64>(16)? != 0,
        locked: r.get::<_, i64>(17)? != 0,
        sort_index: r.get(18)?,
    })
}

fn row_to_movie(r: &rusqlite::Row<'_>) -> std::result::Result<Movie, rusqlite::Error> {
    Ok(Movie {
        id: r.get(0)?,
        provider_id: r.get(1)?,
        name: r.get(2)?,
        url: r.get(3)?,
        category: r.get(4)?,
        poster_url: r.get(5)?,
        backdrop_url: r.get(6)?,
        plot: r.get(7)?,
        year: r.get(8)?,
        genre: r.get(9)?,
        duration_s: r.get(10)?,
        rating: r.get(11)?,
        age_rating: r.get(12)?,
        director: r.get(13)?,
        cast: r.get(14)?,
        trailer_url: r.get(15)?,
    })
}

fn row_to_series(r: &rusqlite::Row<'_>) -> std::result::Result<Series, rusqlite::Error> {
    Ok(Series {
        id: r.get(0)?,
        provider_id: r.get(1)?,
        external_id: r.get(2)?,
        name: r.get(3)?,
        category: r.get(4)?,
        poster_url: r.get(5)?,
        backdrop_url: r.get(6)?,
        plot: r.get(7)?,
        year: r.get(8)?,
        genre: r.get(9)?,
        rating: r.get(10)?,
        age_rating: r.get(11)?,
    })
}

/// Normalisierung für tolerante Suche: Kleinschreibung, Diakritika-Faltung,
/// Trennzeichen → Leerzeichen, mehrfache Leerzeichen kollabieren.
pub fn normalize_search(s: &str) -> String {    let mut out = String::with_capacity(s.len());
    let mut last_space = true; // führende Leerzeichen unterdrücken
    for c in s.to_lowercase().chars() {
        let mapped: &str = match c {
            'ä' | 'á' | 'à' | 'â' | 'ã' | 'å' => "a",
            'ö' | 'ó' | 'ò' | 'ô' | 'õ' => "o",
            'ü' | 'ú' | 'ù' | 'û' => "u",
            'é' | 'è' | 'ê' | 'ë' => "e",
            'í' | 'ì' | 'î' | 'ï' => "i",
            'ß' => "ss",
            'ç' => "c",
            'ñ' => "n",
            _ if c.is_alphanumeric() => {
                out.push(c);
                last_space = false;
                continue;
            }
            // Alles andere (Bindestrich, Punkt, Slash, …) trennt Wörter.
            _ => {
                if !last_space {
                    out.push(' ');
                    last_space = true;
                }
                continue;
            }
        };
        out.push_str(mapped);
        last_space = false;
    }
    while out.ends_with(' ') {
        out.pop();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::m3u::parse_str;

    fn sample_entries(n: usize) -> Vec<M3uEntry> {
        let mut s = String::from("#EXTM3U\n");
        for i in 0..n {
            s.push_str(&format!(
                "#EXTINF:-1 tvg-id=\"ch{i}\" group-title=\"Gruppe {}\",Kanal {i}\nhttp://x.tld/{i}.m3u8\n",
                i % 10
            ));
        }
        parse_str(&s, None).entries
    }

    #[test]
    fn migrationen_laufen_und_sind_idempotent() {
        let db = Database::open_in_memory().unwrap();
        assert!(db.schema_version().unwrap() >= 1);
    }

    #[test]
    fn einstellungen_upsert() {
        let db = Database::open_in_memory().unwrap();
        db.set_setting("sprache", "de").unwrap();
        db.set_setting("sprache", "en").unwrap();
        assert_eq!(db.get_setting("sprache").unwrap().as_deref(), Some("en"));
        assert_eq!(db.get_setting("gibt_es_nicht").unwrap(), None);
    }

    #[test]
    fn anbieter_anlegen_und_listen() {
        let db = Database::open_in_memory().unwrap();
        let id = db
            .insert_provider("Test-Anbieter", ProviderKind::M3uUrl, "https://x.tld/l.m3u", None, None)
            .unwrap();
        let list = db.list_providers().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, id);
        assert!(list[0].enabled);
        assert!(db.insert_provider("  ", ProviderKind::M3uUrl, "x", None, None).is_err());
    }

    #[test]
    fn staging_import_ersetzt_atomar() {
        let mut db = Database::open_in_memory().unwrap();
        let pid = db
            .insert_provider("P", ProviderKind::M3uFile, "/tmp/a.m3u", None, None)
            .unwrap();
        let n = db
            .replace_channels_staged(pid, &sample_entries(100), &ImportReport::default())
            .unwrap();
        assert_eq!(n, 100);
        assert_eq!(db.count_channels(pid).unwrap(), 100);
        assert_eq!(db.list_groups(pid).unwrap().len(), 10);

        // Zweiter Import ersetzt vollständig
        db.replace_channels_staged(pid, &sample_entries(50), &ImportReport::default())
            .unwrap();
        assert_eq!(db.count_channels(pid).unwrap(), 50);
    }

    #[test]
    fn leerer_import_laesst_alte_daten_unangetastet() {
        let mut db = Database::open_in_memory().unwrap();
        let pid = db
            .insert_provider("P", ProviderKind::M3uUrl, "https://x.tld/l.m3u", None, None)
            .unwrap();
        db.replace_channels_staged(pid, &sample_entries(20), &ImportReport::default())
            .unwrap();
        let err = db
            .replace_channels_staged(pid, &[], &ImportReport::default())
            .unwrap_err();
        assert!(err.to_string().contains("bleibt erhalten"));
        assert_eq!(db.count_channels(pid).unwrap(), 20, "alte Daten müssen erhalten bleiben");
    }

    #[test]
    fn anbieter_loeschen_entfernt_abhaengige_daten() {
        let mut db = Database::open_in_memory().unwrap();
        let pid = db
            .insert_provider("P", ProviderKind::M3uFile, "/tmp/a.m3u", None, None)
            .unwrap();
        db.replace_channels_staged(pid, &sample_entries(10), &ImportReport::default())
            .unwrap();
        db.delete_provider(pid).unwrap();
        assert_eq!(db.count_channels(pid).unwrap(), 0);
        assert!(db.list_groups(pid).unwrap().is_empty());
    }

    #[test]
    fn paginierte_kanalliste_und_suche() {
        let mut db = Database::open_in_memory().unwrap();
        let pid = db
            .insert_provider("P", ProviderKind::M3uFile, "/tmp/a.m3u", None, None)
            .unwrap();
        db.replace_channels_staged(pid, &sample_entries(500), &ImportReport::default())
            .unwrap();
        let page = db.list_channels_page(pid, None, 50, 0, ChannelSort::Default).unwrap();
        assert_eq!(page.len(), 50);
        let hits = db.search_channels("KANAL 42", 10).unwrap();
        assert!(hits.iter().any(|c| c.name == "Kanal 42"));
    }

    #[test]
    fn kanal_sortierung_varianten() {
        let mut db = Database::open_in_memory().unwrap();
        let pid = db
            .insert_provider("S", ProviderKind::M3uFile, "/tmp/s.m3u", None, None)
            .unwrap();
        db.replace_channels_staged(pid, &sample_entries(30), &ImportReport::default())
            .unwrap();

        // A–Z: erster Name <= letzter Name.
        let az = db.list_channels_page(pid, None, 30, 0, ChannelSort::NameAsc).unwrap();
        assert_eq!(az.len(), 30);
        assert!(az.first().unwrap().name <= az.last().unwrap().name);

        // Zuletzt hinzugefügt: höchste id zuerst.
        let recent = db.list_channels_page(pid, None, 5, 0, ChannelSort::RecentlyAdded).unwrap();
        assert!(recent.windows(2).all(|w| w[0].id > w[1].id));
    }

    #[test]
    fn provider_umbenennen() {
        let db = Database::open_in_memory().unwrap();
        let pid = db
            .insert_provider("Alt", ProviderKind::M3uUrl, "https://x.tld/a.m3u", None, None)
            .unwrap();
        db.rename_provider(pid, "  Neu  ").unwrap();
        let p = db.list_providers().unwrap();
        assert_eq!(p.iter().find(|x| x.id == pid).unwrap().name, "Neu");
        // Leerer Name wird abgelehnt.
        assert!(db.rename_provider(pid, "   ").is_err());
    }

    #[test]
    fn film_sortierung() {
        let mut db = Database::open_in_memory().unwrap();
        let pid = db.insert_provider("P", ProviderKind::M3uUrl, "http://x", None, None).unwrap();
        let movies = vec![
            NewMovie { name: "Zebra".into(), url: "u1".into(), year: Some(1999), rating: Some(5.0), ..Default::default() },
            NewMovie { name: "Alpha".into(), url: "u2".into(), year: Some(2024), rating: Some(9.0), ..Default::default() },
            NewMovie { name: "Mitte".into(), url: "u3".into(), year: None, rating: None, ..Default::default() },
        ];
        db.replace_movies(pid, &movies).unwrap();

        // A–Z
        let asc = db.list_movies(pid, None, 10, 0, VodSort::NameAsc).unwrap();
        assert_eq!(asc[0].name, "Alpha");
        assert_eq!(asc[2].name, "Zebra");

        // Z–A
        let desc = db.list_movies(pid, None, 10, 0, VodSort::NameDesc).unwrap();
        assert_eq!(desc[0].name, "Zebra");

        // Jahr absteigend: neueste zuerst, Einträge ohne Jahr ans Ende.
        let year_desc = db.list_movies(pid, None, 10, 0, VodSort::YearDesc).unwrap();
        assert_eq!(year_desc[0].name, "Alpha");   // 2024
        assert_eq!(year_desc[1].name, "Zebra");   // 1999
        assert_eq!(year_desc[2].name, "Mitte");   // ohne Jahr

        // Bewertung absteigend, ohne Bewertung ans Ende.
        let rating = db.list_movies(pid, None, 10, 0, VodSort::RatingDesc).unwrap();
        assert_eq!(rating[0].name, "Alpha");
        assert_eq!(rating[2].name, "Mitte");
    }

    #[test]
    fn viele_filme_und_serien_speichern() {
        let mut db = Database::open_in_memory().unwrap();
        let pid = db.insert_provider("P", ProviderKind::M3uUrl, "http://x", None, None).unwrap();

        // 5.000 Filme.
        let movies: Vec<NewMovie> = (0..5_000)
            .map(|i| NewMovie {
                name: format!("Film {i}"),
                url: format!("http://s/movie/u/p/{i}.mkv"),
                category: Some("Filme".into()),
                ..Default::default()
            })
            .collect();
        db.replace_movies(pid, &movies).unwrap();
        assert_eq!(db.count_movies(pid).unwrap(), 5_000);

        // 200 Serien mit je 2 Staffeln à 10 Episoden = 4.000 Episoden.
        let series: Vec<NewSeriesFull> = (0..200)
            .map(|s| NewSeriesFull {
                name: format!("Serie {s}"),
                seasons: (1..=2)
                    .map(|st| NewSeason {
                        number: st,
                        name: None,
                        episodes: (1..=10)
                            .map(|e| NewEpisode {
                                number: e,
                                name: Some(format!("Folge {e}")),
                                url: format!("http://s/series/u/p/{s}_{st}_{e}.mkv"),
                                ..Default::default()
                            })
                            .collect(),
                    })
                    .collect(),
                ..Default::default()
            })
            .collect();
        db.replace_series(pid, &series).unwrap();
        assert_eq!(db.count_series(pid).unwrap(), 200);

        // Stichprobe: Staffeln und Episoden der ersten Serie vorhanden.
        let liste = db.list_series(pid, None, 5, 0, VodSort::default()).unwrap();
        let seasons = db.list_seasons(liste[0].id).unwrap();
        assert_eq!(seasons.len(), 2);
        let eps = db.list_episodes(seasons[0].id).unwrap();
        assert_eq!(eps.len(), 10);
    }

    #[test]
    fn favoriten_und_verlauf() {
        let mut db = Database::open_in_memory().unwrap();
        let pid = db.insert_provider("P", ProviderKind::M3uFile, "/tmp/a.m3u", None, None).unwrap();
        db.replace_channels_staged(pid, &sample_entries(5), &ImportReport::default()).unwrap();
        let ch = db.list_channels_page(pid, None, 5, 0, ChannelSort::Default).unwrap();
        let cid = ch[0].id;

        // Favorit hinzufügen/prüfen/entfernen.
        assert!(!db.is_favorite("channel", cid).unwrap());
        db.add_favorite("channel", cid).unwrap();
        assert!(db.is_favorite("channel", cid).unwrap());
        assert_eq!(db.favorite_channels().unwrap().len(), 1);
        assert_eq!(db.favorite_channel_ids().unwrap(), vec![cid]);
        db.remove_favorite("channel", cid).unwrap();
        assert!(!db.is_favorite("channel", cid).unwrap());

        // Verlauf: zweimal derselbe Kanal → ein Eintrag (aktualisiert).
        db.add_history("channel", cid, "Kanal 1", "http://s/1", None).unwrap();
        db.add_history("channel", cid, "Kanal 1", "http://s/1", None).unwrap();
        let h = db.list_history(10).unwrap();
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].name, "Kanal 1");
        db.clear_history().unwrap();
        assert_eq!(db.list_history(10).unwrap().len(), 0);
    }

    #[test]
    fn vod_filme_serien_abfragen() {
        let db = Database::open_in_memory().unwrap();
        let pid = db
            .insert_provider("VOD", ProviderKind::Xtream, "http://s.tld", None, None)
            .unwrap();
        // Film einfügen.
        db.conn().execute(
            "INSERT INTO movies (provider_id, name, search_name, url, category, poster_url, year)
             VALUES (?1, 'Der Film', 'der film', 'http://s/1.mkv', 'Action', 'http://p/1.jpg', 2024)",
            rusqlite::params![pid],
        ).unwrap();
        // Serie + Staffel + Episode.
        db.conn().execute(
            "INSERT INTO series (provider_id, name, search_name, category)
             VALUES (?1, 'Die Serie', 'die serie', 'Drama')",
            rusqlite::params![pid],
        ).unwrap();
        let sid = db.conn().last_insert_rowid();
        db.conn().execute(
            "INSERT OR IGNORE INTO seasons (series_id, number, name) VALUES (?1, 1, 'Staffel 1')",
            rusqlite::params![sid],
        ).unwrap();
        let season_id = db.conn().last_insert_rowid();
        db.conn().execute(
            "INSERT INTO episodes (season_id, number, name, url) VALUES (?1, 1, 'Pilot', 'http://s/e1.mkv')",
            rusqlite::params![season_id],
        ).unwrap();

        let movies = db.list_movies(pid, None, 50, 0, VodSort::default()).unwrap();
        assert_eq!(movies.len(), 1);
        assert_eq!(movies[0].year, Some(2024));
        assert_eq!(db.movie_categories(pid).unwrap(), vec!["Action"]);

        let series = db.list_series(pid, None, 50, 0, VodSort::default()).unwrap();
        assert_eq!(series.len(), 1);
        let seasons = db.list_seasons(series[0].id).unwrap();
        assert_eq!(seasons.len(), 1);
        assert_eq!(seasons[0].episode_count, 1);
        let eps = db.list_episodes(seasons[0].id).unwrap();
        assert_eq!(eps.len(), 1);
        assert_eq!(eps[0].name.as_deref(), Some("Pilot"));

        // Suche.
        assert_eq!(db.search_movies("film", 10).unwrap().len(), 1);
        assert_eq!(db.search_series("serie", 10).unwrap().len(), 1);
    }

    #[test]
    fn grosser_import_10k_in_transaktion() {
        let mut db = Database::open_in_memory().unwrap();
        let pid = db
            .insert_provider("XXL", ProviderKind::M3uUrl, "https://x.tld/xxl.m3u", None, None)
            .unwrap();
        let t = std::time::Instant::now();
        db.replace_channels_staged(pid, &sample_entries(10_000), &ImportReport::default())
            .unwrap();
        assert_eq!(db.count_channels(pid).unwrap(), 10_000);
        assert!(t.elapsed().as_secs() < 10, "Import zu langsam: {:?}", t.elapsed());
    }

    #[test]
    fn suche_normalisiert_umlaute() {
        assert_eq!(normalize_search("Münchën-TV!"), "munchen tv");
        assert_eq!(normalize_search("  Café -- del   Mar "), "cafe del mar");
    }
}
