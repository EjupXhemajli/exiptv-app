//! IPC-Commands für die Wiedergabe (Frontend ↔ Playback-Thread).

use crate::mpv_loader;
use crate::playback_controller::{self, PlaybackCommand};
use crate::state::AppState;
use serde::Deserialize;
use std::sync::mpsc::Sender;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};

/// Hält den Sender zum Playback-Thread. Wird beim ersten Wiedergabe-Command
/// lazy initialisiert (nachdem die DLL verfügbar ist).
#[derive(Default)]
pub struct PlaybackHandle {
    sender: Mutex<Option<Sender<PlaybackCommand>>>,
}

impl PlaybackHandle {
    fn send(&self, cmd: PlaybackCommand) -> Result<(), String> {
        let guard = self.sender.lock().unwrap();
        match guard.as_ref() {
            Some(tx) => tx.send(cmd).map_err(|_| "Wiedergabe ist nicht aktiv.".to_string()),
            None => Err("Wiedergabe wurde noch nicht gestartet.".to_string()),
        }
    }
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MpvStatusEvent {
    pub stage: String,
    pub message: String,
}

/// Stellt sicher, dass die libmpv-DLL vorhanden ist (lädt sie ggf. herunter),
/// startet dann den Playback-Thread. Sendet Fortschritt als `mpv-setup`-Event.
#[tauri::command]
pub async fn ensure_playback_ready(
    app: AppHandle,
    state: State<'_, AppState>,
    playback: State<'_, PlaybackHandle>,
) -> Result<(), String> {
    // Bereits gestartet?
    if playback.sender.lock().unwrap().is_some() {
        return Ok(());
    }

    let app_data = state.app_data_dir.clone();

    // 1) Bevorzugt: die mit dem Installer gebündelte DLL verwenden.
    //    Sie liegt in den App-Ressourcen und wird einmalig ins
    //    Runtime-Verzeichnis kopiert. Das macht den ersten Start
    //    offline-fähig und maximal stabil.
    if !mpv_loader::is_available(&app_data) {
        if let Ok(bundled) = app
            .path()
            .resolve("libmpv-2.dll", tauri::path::BaseDirectory::Resource)
        {
            if bundled.exists() {
                let rt = mpv_loader::runtime_dir(&app_data);
                let _ = std::fs::create_dir_all(&rt);
                let target = mpv_loader::dll_path(&app_data);
                match std::fs::copy(&bundled, &target) {
                    Ok(_) => {
                        let _ = app.emit("mpv-setup", MpvStatusEvent {
                            stage: "vorhanden".into(),
                            message: "Wiedergabe-Komponente ist einsatzbereit.".into(),
                        });
                    }
                    Err(e) => {
                        tracing::warn!("Gebündelte DLL konnte nicht kopiert werden: {e}");
                    }
                }
            }
        }
    }

    // 2) Fallback: DLL herunterladen, falls (noch) nicht vorhanden.
    let app_for_status = app.clone();
    let app_data_dl = app_data.clone();
    let dll_result = tauri::async_runtime::spawn_blocking(move || {
        if mpv_loader::is_available(&app_data_dl) {
            return Ok(mpv_loader::dll_path(&app_data_dl));
        }
        let client = reqwest::blocking::Client::builder()
            .user_agent(concat!("EXIPTV/", env!("CARGO_PKG_VERSION")))
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .map_err(|e| e.to_string())?;
        mpv_loader::ensure_dll_blocking(&app_data_dl, &client, |s| {
            let _ = app_for_status.emit("mpv-setup", MpvStatusEvent {
                stage: s.stage, message: s.message,
            });
        })
    })
    .await
    .map_err(|e| format!("Interner Fehler: {e}"))?;

    dll_result?; // Fehler weiterreichen (nutzerverständliche Meldung).

    // DLL auffindbar machen.
    mpv_loader::ensure_on_path(&state.app_data_dir);

    // Fenster-Handle (HWND) des Hauptfensters holen.
    let wid = main_window_handle(&app)?;

    // Player-Einstellungen aus der DB lesen (mit Defaults).
    let config = {
        let db = state.db.lock().map_err(|_| "Datenbank ist gerade nicht verfügbar.".to_string())?;
        let get = |k: &str| db.get_setting(k).ok().flatten();
        let flag = |k: &str, default: bool| get(k).map(|v| v == "1" || v == "true").unwrap_or(default);

        // Grundpuffer aus dem Puffer-Modus.
        let mut buffer_seconds = match get("pref.bufferMode").as_deref() {
            Some("klein") => 6,
            Some("gross") => 40,
            _ => 20,
        };
        // Feinpuffer überschreibt den Wert, falls gesetzt (1–30 s).
        if let Some(fine) = get("pref.fineBufferSeconds").and_then(|v| v.parse::<u32>().ok()) {
            if fine > 0 {
                buffer_seconds = fine;
            }
        }
        let audio_delay_ms = get("pref.audioDelayMs")
            .and_then(|v| v.parse::<i64>().ok())
            .unwrap_or(0);

        playback_controller::PlaybackConfig {
            buffer_seconds,
            quality: get("pref.quality").unwrap_or_else(|| "auto".into()),
            hardware_decoding: flag("pref.hardwareDecoding", true),
            deinterlace: flag("pref.deinterlace", false),
            reconnect: flag("pref.reconnect", true),
            preferred_audio_lang: get("pref.preferredAudioLang").unwrap_or_default(),
            preferred_subtitle_lang: get("pref.preferredSubtitleLang").unwrap_or_default(),
            volume_normalization: flag("pref.volumeNormalization", false),
            audio_delay_ms,
        }
    };

    // Playback-Thread starten.
    let tx = playback_controller::spawn(app.clone(), wid, config)?;
    *playback.sender.lock().unwrap() = Some(tx);

    let _ = app.emit("mpv-setup", MpvStatusEvent {
        stage: "bereit".into(), message: "Wiedergabe ist bereit.".into(),
    });
    Ok(())
}

#[cfg(target_os = "windows")]
fn main_window_handle(app: &AppHandle) -> Result<i64, String> {
    // In Tauri 2 trägt das Hauptfenster standardmäßig das Label "main".
    let window = app.get_webview_window("main")
        .ok_or_else(|| "Hauptfenster nicht gefunden.".to_string())?;
    match window.hwnd() {
        // h.0 ist das rohe Handle; über isize normalisiert, damit der Cast
        // unabhängig von der windows-crate-Version funktioniert.
        Ok(h) => Ok(h.0 as isize as i64),
        Err(e) => Err(format!("Fenster-Handle nicht verfügbar: {e}")),
    }
}

#[cfg(not(target_os = "windows"))]
fn main_window_handle(_app: &AppHandle) -> Result<i64, String> {
    Err("Wiedergabe ist auf dieser Plattform noch nicht verfügbar.".into())
}

#[derive(Deserialize)]
pub struct LoadArgs {
    pub url: String,
    #[serde(default)]
    pub user_agent: Option<String>,
    #[serde(default)]
    pub referer: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
}

#[tauri::command]
pub async fn playback_load(
    playback: State<'_, PlaybackHandle>,
    args: LoadArgs,
) -> Result<(), String> {
    let mut headers = Vec::new();
    if let Some(ua) = args.user_agent.filter(|s| !s.is_empty()) {
        headers.push(("User-Agent".into(), ua));
    }
    if let Some(rf) = args.referer.filter(|s| !s.is_empty()) {
        headers.push(("Referer".into(), rf));
    }
    playback.send(PlaybackCommand::Load { url: args.url, headers, title: args.title })
}

#[tauri::command]
pub async fn playback_toggle_pause(playback: State<'_, PlaybackHandle>) -> Result<(), String> {
    playback.send(PlaybackCommand::TogglePause)
}

#[tauri::command]
pub async fn playback_stop(playback: State<'_, PlaybackHandle>) -> Result<(), String> {
    playback.send(PlaybackCommand::Stop)
}

#[tauri::command]
pub async fn playback_seek(playback: State<'_, PlaybackHandle>, seconds: f64) -> Result<(), String> {
    playback.send(PlaybackCommand::Seek(seconds))
}

#[tauri::command]
pub async fn playback_seek_relative(playback: State<'_, PlaybackHandle>, delta: f64) -> Result<(), String> {
    playback.send(PlaybackCommand::SeekRelative(delta))
}

#[tauri::command]
pub async fn playback_set_volume(playback: State<'_, PlaybackHandle>, volume: u8) -> Result<(), String> {
    playback.send(PlaybackCommand::SetVolume(volume.min(150)))
}

#[tauri::command]
pub async fn playback_select_audio(playback: State<'_, PlaybackHandle>, id: i64) -> Result<(), String> {
    playback.send(PlaybackCommand::SelectAudio(id))
}

#[tauri::command]
pub async fn playback_select_subtitle(playback: State<'_, PlaybackHandle>, id: Option<i64>) -> Result<(), String> {
    playback.send(PlaybackCommand::SelectSubtitle(id))
}

#[tauri::command]
pub async fn playback_set_aspect(playback: State<'_, PlaybackHandle>, ratio: Option<String>) -> Result<(), String> {
    playback.send(PlaybackCommand::SetAspect(ratio))
}

#[tauri::command]
pub async fn playback_set_deinterlace(playback: State<'_, PlaybackHandle>, on: bool) -> Result<(), String> {
    playback.send(PlaybackCommand::SetDeinterlace(on))
}

/// Videobereich positionieren (Frontend meldet Bounds des Platzhalters,
/// in physischen Pixeln relativ zum Fenster-Client).
#[tauri::command]
pub async fn playback_set_bounds(
    playback: State<'_, PlaybackHandle>,
    x: i32, y: i32, width: i32, height: i32,
) -> Result<(), String> {
    playback.send(PlaybackCommand::SetBounds { x, y, w: width, h: height })
}

#[tauri::command]
pub async fn playback_show_video(playback: State<'_, PlaybackHandle>, visible: bool) -> Result<(), String> {
    playback.send(PlaybackCommand::ShowVideo(visible))
}

#[tauri::command]
pub async fn playback_request_tracks(playback: State<'_, PlaybackHandle>) -> Result<(), String> {
    playback.send(PlaybackCommand::RequestTracks)
}
