//! Xtream-Codes-Client-Logik: URL-Erzeugung und Antwort-Parsing.
//!
//! Der eigentliche HTTP-Aufruf liegt in der Tauri-Schicht (reqwest); dieses
//! Modul ist rein und damit headless testbar. Es baut die korrekten
//! `player_api.php`-URLs mit Zugangsdaten und wandelt die JSON-Antworten in
//! unser Datenmodell.
//!
//! API-Referenz (De-facto-Standard):
//! - Auth:        `{base}/player_api.php?username=U&password=P`
//! - Live:        `&action=get_live_streams`  → Stream `{base}/live/U/P/{id}.ts`
//! - VOD:         `&action=get_vod_streams`    → Stream `{base}/movie/U/P/{id}.{ext}`
//! - Serien:      `&action=get_series`         → `&action=get_series_info&series_id=ID`
//! - Kategorien:  `get_live_categories`, `get_vod_categories`, `get_series_categories`

use serde::Deserialize;

/// Zugangsdaten und normalisierte Basis-URL eines Xtream-Anbieters.
#[derive(Debug, Clone)]
pub struct XtreamCreds {
    /// Basis wie `http://host:port` (ohne abschließenden Slash, ohne Pfad).
    pub base: String,
    pub username: String,
    pub password: String,
}

impl XtreamCreds {
    /// Normalisiert eine vom Nutzer eingegebene Serveradresse:
    /// - fügt `http://` hinzu, falls kein Schema angegeben
    /// - entfernt einen evtl. mitkopierten `/player_api.php`- oder
    ///   `/get.php`-Pfad samt Query
    /// - entfernt abschließende Slashes
    pub fn new(server: &str, username: &str, password: &str) -> Self {
        let mut s = server.trim().to_string();
        if !s.contains("://") {
            s = format!("http://{s}");
        }
        // Query abschneiden.
        if let Some(pos) = s.find('?') {
            s.truncate(pos);
        }
        // Bekannte Pfad-Endungen entfernen.
        for suffix in ["/player_api.php", "/get.php", "/panel_api.php", "/xmltv.php"] {
            if let Some(stripped) = s.strip_suffix(suffix) {
                s = stripped.to_string();
                break;
            }
        }
        while s.ends_with('/') {
            s.pop();
        }
        Self {
            base: s,
            username: username.trim().to_string(),
            password: password.to_string(),
        }
    }

    fn api(&self, action: &str) -> String {
        format!(
            "{}/player_api.php?username={}&password={}&action={}",
            self.base,
            urlencode(&self.username),
            urlencode(&self.password),
            action
        )
    }

    /// Authentifizierungs-/Kontostatus-URL (ohne action).
    pub fn auth_url(&self) -> String {
        format!(
            "{}/player_api.php?username={}&password={}",
            self.base,
            urlencode(&self.username),
            urlencode(&self.password)
        )
    }

    pub fn live_categories_url(&self) -> String { self.api("get_live_categories") }
    pub fn live_streams_url(&self) -> String { self.api("get_live_streams") }
    pub fn vod_categories_url(&self) -> String { self.api("get_vod_categories") }
    pub fn vod_streams_url(&self) -> String { self.api("get_vod_streams") }
    pub fn series_categories_url(&self) -> String { self.api("get_series_categories") }
    pub fn series_url(&self) -> String { self.api("get_series") }
    pub fn series_info_url(&self, series_id: i64) -> String {
        format!("{}&series_id={}", self.api("get_series_info"), series_id)
    }

    /// Stream-URL eines Live-Kanals (Container `ts`).
    pub fn live_stream_url(&self, stream_id: i64) -> String {
        format!("{}/live/{}/{}/{}.ts", self.base, urlencode(&self.username), urlencode(&self.password), stream_id)
    }
    /// Stream-URL eines Films (Container aus `container_extension`).
    pub fn vod_stream_url(&self, stream_id: i64, ext: &str) -> String {
        let ext = if ext.is_empty() { "mp4" } else { ext };
        format!("{}/movie/{}/{}/{}.{}", self.base, urlencode(&self.username), urlencode(&self.password), stream_id, ext)
    }
    /// Stream-URL einer Serien-Episode.
    pub fn series_stream_url(&self, episode_id: i64, ext: &str) -> String {
        let ext = if ext.is_empty() { "mp4" } else { ext };
        format!("{}/series/{}/{}/{}.{}", self.base, urlencode(&self.username), urlencode(&self.password), episode_id, ext)
    }
    /// XMLTV-EPG-URL des Anbieters.
    pub fn epg_url(&self) -> String {
        format!("{}/xmltv.php?username={}&password={}", self.base, urlencode(&self.username), urlencode(&self.password))
    }

    /// M3U-Playlist-URL (get.php, type=m3u_plus). Fallback, wenn
    /// player_api.php nicht nutzbar ist.
    pub fn m3u_url(&self) -> String {
        format!(
            "{}/get.php?username={}&password={}&type=m3u_plus&output=ts",
            self.base, urlencode(&self.username), urlencode(&self.password)
        )
    }
}

/// Minimale URL-Kodierung für Benutzername/Passwort in der Query.
fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

// ===== API-Antwortstrukturen =====

/// Antwort von `player_api.php` (Auth): Konto- und Serverinfo.
#[derive(Debug, Deserialize)]
pub struct AuthResponse {
    pub user_info: Option<UserInfo>,
    pub server_info: Option<ServerInfo>,
}

#[derive(Debug, Deserialize)]
pub struct UserInfo {
    #[serde(default)]
    pub auth: i64,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub exp_date: Option<String>,
    #[serde(default)]
    pub max_connections: Option<String>,
    #[serde(default)]
    pub active_cons: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ServerInfo {
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub port: Option<String>,
}

/// Kategorie (Live/VOD/Serie – gleiche Struktur).
#[derive(Debug, Deserialize)]
pub struct XtreamCategory {
    pub category_id: String,
    pub category_name: String,
}

/// Live-Stream-Eintrag.
#[derive(Debug, Deserialize)]
pub struct LiveStream {
    #[serde(deserialize_with = "de_i64")]
    pub stream_id: i64,
    pub name: String,
    #[serde(default)]
    pub stream_icon: Option<String>,
    #[serde(default)]
    pub epg_channel_id: Option<String>,
    #[serde(default)]
    pub category_id: Option<String>,
    #[serde(default, deserialize_with = "de_opt_i64")]
    pub num: Option<i64>,
}

/// VOD-/Film-Eintrag.
#[derive(Debug, Deserialize)]
pub struct VodStream {
    #[serde(deserialize_with = "de_i64")]
    pub stream_id: i64,
    pub name: String,
    #[serde(default)]
    pub stream_icon: Option<String>,
    #[serde(default)]
    pub category_id: Option<String>,
    #[serde(default)]
    pub container_extension: Option<String>,
    #[serde(default)]
    pub rating: Option<String>,
    #[serde(default)]
    pub added: Option<String>,
}

/// Serien-Eintrag (Liste).
#[derive(Debug, Deserialize)]
pub struct SeriesEntry {
    #[serde(deserialize_with = "de_i64")]
    pub series_id: i64,
    pub name: String,
    #[serde(default)]
    pub cover: Option<String>,
    #[serde(default)]
    pub category_id: Option<String>,
    #[serde(default)]
    pub plot: Option<String>,
    #[serde(default)]
    pub genre: Option<String>,
    #[serde(default)]
    pub rating: Option<String>,
    #[serde(default, alias = "releaseDate", alias = "release_date")]
    pub release_date: Option<String>,
}

/// Antwort von `get_series_info`: Staffeln/Episoden.
#[derive(Debug, Deserialize)]
pub struct SeriesInfo {
    #[serde(default)]
    pub episodes: std::collections::HashMap<String, Vec<EpisodeEntry>>,
    #[serde(default)]
    pub info: Option<SeriesInfoMeta>,
}

#[derive(Debug, Deserialize)]
pub struct SeriesInfoMeta {
    #[serde(default)]
    pub plot: Option<String>,
    #[serde(default)]
    pub cover: Option<String>,
    #[serde(default)]
    pub genre: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct EpisodeEntry {
    #[serde(deserialize_with = "de_i64_str")]
    pub id: i64,
    #[serde(default, deserialize_with = "de_opt_i64")]
    pub episode_num: Option<i64>,
    pub title: Option<String>,
    #[serde(default)]
    pub container_extension: Option<String>,
    #[serde(default)]
    pub info: Option<EpisodeInfo>,
}

#[derive(Debug, Deserialize)]
pub struct EpisodeInfo {
    #[serde(default)]
    pub plot: Option<String>,
    #[serde(default, deserialize_with = "de_opt_i64")]
    pub duration_secs: Option<i64>,
    #[serde(default)]
    pub movie_image: Option<String>,
}

// ===== Deserialisierungs-Helfer =====
// Xtream-Panels liefern Zahlen mal als Zahl, mal als String – wir tolerieren beides.

use serde::de::{self, Deserializer};

fn de_i64<'de, D: Deserializer<'de>>(d: D) -> Result<i64, D::Error> {
    let v = serde_json::Value::deserialize(d)?;
    value_to_i64(&v).ok_or_else(|| de::Error::custom("erwartete Zahl"))
}

fn de_i64_str<'de, D: Deserializer<'de>>(d: D) -> Result<i64, D::Error> {
    // id kann String oder Zahl sein.
    de_i64(d)
}

fn de_opt_i64<'de, D: Deserializer<'de>>(d: D) -> Result<Option<i64>, D::Error> {
    let v = serde_json::Value::deserialize(d)?;
    Ok(value_to_i64(&v))
}

fn value_to_i64(v: &serde_json::Value) -> Option<i64> {
    match v {
        serde_json::Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)),
        serde_json::Value::String(s) => s.trim().parse::<i64>().ok(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_normalisierung() {
        // get.php-Link wird auf die Basis reduziert.
        let c = XtreamCreds::new("http://host.tld:8080/get.php?username=a&password=b&type=m3u_plus", "a", "b");
        assert_eq!(c.base, "http://host.tld:8080");
        // Ohne Schema → http:// ergänzt.
        let c2 = XtreamCreds::new("host.tld:8080", "u", "p");
        assert_eq!(c2.base, "http://host.tld:8080");
        // Abschließende Slashes weg.
        let c3 = XtreamCreds::new("http://host.tld/", "u", "p");
        assert_eq!(c3.base, "http://host.tld");
    }

    #[test]
    fn api_urls_korrekt() {
        let c = XtreamCreds::new("http://host.tld:8080", "user@x", "pa ss");
        // Sonderzeichen werden kodiert.
        assert!(c.auth_url().contains("username=user%40x"));
        assert!(c.auth_url().contains("password=pa%20ss"));
        assert!(c.live_streams_url().ends_with("action=get_live_streams"));
        assert_eq!(
            c.live_stream_url(42),
            "http://host.tld:8080/live/user%40x/pa%20ss/42.ts"
        );
        assert_eq!(
            c.vod_stream_url(7, "mkv"),
            "http://host.tld:8080/movie/user%40x/pa%20ss/7.mkv"
        );
    }

    #[test]
    fn auth_antwort_parsen() {
        let json = r#"{"user_info":{"auth":1,"status":"Active","exp_date":"1750000000","max_connections":"2","active_cons":"0"},"server_info":{"url":"host.tld","port":"8080"}}"#;
        let r: AuthResponse = serde_json::from_str(json).unwrap();
        let ui = r.user_info.unwrap();
        assert_eq!(ui.auth, 1);
        assert_eq!(ui.status, "Active");
    }

    #[test]
    fn live_streams_mit_string_und_zahl_ids() {
        // stream_id als Zahl, num als String – beide müssen klappen.
        let json = r#"[{"stream_id":123,"name":"Kanal 1","num":"5","stream_icon":"http://logo/1.png","category_id":"3"}]"#;
        let v: Vec<LiveStream> = serde_json::from_str(json).unwrap();
        assert_eq!(v[0].stream_id, 123);
        assert_eq!(v[0].num, Some(5));
        assert_eq!(v[0].name, "Kanal 1");
    }

    #[test]
    fn series_info_episoden() {
        let json = r#"{"info":{"plot":"P","genre":"Drama"},"episodes":{"1":[{"id":"555","episode_num":1,"title":"Pilot","container_extension":"mkv","info":{"plot":"E","duration_secs":2700}}]}}"#;
        let si: SeriesInfo = serde_json::from_str(json).unwrap();
        let s1 = &si.episodes["1"];
        assert_eq!(s1[0].id, 555);
        assert_eq!(s1[0].episode_num, Some(1));
        assert_eq!(s1[0].info.as_ref().unwrap().duration_secs, Some(2700));
    }
}

#[cfg(test)]
mod robustness_tests {
    use super::*;

    #[test]
    fn vod_realistisch_mit_gemischten_typen() {
        // Realistische Xtream-VOD-Antwort: stream_id als Zahl UND als String,
        // fehlende optionale Felder, rating als String.
        let json = r#"[
          {"num":1,"name":"Film A","stream_type":"movie","stream_id":12345,"stream_icon":"http://x/a.jpg","rating":"7.5","category_id":"10","container_extension":"mkv","added":"1600000000"},
          {"num":2,"name":"Film B","stream_type":"movie","stream_id":"67890","stream_icon":"","rating":"0","category_id":"10","container_extension":"mp4"},
          {"num":3,"name":"Film C ohne icon","stream_id":11111,"category_id":"11","container_extension":"avi"}
        ]"#;
        let v: Vec<VodStream> = serde_json::from_str(json).unwrap();
        assert_eq!(v.len(), 3);
        assert_eq!(v[0].stream_id, 12345);
        assert_eq!(v[1].stream_id, 67890); // String→Zahl
        assert_eq!(v[2].container_extension.as_deref(), Some("avi"));
    }

    #[test]
    fn series_realistisch() {
        let json = r#"[
          {"num":1,"name":"Serie A","series_id":555,"cover":"http://x/s.jpg","plot":"...","genre":"Drama","rating":"8","releaseDate":"2020-05-01","category_id":"20"},
          {"num":2,"name":"Serie B","series_id":"556","category_id":"20"}
        ]"#;
        let v: Vec<SeriesEntry> = serde_json::from_str(json).unwrap();
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].series_id, 555);
        assert_eq!(v[1].series_id, 556);
        assert_eq!(v[0].release_date.as_deref(), Some("2020-05-01"));
    }

    #[test]
    fn vod_stream_url_verschiedene_endungen() {
        let c = XtreamCreds::new("http://srv:8080", "u", "p");
        assert_eq!(c.vod_stream_url(5, "mkv"), "http://srv:8080/movie/u/p/5.mkv");
        // Leere Endung → mp4-Fallback.
        assert_eq!(c.vod_stream_url(5, ""), "http://srv:8080/movie/u/p/5.mp4");
    }
}
