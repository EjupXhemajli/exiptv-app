//! Toleranter M3U/Extended-M3U-Parser.
//!
//! Design-Ziele (Anforderungen Abschnitt 5):
//! - fehlerhafte/unvollständige Zeilen überspringen und protokollieren,
//!   niemals den Gesamtimport abbrechen
//! - alle gängigen `tvg-*`-/`group-title`-/Catch-up-Attribute
//! - `#EXTGRP`, `#EXTVLCOPT` (User-Agent, Referer), `#KODIPROP` (ignoriert)
//! - beliebige Zeichencodierungen über `util::encoding`
//! - relative URLs gegen eine Basis-URL auflösen
//! - streamingfähig: arbeitet zeilenweise, geeignet für sehr große Listen

use crate::models::ImportReport;
use crate::util::encoding::{decode_bytes, DetectedEncoding};

/// Ein geparster Playlist-Eintrag (noch ohne Datenbank-IDs).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct M3uEntry {
    pub name: String,
    pub url: String,
    pub group: Option<String>,
    pub tvg_id: Option<String>,
    pub tvg_name: Option<String>,
    pub logo_url: Option<String>,
    pub channel_number: Option<i64>,
    pub is_radio: bool,
    pub catchup: Option<String>,
    pub catchup_days: Option<i64>,
    pub catchup_source: Option<String>,
    pub timeshift: Option<String>,
    pub provider: Option<String>,
    pub audio_track: Option<String>,
    pub user_agent: Option<String>,
    pub referer: Option<String>,
    pub duration: Option<f64>,
}

#[derive(Debug)]
pub struct M3uParseResult {
    pub entries: Vec<M3uEntry>,
    pub report: ImportReport,
}

/// Parst rohe Bytes (Datei- oder HTTP-Inhalt) inklusive Encoding-Erkennung.
pub fn parse_bytes(bytes: &[u8], base_url: Option<&str>) -> M3uParseResult {
    let decoded = decode_bytes(bytes);
    let mut result = parse_str(&decoded.text, base_url);
    result.report.encoding = Some(encoding_name(decoded.encoding).to_owned());
    if decoded.had_errors {
        result
            .report
            .warnings
            .push("Einige Zeichen konnten nicht dekodiert werden und wurden ersetzt.".into());
    }
    result
}

/// Parst dekodierten Text.
pub fn parse_str(input: &str, base_url: Option<&str>) -> M3uParseResult {
    let mut entries = Vec::new();
    let mut report = ImportReport::default();
    let mut groups = std::collections::HashSet::new();

    let mut pending: Option<M3uEntry> = None;
    let mut extgrp: Option<String> = None;
    let mut has_header = false;

    for (line_no, raw_line) in input.lines().enumerate() {
        report.total_lines += 1;
        let line = raw_line.trim_start_matches('\u{feff}').trim();
        if line.is_empty() {
            continue;
        }

        if line.starts_with("#EXTM3U") {
            has_header = true;
            continue;
        }

        if let Some(rest) = line.strip_prefix("#EXTINF:") {
            match parse_extinf(rest) {
                Ok(entry) => pending = Some(entry),
                Err(msg) => {
                    report.channels_skipped += 1;
                    report
                        .warnings
                        .push(format!("Zeile {}: {}", line_no + 1, msg));
                    pending = None;
                }
            }
            continue;
        }

        if let Some(rest) = line.strip_prefix("#EXTGRP:") {
            let g = rest.trim();
            extgrp = (!g.is_empty()).then(|| g.to_owned());
            continue;
        }

        if let Some(rest) = line.strip_prefix("#EXTVLCOPT:") {
            if let Some(entry) = pending.as_mut() {
                apply_vlcopt(entry, rest);
            }
            continue;
        }

        if line.starts_with('#') {
            // #KODIPROP, Kommentare, unbekannte Direktiven: bewusst ignorieren.
            continue;
        }

        // URL-Zeile
        let url = match resolve_url(line, base_url) {
            Some(u) => u,
            None => {
                report.channels_skipped += 1;
                report.warnings.push(format!(
                    "Zeile {}: ungültige Stream-URL übersprungen",
                    line_no + 1
                ));
                pending = None;
                continue;
            }
        };

        let mut entry = pending.take().unwrap_or_else(|| M3uEntry {
            // Einfache M3U ohne EXTINF: Name aus URL ableiten.
            name: derive_name_from_url(&url),
            ..Default::default()
        });
        if entry.group.is_none() {
            entry.group = extgrp.clone();
        }
        if entry.name.trim().is_empty() {
            entry.name = entry
                .tvg_name
                .clone()
                .unwrap_or_else(|| derive_name_from_url(&url));
        }
        entry.url = url;
        if let Some(g) = &entry.group {
            groups.insert(g.clone());
        }
        report.channels_parsed += 1;
        entries.push(entry);
    }

    if !has_header && report.channels_parsed > 0 {
        report
            .warnings
            .push("Kein #EXTM3U-Kopf gefunden – Datei wurde dennoch verarbeitet.".into());
    }
    report.groups_found = groups.len();

    M3uParseResult { entries, report }
}

/// Zerlegt den EXTINF-Rest: `<dauer> [attr="v" ...],<Anzeigename>`
fn parse_extinf(rest: &str) -> Result<M3uEntry, String> {
    // Anzeigename ist alles nach dem letzten Komma AUSSERHALB von Anführungszeichen.
    let comma = find_name_comma(rest);
    let (head, name) = match comma {
        Some(i) => (&rest[..i], rest[i + 1..].trim()),
        None => (rest, ""),
    };

    let mut entry = M3uEntry {
        name: name.to_owned(),
        ..Default::default()
    };

    // Dauer = erstes Token bis Leerzeichen (kann fehlen/fehlerhaft sein).
    let head = head.trim();
    let (dur_token, attr_part) = match head.find(char::is_whitespace) {
        Some(i) => (&head[..i], &head[i..]),
        None => (head, ""),
    };
    entry.duration = dur_token.parse::<f64>().ok();

    for (key, value) in parse_attributes(attr_part) {
        let v = value.trim();
        if v.is_empty() {
            continue;
        }
        match key.to_ascii_lowercase().as_str() {
            "tvg-id" => entry.tvg_id = Some(v.to_owned()),
            "tvg-name" => entry.tvg_name = Some(v.to_owned()),
            "tvg-logo" => entry.logo_url = Some(v.to_owned()),
            "tvg-chno" | "channel-number" => entry.channel_number = v.parse().ok(),
            "group-title" => entry.group = Some(v.to_owned()),
            "radio" => entry.is_radio = matches!(v.to_ascii_lowercase().as_str(), "true" | "1" | "yes"),
            "catchup" | "catchup-type" => entry.catchup = Some(v.to_owned()),
            "catchup-days" => entry.catchup_days = v.parse().ok(),
            "catchup-source" => entry.catchup_source = Some(v.to_owned()),
            "timeshift" => entry.timeshift = Some(v.to_owned()),
            "provider" => entry.provider = Some(v.to_owned()),
            "audio-track" => entry.audio_track = Some(v.to_owned()),
            "user-agent" => entry.user_agent = Some(v.to_owned()),
            "referrer" | "referer" => entry.referer = Some(v.to_owned()),
            _ => {}
        }
    }

    if entry.name.is_empty() && entry.tvg_name.is_none() && entry.tvg_id.is_none() {
        // Zulässig: Name kann später aus URL abgeleitet werden — kein Fehler.
    }
    Ok(entry)
}

/// Findet das Komma, das den Anzeigenamen abtrennt (Quotes berücksichtigen).
fn find_name_comma(s: &str) -> Option<usize> {
    let mut in_quotes = false;
    let mut last = None;
    for (i, c) in s.char_indices() {
        match c {
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => last = Some(i),
            _ => {}
        }
    }
    last
}

/// Extrahiert `key="value"`- und `key=value`-Paare tolerant.
fn parse_attributes(s: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        // Key-Anfang suchen
        while i < bytes.len() && !(bytes[i].is_ascii_alphanumeric()) {
            i += 1;
        }
        let key_start = i;
        while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'-' || bytes[i] == b'_') {
            i += 1;
        }
        if key_start == i {
            break;
        }
        let key = &s[key_start..i];
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= bytes.len() || bytes[i] != b'=' {
            continue; // Token ohne '=' ignorieren
        }
        i += 1;
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        let value = if i < bytes.len() && bytes[i] == b'"' {
            i += 1;
            let vstart = i;
            while i < bytes.len() && bytes[i] != b'"' {
                i += 1;
            }
            let v = &s[vstart..i];
            if i < bytes.len() {
                i += 1; // schließendes Quote
            }
            v
        } else {
            let vstart = i;
            while i < bytes.len() && !bytes[i].is_ascii_whitespace() && bytes[i] != b',' {
                i += 1;
            }
            &s[vstart..i]
        };
        out.push((key.to_owned(), value.to_owned()));
    }
    out
}

fn apply_vlcopt(entry: &mut M3uEntry, opt: &str) {
    let mut it = opt.splitn(2, '=');
    let key = it.next().unwrap_or("").trim().to_ascii_lowercase();
    let value = it.next().unwrap_or("").trim();
    if value.is_empty() {
        return;
    }
    match key.as_str() {
        "http-user-agent" => entry.user_agent = Some(value.to_owned()),
        "http-referrer" | "http-referer" => entry.referer = Some(value.to_owned()),
        _ => {}
    }
}

const ALLOWED_SCHEMES: &[&str] = &["http", "https", "rtsp", "rtp", "udp", "rtmp", "mms", "file"];

fn resolve_url(line: &str, base_url: Option<&str>) -> Option<String> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    if let Some(scheme_end) = line.find("://") {
        let scheme = line[..scheme_end].to_ascii_lowercase();
        return ALLOWED_SCHEMES
            .contains(&scheme.as_str())
            .then(|| line.to_owned());
    }
    // Relative URL: nur mit Basis-URL auflösbar.
    let base = base_url?;
    let base = base.trim_end_matches(|c| c != '/'); // bis zum letzten '/'
    if base.is_empty() {
        return None;
    }
    if let Some(rest) = line.strip_prefix('/') {
        // absolut zum Host
        let scheme_end = base.find("://")?;
        let after = &base[scheme_end + 3..];
        let host_end = after.find('/').map(|i| scheme_end + 3 + i).unwrap_or(base.len());
        return Some(format!("{}/{}", &base[..host_end], rest));
    }
    Some(format!("{base}{line}"))
}

fn derive_name_from_url(url: &str) -> String {
    let no_query = url.split(['?', '#']).next().unwrap_or(url);
    let last = no_query.rsplit('/').next().unwrap_or(no_query);
    let stem = last.rsplit_once('.').map(|(s, _)| s).unwrap_or(last);
    if stem.is_empty() {
        "Unbenannter Sender".to_owned()
    } else {
        stem.replace(['_', '-'], " ")
    }
}

fn encoding_name(e: DetectedEncoding) -> &'static str {
    match e {
        DetectedEncoding::Utf8 => "UTF-8",
        DetectedEncoding::Utf8Bom => "UTF-8 (BOM)",
        DetectedEncoding::Utf16Le => "UTF-16 LE",
        DetectedEncoding::Utf16Be => "UTF-16 BE",
        DetectedEncoding::Windows1252 => "Windows-1252",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"#EXTM3U
#EXTINF:-1 tvg-id="daserste.de" tvg-name="Das Erste" tvg-logo="https://logos.example/ard.png" group-title="Öffentlich-Rechtlich" catchup="default" catchup-days="7",Das Erste HD
http://example.org/streams/ard.m3u8
#EXTINF:-1 tvg-id="zdf.de" group-title="Öffentlich-Rechtlich",ZDF HD
http://example.org/streams/zdf.m3u8
#EXTINF:-1 radio="true" tvg-id="dlf.de",Deutschlandfunk
http://example.org/radio/dlf.mp3
"#;

    #[test]
    fn parst_extended_m3u_mit_attributen() {
        let r = parse_str(SAMPLE, None);
        assert_eq!(r.entries.len(), 3);
        assert_eq!(r.report.channels_parsed, 3);
        assert_eq!(r.report.channels_skipped, 0);
        assert_eq!(r.report.groups_found, 1);

        let ard = &r.entries[0];
        assert_eq!(ard.name, "Das Erste HD");
        assert_eq!(ard.tvg_id.as_deref(), Some("daserste.de"));
        assert_eq!(ard.group.as_deref(), Some("Öffentlich-Rechtlich"));
        assert_eq!(ard.catchup.as_deref(), Some("default"));
        assert_eq!(ard.catchup_days, Some(7));
        assert!(r.entries[2].is_radio);
    }

    #[test]
    fn name_mit_komma_in_attributen() {
        let s = "#EXTM3U\n#EXTINF:-1 tvg-name=\"Sport, News\" group-title=\"A,B\",Sport & News, Kanal 1\nhttp://x.tld/s.ts\n";
        let r = parse_str(s, None);
        assert_eq!(r.entries[0].name, "Kanal 1");
        assert_eq!(r.entries[0].group.as_deref(), Some("A,B"));
        assert_eq!(r.entries[0].tvg_name.as_deref(), Some("Sport, News"));
    }

    #[test]
    fn ueberspringt_fehlerhafte_urls_ohne_abbruch() {
        let s = "#EXTM3U\n#EXTINF:-1,Kaputt\nnicht-eine-url\n#EXTINF:-1,Gut\nhttp://x.tld/ok.m3u8\n";
        let r = parse_str(s, None);
        assert_eq!(r.entries.len(), 1);
        assert_eq!(r.entries[0].name, "Gut");
        assert_eq!(r.report.channels_skipped, 1);
        assert!(!r.report.warnings.is_empty());
    }

    #[test]
    fn leere_datei_liefert_leeres_ergebnis() {
        let r = parse_str("", None);
        assert!(r.entries.is_empty());
        assert_eq!(r.report.channels_parsed, 0);
    }

    #[test]
    fn einfache_m3u_ohne_extinf() {
        let s = "http://x.tld/kanal_eins.m3u8\nhttp://x.tld/kanal-zwei.ts\n";
        let r = parse_str(s, None);
        assert_eq!(r.entries.len(), 2);
        assert_eq!(r.entries[0].name, "kanal eins");
        assert!(r.report.warnings.iter().any(|w| w.contains("#EXTM3U")));
    }

    #[test]
    fn extgrp_und_vlcopt() {
        let s = "#EXTM3U\n#EXTGRP:Nachrichten\n#EXTINF:-1,Kanal\n#EXTVLCOPT:http-user-agent=EXIPTV/1.0\n#EXTVLCOPT:http-referrer=https://ref.tld\nhttp://x.tld/n.ts\n";
        let r = parse_str(s, None);
        let e = &r.entries[0];
        assert_eq!(e.group.as_deref(), Some("Nachrichten"));
        assert_eq!(e.user_agent.as_deref(), Some("EXIPTV/1.0"));
        assert_eq!(e.referer.as_deref(), Some("https://ref.tld"));
    }

    #[test]
    fn relative_urls_werden_aufgeloest() {
        let s = "#EXTM3U\n#EXTINF:-1,A\nsegmente/a.m3u8\n#EXTINF:-1,B\n/root/b.m3u8\n";
        let r = parse_str(s, Some("http://host.tld/listen/haupt.m3u"));
        assert_eq!(r.entries[0].url, "http://host.tld/listen/segmente/a.m3u8");
        assert_eq!(r.entries[1].url, "http://host.tld/root/b.m3u8");
    }

    #[test]
    fn windows_1252_bytes_werden_korrekt_gelesen() {
        let mut bytes = b"#EXTM3U\n#EXTINF:-1 group-title=\"M\xFCnchen\",Bayerisches TV\nhttp://x.tld/by.ts\n".to_vec();
        bytes.shrink_to_fit();
        let r = parse_bytes(&bytes, None);
        assert_eq!(r.entries[0].group.as_deref(), Some("München"));
        assert_eq!(r.report.encoding.as_deref(), Some("Windows-1252"));
    }

    #[test]
    fn grosse_playlist_bleibt_schnell() {
        let mut s = String::from("#EXTM3U\n");
        for i in 0..10_000 {
            s.push_str(&format!(
                "#EXTINF:-1 tvg-id=\"ch{i}\" group-title=\"G{}\",Kanal {i}\nhttp://x.tld/{i}.m3u8\n",
                i % 50
            ));
        }
        let t = std::time::Instant::now();
        let r = parse_str(&s, None);
        assert_eq!(r.entries.len(), 10_000);
        assert_eq!(r.report.groups_found, 50);
        assert!(t.elapsed().as_millis() < 2_000, "Parser zu langsam: {:?}", t.elapsed());
    }

    #[test]
    fn name_faellt_auf_tvg_name_zurueck() {
        let s = "#EXTM3U\n#EXTINF:-1 tvg-name=\"Fallback TV\",\nhttp://x.tld/f.ts\n";
        let r = parse_str(s, None);
        assert_eq!(r.entries[0].name, "Fallback TV");
    }
}
