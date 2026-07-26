//! Abstrakte Playback-Schnittstelle (Anforderung Abschnitt 4).
//!
//! Die konkrete Implementierung (libmpv) lebt in der Tauri-Shell
//! (Phase 4), weil sie Fenster-Handles und native Bibliotheken benötigt.
//! Der Core definiert nur den Vertrag, damit UI, StreamHealthMonitor und
//! Aufnahme-Service gegen eine austauschbare Engine programmieren.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackState {
    Idle,
    Loading,
    Playing,
    Paused,
    Buffering,
    Ended,
    Error,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlaybackStatistics {
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub fps: Option<f64>,
    pub dropped_frames: u64,
    pub bitrate_kbps: Option<u64>,
    pub buffer_seconds: Option<f64>,
    pub hw_decoding: bool,
    pub decoder: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackInfo {
    pub id: i64,
    pub kind: TrackKind,
    pub language: Option<String>,
    pub title: Option<String>,
    pub selected: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrackKind {
    Video,
    Audio,
    Subtitle,
}

#[derive(Debug, thiserror::Error)]
pub enum PlaybackError {
    #[error("Stream konnte nicht geladen werden: {0}")]
    Load(String),
    #[error("Engine nicht initialisiert")]
    NotInitialized,
    #[error("Interner Engine-Fehler: {0}")]
    Engine(String),
}

/// Vertrag für jede Wiedergabe-Engine (libmpv, künftig ggf. libVLC).
pub trait PlaybackEngine: Send {
    fn load(&mut self, url: &str, headers: &[(String, String)]) -> Result<(), PlaybackError>;
    fn play(&mut self) -> Result<(), PlaybackError>;
    fn pause(&mut self) -> Result<(), PlaybackError>;
    fn stop(&mut self) -> Result<(), PlaybackError>;
    fn seek(&mut self, seconds: f64) -> Result<(), PlaybackError>;
    fn set_volume(&mut self, volume: u8) -> Result<(), PlaybackError>;
    fn select_audio_track(&mut self, id: i64) -> Result<(), PlaybackError>;
    fn select_subtitle_track(&mut self, id: Option<i64>) -> Result<(), PlaybackError>;
    fn set_video_track(&mut self, id: i64) -> Result<(), PlaybackError>;
    fn set_aspect_ratio(&mut self, ratio: Option<&str>) -> Result<(), PlaybackError>;
    fn set_deinterlace(&mut self, on: bool) -> Result<(), PlaybackError>;
    fn get_statistics(&self) -> PlaybackStatistics;
    fn get_playback_position(&self) -> Option<f64>;
    fn state(&self) -> PlaybackState;
    fn tracks(&self) -> Vec<TrackInfo>;
    fn reconnect(&mut self) -> Result<(), PlaybackError>;
    fn dispose(&mut self);
}
