//! EXIPTV Kernbibliothek.
//!
//! Enthält alle plattformunabhängigen, headless testbaren Bausteine:
//! Datenmodell, M3U-Parser, SQLite-Schicht mit Migrationen und
//! Staging-Import, Encoding-Erkennung sowie Sicherheits-Hilfsfunktionen
//! (Maskierung von Zugangsdaten in URLs/Logs).
//!
//! Die Tauri-Shell (`src-tauri`) konsumiert diese Bibliothek und ergänzt
//! ausschließlich Plattform-Anbindung (Fenster, IPC, Credential Manager,
//! Playback-Engine).

pub mod db;
pub mod error;
pub mod models;
pub mod parser;
pub mod playback;
pub mod util;

pub use error::{CoreError, Result};
