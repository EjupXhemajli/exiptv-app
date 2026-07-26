//! libmpv-basierte Wiedergabe-Engine (Phase 4).
//!
//! - `MpvEngine` kapselt eine libmpv-Instanz und implementiert das
//!   `PlaybackEngine`-Trait aus `exiptv-core`.
//! - mpv rendert in ein natives Kindfenster (HWND per `wid`-Property),
//!   damit das Video hardwarebeschleunigt außerhalb der WebView läuft.
//! - Der `StreamHealthMonitor` beobachtet mpv-Properties und stößt die
//!   gestufte Wiederherstellung an.

mod engine;
pub mod health;
mod window;

pub use engine::MpvEngine;
pub use health::{RecoveryAction, StreamHealthMonitor};
pub use window::VideoWindow;
