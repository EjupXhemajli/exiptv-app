//! IPC-Commands: dünne Schicht über `exiptv-core`.
//!
//! Regeln:
//! - blockierende DB-Arbeit läuft über `spawn_blocking`, die UI friert nie ein
//! - Fehlermeldungen sind nutzerverständlich; technische Details gehen
//!   maskiert ins Log und in die Diagnoseansicht
//! - Zugangsdaten: nur `secret_ref` in der DB, Klartext in den
//!   Betriebssystem-Schlüsselbund

use crate::state::AppState;
use exiptv_core::models::{Channel, ChannelGroup, ImportReport, Provider, ProviderKind};
use exiptv_core::parser::m3u;
use exiptv_core::util::sanitize::sanitize_url;
use serde::{Deserialize, Serialize};
use tauri::{Emitter, State};

use crate::CmdResult;

#[derive(Serialize, Clone)]
pub struct ImportProgress {
    pub provider_id: i64,
    pub stage: String, // "laden" | "verarbeiten" | "speichern" | "fertig"
    pub channels: usize,
}

#[derive(Deserialize)]
pub struct NewProvider {
    pub name: String,
    pub kind: String,
    pub source: String,
    pub username: Option<String>,
    pub password: Option<String>,
}

#[tauri::command]
pub async fn list_providers(state: State<'_, AppState>) -> CmdResult<Vec<Provider>> {
    let db = state.db.lock().map_err(lock_err)?;
    db.list_providers().map_err(user_err)
}

#[tauri::command]
pub async fn add_provider(state: State<'_, AppState>, input: NewProvider) -> CmdResult<i64> {
    let kind = ProviderKind::parse(&input.kind)
        .ok_or_else(|| "Unbekannter Anbietertyp.".to_string())?;

    // Passwort in den Schlüsselbund, Referenz in die DB.
    let secret_ref = match &input.password {
        Some(pw) if !pw.is_empty() => {
            let reference = format!("provider:{}:{}", input.kind, uuid_like());
            crate::secrets::store(&reference, pw)?;
            Some(reference)
        }
        _ => None,
    };

    let db = state.db.lock().map_err(lock_err)?;
    db.insert_provider(
        &input.name,
        kind,
        &input.source,
        input.username.as_deref(),
        secret_ref.as_deref(),
    )
    .map_err(user_err)
}

#[tauri::command]
pub async fn delete_provider(state: State<'_, AppState>, id: i64) -> CmdResult<()> {
    // Erst Geheimnis-Referenz holen, dann DB-Eintrag löschen, dann Secret.
    let secret_ref = {
        let db = state.db.lock().map_err(lock_err)?;
        let provider = db
            .list_providers()
            .map_err(user_err)?
            .into_iter()
            .find(|p| p.id == id);
        db.delete_provider(id).map_err(user_err)?;
        provider.and_then(|p| p.secret_ref)
    };
    if let Some(r) = secret_ref {
        crate::secrets::delete(&r)?;
    }
    Ok(())
}

#[tauri::command]
pub async fn set_provider_enabled(
    state: State<'_, AppState>,
    id: i64,
    enabled: bool,
) -> CmdResult<()> {
    let db = state.db.lock().map_err(lock_err)?;
    db.set_provider_enabled(id, enabled).map_err(user_err)
}

#[tauri::command]
pub async fn import_m3u_from_file(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    provider_id: i64,
    path: String,
) -> CmdResult<ImportReport> {
    emit_progress(&app, provider_id, "laden", 0);
    let bytes = tokio::fs::read(&path)
        .await
        .map_err(|_| "Die Datei konnte nicht gelesen werden. Bitte Pfad und Berechtigungen prüfen.".to_string())?;
    import_bytes(&app, state, provider_id, bytes, None).await
}

#[tauri::command]
pub async fn import_m3u_from_url(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    provider_id: i64,
    url: String,
) -> CmdResult<ImportReport> {
    emit_progress(&app, provider_id, "laden", 0);
    let bytes = crate::http::get_with_retry(&state.http, &url, None, 2)
        .await
        .map_err(|e| {
            tracing::warn!(url = %sanitize_url(&url), fehler = %e, "Playlist-Download fehlgeschlagen");
            format!(
                "Die Playlist konnte nicht geladen werden ({e}). \
                 Die zuletzt funktionierende Version bleibt weiterhin verfügbar."
            )
        })?;
    import_bytes(&app, state, provider_id, bytes, Some(url)).await
}

async fn import_bytes(
    app: &tauri::AppHandle,
    state: State<'_, AppState>,
    provider_id: i64,
    bytes: Vec<u8>,
    base_url: Option<String>,
) -> CmdResult<ImportReport> {
    emit_progress(app, provider_id, "verarbeiten", 0);

    // Kurzen Textausschnitt der Antwort für die Diagnose merken (max. 400 Zeichen).
    let sample: String = String::from_utf8_lossy(&bytes[..bytes.len().min(400)])
        .trim_start()
        .to_string();
    let total_bytes = bytes.len();

    // Parsen + Aufteilen sind CPU-lastig → nicht auf dem UI-Thread.
    // Die Playlist wird dabei in Live-TV, Filme und Serien getrennt
    // (erkannt am URL-Pfad /live/, /movie/, /series/).
    let app2 = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let parsed = m3u::parse_bytes(&bytes, base_url.as_deref());
        emit_progress(&app2, provider_id, "speichern", parsed.entries.len());
        let total = parsed.entries.len();
        let split = crate::m3u_split::split_entries(parsed.entries);
        (split, parsed.report, total)
    })
    .await
    .map_err(|e| format!("Interner Fehler bei der Verarbeitung: {e}"))?;

    let (split, mut report, total_entries) = result;

    // Aussagekräftige Diagnose, wenn gar nichts erkannt wurde.
    if total_entries == 0 {
        let hint = if total_bytes == 0 {
            "Der Server hat eine leere Antwort geschickt. Das deutet auf einen \
             abgelaufenen Zugang, einen offline stehenden Server oder eine \
             falsche Serveradresse hin."
        } else if looks_like_html(&sample) {
            "Der Server hat eine Webseite statt einer Playlist geschickt. Das \
             passiert meist bei falschem Benutzernamen/Passwort oder wenn der \
             Zugang abgelaufen ist."
        } else {
            "Die Antwort enthielt keine erkennbaren Sender. Prüfe, ob die URL \
             wirklich auf eine M3U-Playlist zeigt (z. B. Endung type=m3u_plus)."
        };
        let name = provider_name(&state, provider_id).unwrap_or_default();
        return Err(format!(
            "Der Anbieter „{name}“ lieferte keine Sender. {hint} \
             Die zuletzt funktionierende Version bleibt erhalten."
        ));
    }

    let live_count = split.live.len();
    let movie_count = split.movies.len();
    let series_count = split.series.len();

    {
        let mut db = state.db.lock().map_err(lock_err)?;
        crate::m3u_split::store_split(&mut db, provider_id, split, &report)?;
    }

    report.channels_parsed = live_count;
    report.movies_parsed = movie_count;
    report.series_parsed = series_count;
    // Ehrlicher Hinweis, wenn die Playlist nur Live-TV enthält.
    if live_count > 0 && movie_count == 0 && series_count == 0 {
        report.warnings.push(
            "Diese Playlist enthält nur Live-Sender – keine Filme oder Serien. \
             Das ist eine Einschränkung des Anbieters."
                .to_string(),
        );
    }

    emit_progress(app, provider_id, "fertig", live_count);
    tracing::info!(
        provider_id,
        kanaele = live_count,
        filme = movie_count,
        serien = series_count,
        uebersprungen = report.channels_skipped,
        "Playlist-Import abgeschlossen"
    );
    Ok(report)
}

#[tauri::command]
pub async fn list_groups(
    state: State<'_, AppState>,
    provider_id: i64,
) -> CmdResult<Vec<ChannelGroup>> {
    let db = state.db.lock().map_err(lock_err)?;
    db.list_groups(provider_id).map_err(user_err)
}

#[tauri::command]
pub async fn list_channels(
    state: State<'_, AppState>,
    provider_id: i64,
    group_id: Option<i64>,
    limit: i64,
    offset: i64,
    sort: Option<String>,
) -> CmdResult<Vec<Channel>> {
    let sort = sort
        .as_deref()
        .map(exiptv_core::models::ChannelSort::parse)
        .unwrap_or_default();
    let db = state.db.lock().map_err(lock_err)?;
    db.list_channels_page(provider_id, group_id, limit.clamp(1, 500), offset.max(0), sort)
        .map_err(user_err)
}

#[tauri::command]
pub async fn rename_provider(
    state: State<'_, AppState>,
    id: i64,
    name: String,
) -> CmdResult<()> {
    let db = state.db.lock().map_err(lock_err)?;
    db.rename_provider(id, &name).map_err(user_err)
}

#[tauri::command]
pub async fn count_channels(state: State<'_, AppState>, provider_id: i64) -> CmdResult<i64> {
    let db = state.db.lock().map_err(lock_err)?;
    db.count_channels(provider_id).map_err(user_err)
}

#[tauri::command]
pub async fn search_channels(state: State<'_, AppState>, query: String) -> CmdResult<Vec<Channel>> {
    if query.trim().len() < 2 {
        return Ok(vec![]);
    }
    let db = state.db.lock().map_err(lock_err)?;
    db.search_channels(&query, 100).map_err(user_err)
}

#[tauri::command]
pub async fn get_setting(state: State<'_, AppState>, key: String) -> CmdResult<Option<String>> {
    let db = state.db.lock().map_err(lock_err)?;
    db.get_setting(&key).map_err(user_err)
}

#[tauri::command]
pub async fn set_setting(state: State<'_, AppState>, key: String, value: String) -> CmdResult<()> {
    let db = state.db.lock().map_err(lock_err)?;
    db.set_setting(&key, &value).map_err(user_err)
}

#[derive(Serialize)]
pub struct Diagnostics {
    pub app_version: String,
    pub os: String,
    pub arch: String,
    pub db_schema_version: i64,
}

#[tauri::command]
pub async fn app_diagnostics(state: State<'_, AppState>) -> CmdResult<Diagnostics> {
    let db = state.db.lock().map_err(lock_err)?;
    Ok(Diagnostics {
        app_version: env!("CARGO_PKG_VERSION").into(),
        os: std::env::consts::OS.into(),
        arch: std::env::consts::ARCH.into(),
        db_schema_version: db.schema_version().map_err(user_err)?,
    })
}

// ----------------------------------------------------------------------

fn emit_progress(app: &tauri::AppHandle, provider_id: i64, stage: &str, channels: usize) {
    let _ = app.emit(
        "import-progress",
        ImportProgress { provider_id, stage: stage.into(), channels },
    );
}

fn lock_err<T>(_: T) -> String {
    "Interner Zustandsfehler. Bitte EXIPTV neu starten.".into()
}

fn user_err(e: exiptv_core::CoreError) -> String {
    tracing::error!(fehler = %e, "Core-Fehler");
    e.to_string()
}

/// Eindeutige Referenz ohne zusätzliche Abhängigkeit (Zeit + Zähler).
fn uuid_like() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static C: AtomicU64 = AtomicU64::new(0);
    format!(
        "{:x}-{:x}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0),
        C.fetch_add(1, Ordering::Relaxed)
    )
}

// ------------------------------------------------------------------
// VOD: Filme & Serien
// ------------------------------------------------------------------

#[tauri::command]
pub async fn movie_categories(state: State<'_, AppState>, provider_id: i64) -> CmdResult<Vec<String>> {
    let db = state.db.lock().map_err(lock_err)?;
    db.movie_categories(provider_id).map_err(user_err)
}

#[tauri::command]
pub async fn list_movies(
    state: State<'_, AppState>,
    provider_id: i64,
    category: Option<String>,
    limit: i64,
    offset: i64,
    sort: Option<String>,
) -> CmdResult<Vec<exiptv_core::models::Movie>> {
    let sort = exiptv_core::models::VodSort::parse(sort.as_deref().unwrap_or("name_asc"));
    let db = state.db.lock().map_err(lock_err)?;
    db.list_movies(provider_id, category.as_deref(), limit.clamp(1, 200), offset.max(0), sort)
        .map_err(user_err)
}

#[tauri::command]
pub async fn series_categories(state: State<'_, AppState>, provider_id: i64) -> CmdResult<Vec<String>> {
    let db = state.db.lock().map_err(lock_err)?;
    db.series_categories(provider_id).map_err(user_err)
}

#[tauri::command]
pub async fn list_series(
    state: State<'_, AppState>,
    provider_id: i64,
    category: Option<String>,
    limit: i64,
    offset: i64,
    sort: Option<String>,
) -> CmdResult<Vec<exiptv_core::models::Series>> {
    let sort = exiptv_core::models::VodSort::parse(sort.as_deref().unwrap_or("name_asc"));
    let db = state.db.lock().map_err(lock_err)?;
    db.list_series(provider_id, category.as_deref(), limit.clamp(1, 200), offset.max(0), sort)
        .map_err(user_err)
}

#[tauri::command]
pub async fn list_seasons(state: State<'_, AppState>, series_id: i64) -> CmdResult<Vec<exiptv_core::models::Season>> {
    let db = state.db.lock().map_err(lock_err)?;
    db.list_seasons(series_id).map_err(user_err)
}

#[tauri::command]
pub async fn list_episodes(state: State<'_, AppState>, season_id: i64) -> CmdResult<Vec<exiptv_core::models::Episode>> {
    let db = state.db.lock().map_err(lock_err)?;
    db.list_episodes(season_id).map_err(user_err)
}

#[tauri::command]
pub async fn count_movies(state: State<'_, AppState>, provider_id: i64) -> CmdResult<i64> {
    let db = state.db.lock().map_err(lock_err)?;
    db.count_movies(provider_id).map_err(user_err)
}

#[tauri::command]
pub async fn count_series(state: State<'_, AppState>, provider_id: i64) -> CmdResult<i64> {
    let db = state.db.lock().map_err(lock_err)?;
    db.count_series(provider_id).map_err(user_err)
}

/// Erkennt, ob eine Antwort eine HTML-Seite ist (Login-/Fehlerseite statt
/// Playlist). Xtream-Panels liefern das bei ungültigem Zugang.
fn looks_like_html(sample: &str) -> bool {
    let s = sample.trim_start().to_ascii_lowercase();
    s.starts_with("<!doctype html")
        || s.starts_with("<html")
        || s.starts_with("<head")
        || s.contains("<body")
        || s.contains("<title")
}

/// Liest den Anzeigenamen eines Anbieters (für Fehlermeldungen).
fn provider_name(state: &State<'_, AppState>, provider_id: i64) -> Option<String> {
    let db = state.db.lock().ok()?;
    db.list_providers()
        .ok()?
        .into_iter()
        .find(|p| p.id == provider_id)
        .map(|p| p.name)
}

// ------------------------------------------------------------------
// Xtream-Import-Commands
// ------------------------------------------------------------------

/// Liest Server/Benutzer eines Xtream-Providers und das Passwort aus dem
/// Credential Manager.
fn xtream_creds(state: &State<'_, AppState>, provider_id: i64) -> Result<(String, String, String), String> {
    let db = state.db.lock().map_err(lock_err)?;
    let provider = db.list_providers().map_err(user_err)?
        .into_iter().find(|p| p.id == provider_id)
        .ok_or_else(|| "Anbieter nicht gefunden.".to_string())?;
    let username = provider.username.clone().unwrap_or_default();
    let password = match &provider.secret_ref {
        Some(r) => crate::secrets::load(r)?.unwrap_or_default(),
        None => String::new(),
    };
    Ok((provider.source.clone(), username, password))
}

#[tauri::command]
pub async fn import_xtream(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    provider_id: i64,
) -> CmdResult<ImportReport> {
    let (server, username, password) = xtream_creds(&state, provider_id)?;
    crate::xtream_import::import_xtream(&app, state, provider_id, server, username, password).await
}

#[tauri::command]
pub async fn load_series_seasons(
    state: State<'_, AppState>,
    provider_id: i64,
    series_db_id: i64,
    series_external_id: i64,
) -> CmdResult<()> {
    let (server, username, password) = xtream_creds(&state, provider_id)?;
    crate::xtream_import::load_series_detail(
        state, provider_id, server, username, password, series_external_id, series_db_id,
    ).await
}

// ------------------------------------------------------------------
// Favoriten & Verlauf
// ------------------------------------------------------------------

#[tauri::command]
pub async fn add_favorite(state: State<'_, AppState>, item_type: String, item_id: i64) -> CmdResult<()> {
    let db = state.db.lock().map_err(lock_err)?;
    db.add_favorite(&item_type, item_id).map_err(user_err)
}

#[tauri::command]
pub async fn remove_favorite(state: State<'_, AppState>, item_type: String, item_id: i64) -> CmdResult<()> {
    let db = state.db.lock().map_err(lock_err)?;
    db.remove_favorite(&item_type, item_id).map_err(user_err)
}

#[tauri::command]
pub async fn is_favorite(state: State<'_, AppState>, item_type: String, item_id: i64) -> CmdResult<bool> {
    let db = state.db.lock().map_err(lock_err)?;
    db.is_favorite(&item_type, item_id).map_err(user_err)
}

#[tauri::command]
pub async fn favorite_channels(state: State<'_, AppState>) -> CmdResult<Vec<Channel>> {
    let db = state.db.lock().map_err(lock_err)?;
    db.favorite_channels().map_err(user_err)
}

#[tauri::command]
pub async fn favorite_channel_ids(state: State<'_, AppState>) -> CmdResult<Vec<i64>> {
    let db = state.db.lock().map_err(lock_err)?;
    db.favorite_channel_ids().map_err(user_err)
}

#[tauri::command]
pub async fn add_history(
    state: State<'_, AppState>,
    item_type: String,
    item_id: i64,
    name: String,
    url: String,
    logo: Option<String>,
) -> CmdResult<()> {
    let db = state.db.lock().map_err(lock_err)?;
    db.add_history(&item_type, item_id, &name, &url, logo.as_deref()).map_err(user_err)
}

#[tauri::command]
pub async fn list_history(state: State<'_, AppState>) -> CmdResult<Vec<exiptv_core::models::HistoryEntry>> {
    let db = state.db.lock().map_err(lock_err)?;
    db.list_history(200).map_err(user_err)
}

#[tauri::command]
pub async fn clear_history(state: State<'_, AppState>) -> CmdResult<()> {
    let db = state.db.lock().map_err(lock_err)?;
    db.clear_history().map_err(user_err)
}

// ------------------------------------------------------------------
// Wartung: Log lesen, Cache leeren
// ------------------------------------------------------------------

/// Liest die neueste Logdatei (für die Diagnoseansicht). Sensible Daten
/// werden bereits beim Schreiben maskiert.
#[tauri::command]
pub async fn read_log(app: tauri::AppHandle) -> CmdResult<String> {
    use tauri::Manager;
    let dir = app.path().app_log_dir().map_err(|_| "Log-Verzeichnis nicht gefunden.".to_string())?;
    // Neueste exiptv.log*-Datei finden.
    let mut newest: Option<(std::time::SystemTime, std::path::PathBuf)> = None;
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with("exiptv.log") {
                if let Ok(meta) = e.metadata() {
                    if let Ok(modt) = meta.modified() {
                        if newest.as_ref().map(|(t, _)| modt > *t).unwrap_or(true) {
                            newest = Some((modt, e.path()));
                        }
                    }
                }
            }
        }
    }
    let path = newest.map(|(_, p)| p).ok_or_else(|| "Keine Logdatei vorhanden.".to_string())?;
    let content = std::fs::read_to_string(&path).map_err(|e| format!("Log konnte nicht gelesen werden: {e}"))?;
    // Nur die letzten ~400 Zeilen zurückgeben (Diagnose bleibt übersichtlich).
    let lines: Vec<&str> = content.lines().collect();
    let start = lines.len().saturating_sub(400);
    Ok(lines[start..].join("\n"))
}

/// Leert den Bild-Cache (Poster/Logos). Aktuell werden Bilder direkt geladen;
/// diese Funktion entfernt ein evtl. vorhandenes Cache-Verzeichnis und meldet
/// Erfolg, damit die Oberfläche Bilder neu anfordert.
#[tauri::command]
pub async fn clear_image_cache(app: tauri::AppHandle) -> CmdResult<()> {
    use tauri::Manager;
    if let Ok(cache) = app.path().app_cache_dir() {
        let img = cache.join("images");
        if img.exists() {
            let _ = std::fs::remove_dir_all(&img);
        }
    }
    Ok(())
}

// ------------------------------------------------------------------
// Bild-Cache: Poster/Logos über das Backend laden (mit User-Agent),
// lokal zwischenspeichern und als konvertierbaren Pfad zurückgeben.
// ------------------------------------------------------------------

/// Lädt ein Bild (Poster/Logo) über das Backend und cached es lokal.
/// Gibt den lokalen Dateipfad zurück, den das Frontend über
/// `convertFileSrc` anzeigen kann. So laden auch Poster, die einen
/// bestimmten User-Agent verlangen, und werden nicht bei jedem Start
/// neu heruntergeladen.
#[tauri::command]
pub async fn cache_image(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    url: String,
) -> CmdResult<String> {
    use tauri::Manager;
    use std::hash::{Hash, Hasher};

    if url.is_empty() {
        return Err("leer".into());
    }

    // Cache-Verzeichnis anlegen.
    let cache_root = app.path().app_cache_dir().map_err(|_| "Cache-Verzeichnis nicht verfügbar.".to_string())?;
    let img_dir = cache_root.join("images");
    std::fs::create_dir_all(&img_dir).map_err(|e| format!("Cache konnte nicht angelegt werden: {e}"))?;

    // Dateiname aus URL-Hash + Endung.
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    url.hash(&mut hasher);
    let hash = hasher.finish();
    let ext = url.rsplit('.').next()
        .filter(|e| e.len() <= 4 && e.chars().all(|c| c.is_ascii_alphanumeric()))
        .unwrap_or("img");
    let path = img_dir.join(format!("{hash:016x}.{ext}"));

    // Bereits im Cache?
    if path.exists() {
        return Ok(path.to_string_lossy().to_string());
    }

    // Herunterladen (mit dem konfigurierten Client inkl. User-Agent).
    let resp = state.http.get(&url).send().await.map_err(|_| "Bild nicht erreichbar.".to_string())?;
    if !resp.status().is_success() {
        return Err(format!("Bild-Status {}", resp.status().as_u16()));
    }
    // Nur echte Bilder speichern (Content-Type prüfen).
    let ct = resp.headers().get("content-type").and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
    if !ct.starts_with("image/") && !ct.is_empty() {
        return Err("kein Bild".into());
    }
    let bytes = resp.bytes().await.map_err(|_| "Bild-Download abgebrochen.".to_string())?;
    if bytes.is_empty() {
        return Err("leeres Bild".into());
    }
    std::fs::write(&path, &bytes).map_err(|e| format!("Bild konnte nicht gespeichert werden: {e}"))?;
    Ok(path.to_string_lossy().to_string())
}

/// Beendet die Anwendung geordnet (Wiedergabe stoppen, Fenster schließen).
#[tauri::command]
pub async fn quit_app(app: tauri::AppHandle) -> CmdResult<()> {
    tracing::info!("Anwendung wird auf Nutzerwunsch beendet");
    app.exit(0);
    Ok(())
}

// ------------------------------------------------------------------
// Nachrichten für die Startseite (öffentliche RSS-Feeds)
// ------------------------------------------------------------------

/// Öffentliche Nachrichten-Feeds (ARD/tagesschau und Sportschau).
/// Schwerpunkt Sport liegt auf Fußball: Bundesliga, Nationalmannschaft,
/// Champions League und DFB-Pokal.
const NEWS_FEEDS: &[(&str, &str)] = &[
    // Politik / allgemeine Nachrichten
    ("Politik", "https://www.tagesschau.de/infoservices/alle-meldungen-100~rss2.xml"),
    ("Politik", "https://www.tagesschau.de/inland/index~rss2.xml"),
    // Sport – Schwerpunkt Fußball
    ("Fußball", "https://www.sportschau.de/fussball/bundesliga/index~rss2.xml"),
    ("Fußball", "https://www.sportschau.de/fussball/nationalmannschaft/index~rss2.xml"),
    ("Fußball", "https://www.sportschau.de/fussball/champions-league/index~rss2.xml"),
    ("Fußball", "https://www.sportschau.de/fussball/index~rss2.xml"),
    ("Sport", "https://www.sportschau.de/index~rss2.xml"),
];

#[tauri::command]
pub async fn fetch_news(
    state: State<'_, AppState>,
    per_feed: Option<usize>,
) -> CmdResult<Vec<exiptv_core::parser::rss::NewsItem>> {
    use exiptv_core::parser::rss;

    let per_feed = per_feed.unwrap_or(4).clamp(1, 20);
    let mut politik: Vec<rss::NewsItem> = Vec::new();
    let mut sport: Vec<rss::NewsItem> = Vec::new();

    for &(kategorie, url) in NEWS_FEEDS {
        // Genug gesammelt? Dann weitere Quellen überspringen.
        if politik.len() >= 12 && sport.len() >= 12 {
            break;
        }
        match crate::http::get_with_retry(&state.http, url, None, 1).await {
            Ok(bytes) => {
                let xml = String::from_utf8_lossy(&bytes);
                let items = rss::parse_feed(&xml, kategorie, per_feed);
                tracing::info!(quelle = kategorie, anzahl = items.len(), "Nachrichten geladen");
                for item in items {
                    // Doppelte Meldungen (gleiche Überschrift) überspringen.
                    let ziel = if kategorie == "Politik" { &mut politik } else { &mut sport };
                    if !ziel.iter().any(|n| n.title == item.title) {
                        ziel.push(item);
                    }
                }
            }
            Err(e) => {
                tracing::warn!(quelle = kategorie, fehler = %e, "Nachrichtenquelle nicht erreichbar");
            }
        }
    }

    if politik.is_empty() && sport.is_empty() {
        return Err("Es konnten keine Nachrichten geladen werden. \
                    Bitte prüfe deine Internetverbindung."
            .into());
    }

    // Politik und Sport abwechselnd mischen.
    let mut gemischt: Vec<rss::NewsItem> = Vec::with_capacity(politik.len() + sport.len());
    let mut p = politik.into_iter();
    let mut s = sport.into_iter();
    loop {
        let a = p.next();
        let b = s.next();
        if a.is_none() && b.is_none() { break; }
        if let Some(x) = a { gemischt.push(x); }
        if let Some(y) = b { gemischt.push(y); }
    }
    gemischt.truncate(16);

    let mit_bild = gemischt.iter().filter(|n| n.image_url.is_some()).count();
    tracing::info!(gesamt = gemischt.len(), mit_bild, "Nachrichten geladen");
    Ok(gemischt)
}

/// Lädt das Vorschaubild einer einzelnen Artikelseite nach.
///
/// Wird von der Oberfläche für Meldungen ohne Bild aufgerufen – so
/// erscheint die Slideshow sofort und die Bilder kommen nach und nach dazu,
/// statt dass die Startseite auf alle Abrufe wartet.
#[tauri::command]
pub async fn fetch_article_image(
    state: State<'_, AppState>,
    url: String,
) -> CmdResult<Option<String>> {
    if url.is_empty() || !url.starts_with("http") {
        return Ok(None);
    }
    let seite = tokio::time::timeout(
        std::time::Duration::from_secs(8),
        crate::http::get_with_retry(&state.http, &url, None, 0),
    )
    .await;
    match seite {
        Ok(Ok(bytes)) => {
            let html = String::from_utf8_lossy(&bytes);
            Ok(exiptv_core::parser::rss::extract_article_image(&html))
        }
        _ => Ok(None),
    }
}
