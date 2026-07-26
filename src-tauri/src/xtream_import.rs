//! Xtream-Codes-Import: ruft die player_api.php-Endpunkte ab, parst die
//! JSON-Antworten (Core-Logik) und speichert Live-Kanäle, Filme und Serien
//! in der Datenbank. Löst das eigentliche Problem, dass ein Xtream-Zugang
//! die vollständige URL mit Zugangsdaten braucht.

use crate::state::AppState;
use crate::CmdResult;
use exiptv_core::parser::m3u::M3uEntry;
use exiptv_core::parser::xtream::*;
use exiptv_core::db::{NewEpisode, NewMovie, NewSeason, NewSeriesFull};
use std::collections::HashMap;
use tauri::{Emitter, State};

fn user_err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

/// Fallback-Import über die get.php-M3U-Playlist, wenn player_api.php nicht
/// funktioniert. Teilt die Playlist in Live-TV, Filme und Serien auf.
async fn import_via_m3u_fallback(
    app: &tauri::AppHandle,
    state: &State<'_, AppState>,
    provider_id: i64,
    creds: &XtreamCreds,
) -> CmdResult<exiptv_core::models::ImportReport> {
    let emit = |stage: &str, n: usize| {
        let _ = app.emit("import-progress", serde_json::json!({
            "provider_id": provider_id, "stage": stage, "channels": n,
        }));
    };

    emit("laden", 0);
    let bytes = fetch(&state.http, &creds.m3u_url()).await.map_err(|e| {
        format!("Die Playlist konnte nicht geladen werden. {e}")
    })?;
    if bytes.is_empty() {
        return Err("Der Server lieferte eine leere Playlist. Bitte prüfe Zugang und Serveradresse.".into());
    }

    emit("verarbeiten", 0);
    // Parsen + Aufteilen im Blocking-Pool (CPU-lastig bei großen Listen).
    let app2 = app.clone();
    let split = tauri::async_runtime::spawn_blocking(move || {
        let parsed = exiptv_core::parser::m3u::parse_bytes(&bytes, None);
        let _ = app2.emit("import-progress", serde_json::json!({
            "provider_id": provider_id, "stage": "speichern", "channels": parsed.entries.len(),
        }));
        crate::m3u_split::split_entries(parsed.entries)
    })
    .await
    .map_err(|e| format!("Interner Fehler bei der Verarbeitung: {e}"))?;

    let report = exiptv_core::models::ImportReport::default();
    let (live_count, movie_count, series_count) = {
        let mut db = state.db.lock().map_err(user_err)?;
        crate::m3u_split::store_split(&mut db, provider_id, split, &report)?
    };

    emit("fertig", live_count);
    tracing::info!(provider_id, live_count, movie_count, series_count, "M3U-Fallback-Import abgeschlossen");

    let mut out = exiptv_core::models::ImportReport::default();
    out.channels_parsed = live_count;
    out.movies_parsed = movie_count;
    out.series_parsed = series_count;
    if live_count == 0 && movie_count == 0 && series_count == 0 {
        out.warnings.push(
            "Der Server lieferte keine verwertbaren Einträge. Bitte Zugang prüfen.".to_string(),
        );
    }
    Ok(out)
}

/// Führt den vollständigen Xtream-Import durch.
///
/// Strategie: Zuerst der M3U-Weg (get.php) mit Aufteilung in Live/Film/Serie.
/// Der funktioniert bei praktisch allen Panels zuverlässig. Nur wenn der
/// M3U-Weg keine Einträge liefert, wird die player_api.php-API versucht
/// (manche Panels liefern NUR über die API).
pub async fn import_xtream(
    app: &tauri::AppHandle,
    state: State<'_, AppState>,
    provider_id: i64,
    server: String,
    username: String,
    password: String,
) -> CmdResult<exiptv_core::models::ImportReport> {
    let creds = XtreamCreds::new(&server, &username, &password);

    // 1) Primär: M3U-Weg (get.php) – bei den meisten Servern der stabilste.
    match import_via_m3u_fallback(app, &state, provider_id, &creds).await {
        Ok(report) if report.channels_parsed > 0 || report.movies_parsed > 0 || report.series_parsed > 0 => {
            tracing::info!("Import über get.php/M3U erfolgreich");
            return Ok(report);
        }
        Ok(_) => {
            tracing::warn!("get.php lieferte keine Einträge – versuche player_api.php");
        }
        Err(e) => {
            tracing::warn!("get.php fehlgeschlagen ({e}) – versuche player_api.php");
        }
    }

    // 2) Sekundär: player_api.php (Xtream-API).
    import_via_player_api(app, &state, provider_id, &creds).await
}

/// Import über die player_api.php-Xtream-API (Live/VOD/Serien getrennt).
async fn import_via_player_api(
    app: &tauri::AppHandle,
    state: &State<'_, AppState>,
    provider_id: i64,
    creds: &XtreamCreds,
) -> CmdResult<exiptv_core::models::ImportReport> {
    let client = &state.http;

    let emit = |stage: &str, n: usize| {
        let _ = app.emit("import-progress", serde_json::json!({
            "provider_id": provider_id, "stage": stage, "channels": n,
        }));
    };

    // 1) Authentifizierung prüfen.
    emit("laden", 0);
    let auth_bytes = fetch(client, &creds.auth_url()).await.map_err(|_| {
        "Weder die Playlist (get.php) noch die Xtream-API (player_api.php) lieferten Daten. \
         Der Anbieter lehnt die Zugangsdaten ab oder der Server ist nicht erreichbar. \
         Bitte Benutzername, Passwort und Serveradresse prüfen."
            .to_string()
    })?;
    let auth: AuthResponse = serde_json::from_slice(&auth_bytes).map_err(|_| {
        "Der Server lieferte keine gültige Xtream-Antwort. Bitte Serveradresse prüfen.".to_string()
    })?;
    match &auth.user_info {
        Some(ui) if ui.auth == 1 && ui.status.eq_ignore_ascii_case("Active") => {}
        Some(ui) if ui.auth == 1 && ui.status.is_empty() => {}
        _ => {
            return Err(
                "Die Zugangsdaten wurden vom Anbieter abgelehnt. \
                 Bitte Benutzername, Passwort und Serveradresse prüfen."
                    .to_string(),
            );
        }
    }

    // 2) Live-TV: Kategorien + Streams.
    emit("verarbeiten", 0);
    let live_cats = fetch_categories(client, &creds.live_categories_url()).await.unwrap_or_default();
    let live_raw = fetch(client, &creds.live_streams_url()).await?;
    let live: Vec<LiveStream> = serde_json::from_slice(&live_raw).unwrap_or_default();

    let mut channels: Vec<M3uEntry> = Vec::with_capacity(live.len());
    for s in &live {
        let group = s.category_id.as_ref().and_then(|id| live_cats.get(id).cloned());
        channels.push(M3uEntry {
            name: s.name.clone(),
            url: creds.live_stream_url(s.stream_id),
            group,
            tvg_id: s.epg_channel_id.clone(),
            tvg_name: Some(s.name.clone()),
            logo_url: s.stream_icon.clone(),
            channel_number: s.num,
            ..Default::default()
        });
    }
    emit("speichern", channels.len());
    let live_count = channels.len();
    {
        let mut db = state.db.lock().map_err(user_err)?;
        let report = exiptv_core::models::ImportReport::default();
        db.replace_channels_staged(provider_id, &channels, &report)
            .map_err(user_err)?;
    }

    // 3) VOD/Filme.
    let vod_cats = fetch_categories(client, &creds.vod_categories_url()).await.unwrap_or_default();
    let vod_raw = match fetch(client, &creds.vod_streams_url()).await {
        Ok(b) => b,
        Err(e) => { tracing::warn!("VOD-Abruf fehlgeschlagen: {e}"); Vec::new() }
    };
    // Eintragsweise parsen: ein fehlerhafter Film darf nicht alle killen.
    let vod: Vec<VodStream> = parse_array_lenient(&vod_raw, "VOD");
    tracing::info!("VOD-Rohantwort {} Bytes, {} Filme geparst", vod_raw.len(), vod.len());
    let movies: Vec<NewMovie> = vod.iter().map(|m| {
        let ext = m.container_extension.clone().unwrap_or_default();
        NewMovie {
            external_id: Some(m.stream_id.to_string()),
            name: m.name.clone(),
            url: creds.vod_stream_url(m.stream_id, &ext),
            category: m.category_id.as_ref().and_then(|id| vod_cats.get(id).cloned()),
            poster_url: m.stream_icon.clone(),
            rating: m.rating.as_ref().and_then(|r| r.parse().ok()),
            ..Default::default()
        }
    }).collect();
    let movie_count = movies.len();
    {
        let mut db = state.db.lock().map_err(user_err)?;
        db.replace_movies(provider_id, &movies).map_err(user_err)?;
    }

    // 4) Serien (Liste). Staffeln/Episoden werden beim Öffnen nachgeladen,
    //    um nicht tausende Detail-Anfragen beim Import zu machen.
    let series_cats = fetch_categories(client, &creds.series_categories_url()).await.unwrap_or_default();
    let series_raw = match fetch(client, &creds.series_url()).await {
        Ok(b) => b,
        Err(e) => { tracing::warn!("Serien-Abruf fehlgeschlagen: {e}"); Vec::new() }
    };
    let series_list: Vec<SeriesEntry> = parse_array_lenient(&series_raw, "Serien");
    tracing::info!("Serien-Rohantwort {} Bytes, {} Serien geparst", series_raw.len(), series_list.len());
    let series: Vec<NewSeriesFull> = series_list.iter().map(|s| NewSeriesFull {
        external_id: Some(s.series_id.to_string()),
        name: s.name.clone(),
        category: s.category_id.as_ref().and_then(|id| series_cats.get(id).cloned()),
        poster_url: s.cover.clone(),
        plot: s.plot.clone(),
        genre: s.genre.clone(),
        rating: s.rating.as_ref().and_then(|r| r.parse().ok()),
        year: s.release_date.as_ref().and_then(|d| d.get(0..4)).and_then(|y| y.parse().ok()),
        seasons: Vec::new(), // lazy
    }).collect();
    let series_count = series.len();
    {
        let mut db = state.db.lock().map_err(user_err)?;
        db.replace_series(provider_id, &series).map_err(user_err)?;
    }

    emit("fertig", live_count);
    tracing::info!(provider_id, live_count, movie_count, series_count, "Xtream-Import abgeschlossen");

    let mut report = exiptv_core::models::ImportReport::default();
    report.channels_parsed = live_count;
    report.movies_parsed = movie_count;
    report.series_parsed = series_count;
    // Ehrlicher Hinweis, wenn Live-TV kam, aber kein VOD: liegt fast immer am
    // Zugang (keine VOD-/Serien-Rechte), nicht an der App.
    if live_count > 0 && movie_count == 0 && series_count == 0 {
        report.warnings.push(
            "Dieser Zugang liefert keine Filme oder Serien (nur Live-TV). \
             Das ist eine Einschränkung des Anbieter-Kontos."
                .to_string(),
        );
    }
    Ok(report)
}

/// Lädt Staffeln + Episoden einer Serie nach (beim Öffnen der Detailseite).
pub async fn load_series_detail(
    state: State<'_, AppState>,
    provider_id: i64,
    server: String,
    username: String,
    password: String,
    series_external_id: i64,
    series_db_id: i64,
) -> CmdResult<()> {
    let creds = XtreamCreds::new(&server, &username, &password);
    let raw = fetch(&state.http, &creds.series_info_url(series_external_id)).await?;
    let info: SeriesInfo = serde_json::from_slice(&raw)
        .map_err(|_| "Die Serieninformationen konnten nicht gelesen werden.".to_string())?;

    // Staffeln/Episoden aufbauen.
    let mut seasons: Vec<NewSeason> = Vec::new();
    let mut season_keys: Vec<i64> = info.episodes.keys().filter_map(|k| k.parse().ok()).collect();
    season_keys.sort();
    for sk in season_keys {
        let eps_raw = &info.episodes[&sk.to_string()];
        let episodes: Vec<NewEpisode> = eps_raw.iter().map(|e| {
            let ext = e.container_extension.clone().unwrap_or_default();
            NewEpisode {
                number: e.episode_num.unwrap_or(0),
                name: e.title.clone(),
                url: creds.series_stream_url(e.id, &ext),
                plot: e.info.as_ref().and_then(|i| i.plot.clone()),
                duration_s: e.info.as_ref().and_then(|i| i.duration_secs),
                poster_url: e.info.as_ref().and_then(|i| i.movie_image.clone()),
            }
        }).collect();
        seasons.push(NewSeason { number: sk, name: None, episodes });
    }

    // In die DB schreiben (nur diese eine Serie ersetzen wir gezielt).
    {
        let mut db = state.db.lock().map_err(user_err)?;
        db.replace_series_seasons(series_db_id, &seasons).map_err(user_err)?;
    }
    let _ = provider_id;
    Ok(())
}

// ===== HTTP-Helfer =====

/// Parst ein JSON-Array eintragsweise und überspringt fehlerhafte Elemente.
/// Robust gegen: leere Antwort, in ein Objekt gewrappte Arrays und einzelne
/// Einträge mit unerwartetem Format (die sonst das ganze Array verwerfen).
fn parse_array_lenient<T: serde::de::DeserializeOwned>(bytes: &[u8], label: &str) -> Vec<T> {
    if bytes.is_empty() {
        return Vec::new();
    }
    let value: serde_json::Value = match serde_json::from_slice(bytes) {
        Ok(v) => v,
        Err(e) => {
            let preview = String::from_utf8_lossy(&bytes[..bytes.len().min(200)]);
            tracing::warn!("{label}: Antwort ist kein gültiges JSON ({e}). Beginn: {preview}");
            return Vec::new();
        }
    };
    // Das Array kann direkt oder unter einem Schlüssel liegen.
    let array = match &value {
        serde_json::Value::Array(a) => a.clone(),
        serde_json::Value::Object(o) => {
            // Häufige Wrapper-Schlüssel probieren.
            o.get("data").or_else(|| o.get("movies")).or_else(|| o.get("streams"))
                .and_then(|v| v.as_array().cloned())
                .unwrap_or_default()
        }
        _ => Vec::new(),
    };
    let total = array.len();
    let mut out = Vec::with_capacity(total);
    let mut skipped = 0usize;
    for item in array {
        match serde_json::from_value::<T>(item) {
            Ok(parsed) => out.push(parsed),
            Err(_) => skipped += 1,
        }
    }
    if skipped > 0 {
        tracing::warn!("{label}: {skipped} von {total} Einträgen übersprungen (Formatfehler)");
    }
    out
}

async fn fetch(client: &reqwest::Client, url: &str) -> CmdResult<Vec<u8>> {
    let resp = client.get(url).send().await.map_err(|e| {
        if e.is_connect() {
            "Verbindung zum Server fehlgeschlagen. Bitte Serveradresse und Internetverbindung prüfen."
                .to_string()
        } else if e.is_timeout() {
            "Zeitüberschreitung beim Server. Bitte später erneut versuchen.".to_string()
        } else {
            exiptv_core::util::sanitize::sanitize_text(&e.to_string())
        }
    })?;
    let status = resp.status();
    if status.as_u16() == 401 || status.as_u16() == 403 {
        return Err("Die Zugangsdaten wurden vom Anbieter abgelehnt (nicht autorisiert). \
                    Bitte überprüfe Benutzername, Passwort und Serveradresse.".to_string());
    }
    if !status.is_success() {
        return Err(format!("Der Server antwortete mit Status {}.", status.as_u16()));
    }
    // Antwort chunk-weise lesen – robust bei sehr großen Playlisten
    // (mehrere hundert MB), wo bytes() mit "error decoding response body"
    // fehlschlagen würde.
    let mut resp = resp;
    let mut buf: Vec<u8> = Vec::new();
    loop {
        match resp.chunk().await {
            Ok(Some(chunk)) => buf.extend_from_slice(&chunk),
            Ok(None) => break,
            Err(e) => return Err(format!("Übertragung abgebrochen: {e}")),
        }
    }
    Ok(buf)
}

/// Lädt eine Kategorienliste und baut eine id→name-Map.
async fn fetch_categories(client: &reqwest::Client, url: &str) -> Option<HashMap<String, String>> {
    let raw = fetch(client, url).await.ok()?;
    let cats: Vec<XtreamCategory> = serde_json::from_slice(&raw).ok()?;
    Some(cats.into_iter().map(|c| (c.category_id, c.category_name)).collect())
}
