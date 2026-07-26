use thiserror::Error;

/// Zentrale Fehlerhierarchie des Cores.
///
/// Jede Variante trägt genug Kontext für die Diagnoseansicht,
/// aber niemals Zugangsdaten (siehe `util::sanitize`).
#[derive(Debug, Error)]
pub enum CoreError {
    #[error("Datenbankfehler: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("Migration {version} fehlgeschlagen: {message}")]
    Migration { version: i64, message: String },

    #[error("Playlist konnte nicht verarbeitet werden: {0}")]
    Playlist(String),

    #[error("Ein-/Ausgabefehler: {0}")]
    Io(#[from] std::io::Error),

    #[error("Ungültige Eingabe: {0}")]
    InvalidInput(String),

    #[error("Serialisierung fehlgeschlagen: {0}")]
    Serde(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, CoreError>;
