//! StreamHealthMonitor (Anforderung Abschnitt 6).
//!
//! Beobachtet den mpv-Zustand in kurzen Intervallen und erkennt:
//! - keine neuen Videoframes / eingefrorenes Bild
//! - dauerhaftes Puffern (leerer Cache)
//! - Wiedergabeende bei Live-Streams (Verbindungsabbruch)
//!
//! Reaktion (gestuft, wie gefordert): erst intern (mpv reconnectet via
//! stream-lavf-o selbst), dann erneutes Laden der URL, danach Meldung an
//! das Frontend. Der Monitor löst KEIN störendes Fehlerfenster aus, sondern
//! sendet strukturierte Ereignisse; die UI entscheidet über die Darstellung.

use serde::Serialize;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthReport {
    pub healthy: bool,
    pub reason: Option<String>,
    /// Interner Wiederherstellungsversuch läuft (UI zeigt dezenten Hinweis).
    pub recovering: bool,
    pub recovery_stage: u8,
}

/// Zustandsmaschine des Monitors. Bewusst ohne eigenen Thread: Wird vom
/// Playback-Poll-Loop im Command-Layer getaktet, damit alle mpv-Zugriffe
/// serialisiert bleiben.
pub struct StreamHealthMonitor {
    last_frame_count: i64,
    last_progress: Instant,
    stage: u8,
    stage_since: Instant,
    stall_grace: Duration,
    stage_grace: Duration,
}

impl Default for StreamHealthMonitor {
    fn default() -> Self { Self::new() }
}

impl StreamHealthMonitor {
    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            last_frame_count: -1,
            last_progress: now,
            stage: 0,
            stage_since: now,
            stall_grace: Duration::from_secs(6),
            stage_grace: Duration::from_secs(8),
        }
    }

    pub fn reset(&mut self) {
        let now = Instant::now();
        self.last_frame_count = -1;
        self.last_progress = now;
        self.stage = 0;
        self.stage_since = now;
    }

    /// Einen Tick auswerten.
    /// `frame_count`: dekodierte Videoframes (monoton steigend bei gesundem Stream).
    /// `buffering`: mpv wartet auf Cache.
    /// `ended`: Wiedergabe beendet (bei Live = Abbruch).
    ///
    /// Rückgabe `RecoveryAction`, was der Aufrufer tun soll.
    pub fn tick(&mut self, frame_count: i64, buffering: bool, ended: bool) -> RecoveryAction {
        let now = Instant::now();

        // Fortschritt erkannt?
        if frame_count > self.last_frame_count {
            self.last_frame_count = frame_count;
            self.last_progress = now;
            if self.stage != 0 {
                // Erholt.
                self.stage = 0;
                self.stage_since = now;
                return RecoveryAction::Recovered;
            }
            return RecoveryAction::None;
        }

        let stalled = ended
            || (now.duration_since(self.last_progress) > self.stall_grace)
            || (buffering && now.duration_since(self.last_progress) > self.stall_grace);

        if !stalled {
            return RecoveryAction::None;
        }

        // Stillstand: gestuft eskalieren, aber jeder Stufe Zeit geben.
        if now.duration_since(self.stage_since) < self.stage_grace && self.stage != 0 {
            return RecoveryAction::Waiting(self.stage);
        }

        self.stage = self.stage.saturating_add(1);
        self.stage_since = now;
        match self.stage {
            1 => RecoveryAction::Wait,        // mpv reconnectet intern; abwarten
            2 => RecoveryAction::Reload,      // URL neu laden
            3 => RecoveryAction::Reload,      // zweiter Versuch
            _ => RecoveryAction::GiveUp,      // Nutzer informieren
        }
    }

    pub fn stage(&self) -> u8 { self.stage }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryAction {
    None,
    Recovered,
    Wait,
    Waiting(u8),
    Reload,
    GiveUp,
}
