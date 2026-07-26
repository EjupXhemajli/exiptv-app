//! Zentrales Datenmodell (Abschnitt 26 der Anforderungen).
//!
//! Die Strukturen spiegeln das SQLite-Schema aus `db::migrations` wider.
//! IDs sind `i64`-Rowids; `provider_id`/`profile_id` stellen die
//! geforderten Zuordnungen her.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    M3uUrl,
    M3uFile,
    Xtream,
    DirectStream,
}

impl ProviderKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            ProviderKind::M3uUrl => "m3u_url",
            ProviderKind::M3uFile => "m3u_file",
            ProviderKind::Xtream => "xtream",
            ProviderKind::DirectStream => "direct",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "m3u_url" => Some(Self::M3uUrl),
            "m3u_file" => Some(Self::M3uFile),
            "xtream" => Some(Self::Xtream),
            "direct" => Some(Self::DirectStream),
            _ => None,
        }
    }
}

/// Sortiervarianten für Kanallisten (Gruppen-Sortierung).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ChannelSort {
    /// Anbieter-Reihenfolge (sort_index), wie importiert.
    #[default]
    Default,
    /// Alphabetisch A–Z.
    NameAsc,
    /// Alphabetisch Z–A.
    NameDesc,
    /// Zuletzt hinzugefügt zuerst.
    RecentlyAdded,
    /// Nach Kanalnummer.
    ChannelNumber,
}

impl ChannelSort {
    pub fn parse(s: &str) -> Self {
        match s {
            "name_asc" => Self::NameAsc,
            "name_desc" => Self::NameDesc,
            "recently_added" => Self::RecentlyAdded,
            "channel_number" => Self::ChannelNumber,
            _ => Self::Default,
        }
    }
}

/// Anbieter / Quelle. Zugangsdaten (Passwörter) liegen NICHT hier,
/// sondern im Windows Credential Manager; `secret_ref` referenziert
/// den dortigen Eintrag.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provider {
    pub id: i64,
    pub name: String,
    pub kind: ProviderKind,
    /// Quelle ohne Zugangsdaten (bereinigt), z. B. Server-URL oder Dateipfad.
    pub source: String,
    /// Benutzername (kein Geheimnis im engeren Sinn, aber log-maskiert).
    pub username: Option<String>,
    /// Referenzschlüssel auf den Credential-Manager-Eintrag.
    pub secret_ref: Option<String>,
    pub enabled: bool,
    pub auto_refresh_hours: Option<i64>,
    pub epg_url: Option<String>,
    pub user_agent: Option<String>,
    pub referer: Option<String>,
    pub last_refresh_at: Option<i64>,
    pub expires_at: Option<i64>,
    pub max_connections: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelGroup {
    pub id: i64,
    pub provider_id: i64,
    pub name: String,
    pub sort_index: i64,
    pub hidden: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Channel {
    pub id: i64,
    pub provider_id: i64,
    pub group_id: Option<i64>,
    pub name: String,
    pub url: String,
    pub tvg_id: Option<String>,
    pub tvg_name: Option<String>,
    pub logo_url: Option<String>,
    pub channel_number: Option<i64>,
    pub is_radio: bool,
    pub catchup: Option<String>,
    pub catchup_days: Option<i64>,
    pub catchup_source: Option<String>,
    pub timeshift: Option<String>,
    pub user_agent: Option<String>,
    pub referer: Option<String>,
    pub hidden: bool,
    pub locked: bool,
    pub sort_index: i64,
}

/// Ergebnis eines Playlist-Imports (für Fortschritt & Protokoll).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ImportReport {
    pub total_lines: usize,
    pub channels_parsed: usize,
    pub channels_skipped: usize,
    pub groups_found: usize,
    #[serde(default)]
    pub movies_parsed: usize,
    #[serde(default)]
    pub series_parsed: usize,
    pub warnings: Vec<String>,
    pub encoding: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserProfile {
    pub id: i64,
    pub name: String,
    pub avatar: Option<String>,
    pub is_kids: bool,
    /// Argon2-/PBKDF2-Hash der PIN — niemals Klartext.
    pub pin_hash: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSetting {
    pub key: String,
    pub value: String,
    pub updated_at: i64,
}

// ------------------------------------------------------------------
// VOD: Filme & Serien (Abschnitt 9)
// ------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Movie {
    pub id: i64,
    pub provider_id: i64,
    pub name: String,
    pub url: String,
    pub category: Option<String>,
    pub poster_url: Option<String>,
    pub backdrop_url: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Series {
    pub id: i64,
    pub provider_id: i64,
    pub external_id: Option<String>,
    pub name: String,
    pub category: Option<String>,
    pub poster_url: Option<String>,
    pub backdrop_url: Option<String>,
    pub plot: Option<String>,
    pub year: Option<i64>,
    pub genre: Option<String>,
    pub rating: Option<f64>,
    pub age_rating: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Season {
    pub id: i64,
    pub series_id: i64,
    pub number: i64,
    pub name: Option<String>,
    pub episode_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Episode {
    pub id: i64,
    pub season_id: i64,
    pub number: i64,
    pub name: Option<String>,
    pub url: String,
    pub plot: Option<String>,
    pub duration_s: Option<i64>,
    pub poster_url: Option<String>,
}

/// Ein Eintrag im Wiedergabeverlauf (für die Verlauf-Ansicht).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub item_type: String,
    pub item_id: i64,
    pub name: String,
    pub url: String,
    pub logo_url: Option<String>,
    pub watched_at: i64,
}
