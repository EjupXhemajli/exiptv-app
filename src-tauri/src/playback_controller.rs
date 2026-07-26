//! Playback-Controller: verbindet mpv-Engine, Videofenster und
//! StreamHealthMonitor mit dem IPC-Layer.
//!
//! Ein einzelner Hintergrund-Thread besitzt die mpv-Instanz (mpv ist nicht
//! thread-sicher gemeinsam nutzbar). Commands vom Frontend werden über einen
//! Kanal an diesen Thread geschickt; Statusänderungen laufen als Tauri-Events
//! zurück. So bleiben alle mpv-Zugriffe serialisiert und die UI reaktiv.

use crate::playback::{MpvEngine, RecoveryAction, StreamHealthMonitor, VideoWindow};
use exiptv_core::playback::{PlaybackEngine, PlaybackState};
use std::sync::mpsc::{Receiver, Sender};
use std::time::Duration;
use tauri::{AppHandle, Emitter};

/// Befehle an den Playback-Thread.
pub enum PlaybackCommand {
    Load { url: String, headers: Vec<(String, String)>, title: Option<String> },
    Play,
    Pause,
    TogglePause,
    Stop,
    Seek(f64),
    SeekRelative(f64),
    SetVolume(u8),
    SelectAudio(i64),
    SelectSubtitle(Option<i64>),
    SetAspect(Option<String>),
    SetDeinterlace(bool),
    SetBounds { x: i32, y: i32, w: i32, h: i32 },
    ShowVideo(bool),
    RequestTracks,
    Shutdown,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackStatusEvent {
    pub state: PlaybackState,
    pub position: Option<f64>,
    pub duration: Option<f64>,
    pub volume: u8,
    pub recovering: bool,
    pub recovery_stage: u8,
    pub title: Option<String>,
    pub tracks: Vec<exiptv_core::playback::TrackInfo>,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackErrorEvent {
    pub message: String,
}

/// Player-Einstellungen, aus den App-Settings gelesen und an mpv übergeben.
#[derive(Clone, Debug)]
pub struct PlaybackConfig {
    pub buffer_seconds: u32,
    pub quality: String,        // auto|high|medium|low
    pub hardware_decoding: bool,
    pub deinterlace: bool,
    pub reconnect: bool,
    pub preferred_audio_lang: String,
    pub preferred_subtitle_lang: String,
    pub volume_normalization: bool,
    pub audio_delay_ms: i64,
}

impl Default for PlaybackConfig {
    fn default() -> Self {
        Self {
            buffer_seconds: 20,
            quality: "auto".into(),
            hardware_decoding: true,
            deinterlace: false,
            reconnect: true,
            preferred_audio_lang: String::new(),
            preferred_subtitle_lang: String::new(),
            volume_normalization: false,
            audio_delay_ms: 0,
        }
    }
}

/// Startet den Playback-Thread und liefert den Sender für Commands.
pub fn spawn(app: AppHandle, wid: i64, config: PlaybackConfig) -> Result<Sender<PlaybackCommand>, String> {
    let (tx, rx) = std::sync::mpsc::channel::<PlaybackCommand>();
    std::thread::Builder::new()
        .name("exiptv-playback".into())
        .spawn(move || playback_loop(app, wid, config, rx))
        .map_err(|e| format!("Playback-Thread konnte nicht gestartet werden: {e}"))?;
    Ok(tx)
}

fn playback_loop(app: AppHandle, wid: i64, config: PlaybackConfig, rx: Receiver<PlaybackCommand>) {
    let video = match VideoWindow::new(wid as isize) {
        Ok(v) => v,
        Err(e) => {
            let _ = app.emit("playback-error", PlaybackErrorEvent {
                message: format!("Videobereich konnte nicht eingerichtet werden: {e}"),
            });
            return;
        }
    };

    let mut engine = match MpvEngine::new(video.wid(), &config) {
        Ok(e) => e,
        Err(e) => {
            let _ = app.emit("playback-error", PlaybackErrorEvent {
                message: format!(
                    "Die Wiedergabe konnte nicht gestartet werden. {e} \
                     Bitte starte EXIPTV neu; die Wiedergabe-Komponente wird dann erneut geprüft."
                ),
            });
            return;
        }
    };

    let mut monitor = StreamHealthMonitor::new();
    let mut volume: u8 = 100;
    let mut title: Option<String> = None;
    let mut active = false;
    let mut recovering = false;
    let mut last_track_count: i64 = -1;

    let poll = Duration::from_millis(250);
    loop {
        // Alle anstehenden Commands abarbeiten.
        loop {
            match rx.try_recv() {
                Ok(cmd) => {
                    let is_load = matches!(cmd, PlaybackCommand::Load { .. });
                    if handle_command(&mut engine, &video, &mut volume, &mut title,
                                      &mut active, &mut monitor, cmd) {
                        engine.dispose();
                        return;
                    }
                    // Nach einem neuen Load die Spurerkennung zurücksetzen,
                    // damit die Spuren des neuen Streams gesendet werden.
                    if is_load { last_track_count = -1; }
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    engine.dispose();
                    return;
                }
            }
        }

        engine.pump_events();

        if active {
            // Gesundheit prüfen: monoton steigende Frame-/Zeitmarke.
            let frame_count = decoded_frames(&engine);
            let st = engine.state();
            let buffering = st == PlaybackState::Buffering;
            let ended = st == PlaybackState::Ended;

            match monitor.tick(frame_count, buffering, ended) {
                RecoveryAction::None => { recovering = false; }
                RecoveryAction::Waiting(_) | RecoveryAction::Wait => {
                    recovering = true;
                    let _ = app.emit("playback-recovering", monitor.stage());
                }
                RecoveryAction::Reload => {
                    recovering = true;
                    let _ = app.emit("playback-recovering", monitor.stage());
                    if let Err(e) = engine.reconnect() {
                        tracing::warn!("Reconnect fehlgeschlagen: {e}");
                    }
                }
                RecoveryAction::Recovered => {
                    recovering = false;
                    let _ = app.emit("playback-recovered", ());
                }
                RecoveryAction::GiveUp => {
                    recovering = false;
                    active = false;
                    let _ = app.emit("playback-error", PlaybackErrorEvent {
                        message: "Der Sender konnte momentan nicht dauerhaft geladen werden. \
                                  Bitte versuche es später erneut oder wähle einen anderen Sender.".into(),
                    });
                }
            }

            // Spuren nur abfragen, wenn sich ihre Anzahl geändert hat
            // (spart mpv-Property-Zugriffe bei jedem Tick).
            let track_count = engine.track_count();
            let tracks = if track_count != last_track_count {
                last_track_count = track_count;
                engine.tracks()
            } else {
                Vec::new()
            };

            // Status an die UI.
            let _ = app.emit("playback-status", PlaybackStatusEvent {
                state: engine.state(),
                position: engine.get_playback_position(),
                duration: duration(&engine),
                volume,
                recovering,
                recovery_stage: monitor.stage(),
                title: title.clone(),
                tracks,
            });
        }

        // Windows-Nachrichten des Videofensters verarbeiten und dabei warten.
        // Das Fenster gehört diesem Thread, deshalb MUSS dieser Thread seine
        // Nachrichten abholen – sonst blockiert der Hauptthread und die
        // Oberfläche friert ein („Keine Rückmeldung").
        let steps = 10;
        let step = poll / steps;
        for _ in 0..steps {
            crate::playback::pump_thread_messages();
            // Zwischendurch auf neue Commands prüfen, damit Stop/Schließen
            // sofort greift und nicht bis zu 250 ms verzögert wird.
            if let Ok(cmd) = rx.try_recv() {
                let is_load = matches!(cmd, PlaybackCommand::Load { .. });
                if handle_command(&mut engine, &video, &mut volume, &mut title,
                                  &mut active, &mut monitor, cmd) {
                    engine.dispose();
                    return;
                }
                if is_load { last_track_count = -1; }
            }
            std::thread::sleep(step);
        }
    }
}

fn handle_command(
    engine: &mut MpvEngine,
    video: &VideoWindow,
    volume: &mut u8,
    title: &mut Option<String>,
    active: &mut bool,
    monitor: &mut StreamHealthMonitor,
    cmd: PlaybackCommand,
) -> bool {
    match cmd {
        PlaybackCommand::Load { url, headers, title: t } => {
            *title = t;
            monitor.reset();
            video.show(true);
            if let Err(e) = engine.load(&url, &headers) {
                tracing::warn!("Laden fehlgeschlagen: {e}");
                *active = false;
            } else {
                let _ = engine.set_volume(*volume);
                *active = true;
            }
        }
        PlaybackCommand::Play => { let _ = engine.play(); }
        PlaybackCommand::Pause => { let _ = engine.pause(); }
        PlaybackCommand::TogglePause => {
            match engine.state() {
                PlaybackState::Playing | PlaybackState::Buffering => { let _ = engine.pause(); }
                _ => { let _ = engine.play(); }
            }
        }
        PlaybackCommand::Stop => {
            let _ = engine.stop();
            video.show(false);
            *active = false;
        }
        PlaybackCommand::Seek(s) => { let _ = engine.seek(s); }
        PlaybackCommand::SeekRelative(delta) => { let _ = engine.seek_relative(delta); }
        PlaybackCommand::SetVolume(v) => { *volume = v; let _ = engine.set_volume(v); }
        PlaybackCommand::SelectAudio(id) => { let _ = engine.select_audio_track(id); }
        PlaybackCommand::SelectSubtitle(id) => { let _ = engine.select_subtitle_track(id); }
        PlaybackCommand::SetAspect(r) => { let _ = engine.set_aspect_ratio(r.as_deref()); }
        PlaybackCommand::SetDeinterlace(on) => { let _ = engine.set_deinterlace(on); }
        PlaybackCommand::SetBounds { x, y, w, h } => { video.set_bounds(x, y, w, h); }
        PlaybackCommand::ShowVideo(v) => { video.show(v); }
        PlaybackCommand::RequestTracks => { /* Spuren gehen im Status-Event mit */ }
        PlaybackCommand::Shutdown => return true,
    }
    false
}

// Dekodierte Frames als Fortschrittsindikator (mpv-Property).
fn decoded_frames(engine: &MpvEngine) -> i64 {
    engine.decoded_frame_count()
}
fn duration(engine: &MpvEngine) -> Option<f64> {
    engine.duration()
}
