//! Klassifizierung von M3U-Einträgen aus `m3u_plus`-Playlisten in
//! Live-TV, Film oder Serie.
//!
//! Xtream-Server liefern in `type=m3u_plus`-Playlisten auch VOD-Inhalte.
//! Diese lassen sich am URL-Pfad erkennen:
//! - `/live/USER/PASS/ID.ext`   → Live-TV
//! - `/movie/USER/PASS/ID.ext`  → Film
//! - `/series/USER/PASS/ID.ext` → Serien-Episode
//!
//! So kann die App auch ohne funktionierendes `player_api.php` Filme und
//! Serien getrennt darstellen.

/// Art eines Playlist-Eintrags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamKind {
    Live,
    Movie,
    Series,
}

/// Erkennt die Art eines Eintrags anhand der Stream-URL.
pub fn classify(url: &str) -> StreamKind {
    // Auf den Pfad-Teil schauen (Query ignorieren).
    let path = url.split('?').next().unwrap_or(url).to_ascii_lowercase();
    if path.contains("/movie/") || path.contains("/vod/") {
        StreamKind::Movie
    } else if path.contains("/series/") {
        StreamKind::Series
    } else {
        // Alles andere (inkl. /live/ und klassische TS-Streams) ist Live-TV.
        StreamKind::Live
    }
}

/// Zerlegt einen Serien-Episoden-Titel in (Serienname, Staffel, Episode).
///
/// Xtream-M3U-Titel folgen oft Mustern wie:
/// - "Serienname S01 E05"
/// - "Serienname S01E05"
/// - "Serienname 1x05"
/// Wird nichts erkannt, ist Staffel/Episode None und der ganze Titel gilt
/// als Serienname.
pub fn parse_series_title(title: &str) -> (String, Option<i64>, Option<i64>) {
    let t = title.trim();

    // Muster "SxxExx" bzw. "Sxx Exx".
    if let Some((name, s, e)) = try_sxxexx(t) {
        return (name, Some(s), Some(e));
    }
    // Muster "1x05".
    if let Some((name, s, e)) = try_nxn(t) {
        return (name, Some(s), Some(e));
    }
    (t.to_string(), None, None)
}

fn try_sxxexx(t: &str) -> Option<(String, i64, i64)> {
    // Suche nach 'S' gefolgt von Ziffern, optional Space, 'E', Ziffern.
    let lower = t.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b's' && i + 1 < bytes.len() && bytes[i + 1].is_ascii_digit() {
            // Staffelziffern lesen.
            let s_start = i + 1;
            let mut j = s_start;
            while j < bytes.len() && bytes[j].is_ascii_digit() { j += 1; }
            let season: i64 = lower[s_start..j].parse().ok()?;
            // optionale Trenner überspringen.
            let mut k = j;
            while k < bytes.len() && (bytes[k] == b' ' || bytes[k] == b'-' || bytes[k] == b'.') { k += 1; }
            if k < bytes.len() && bytes[k] == b'e' && k + 1 < bytes.len() && bytes[k + 1].is_ascii_digit() {
                let e_start = k + 1;
                let mut m = e_start;
                while m < bytes.len() && bytes[m].is_ascii_digit() { m += 1; }
                let episode: i64 = lower[e_start..m].parse().ok()?;
                let name = t[..i].trim().trim_end_matches(['-', '.', ' ']).trim().to_string();
                let name = if name.is_empty() { t.to_string() } else { name };
                return Some((name, season, episode));
            }
        }
        i += 1;
    }
    None
}

fn try_nxn(t: &str) -> Option<(String, i64, i64)> {
    // Muster "<name> <s>x<e>".
    let bytes = t.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'x' || bytes[i] == b'X' {
            // links Ziffern?
            let mut l = i;
            while l > 0 && bytes[l - 1].is_ascii_digit() { l -= 1; }
            // rechts Ziffern?
            let mut r = i + 1;
            while r < bytes.len() && bytes[r].is_ascii_digit() { r += 1; }
            if l < i && r > i + 1 {
                let season: i64 = t[l..i].parse().ok()?;
                let episode: i64 = t[i + 1..r].parse().ok()?;
                let name = t[..l].trim().trim_end_matches(['-', '.', ' ']).trim().to_string();
                let name = if name.is_empty() { t.to_string() } else { name };
                return Some((name, season, episode));
            }
        }
        i += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn klassifizierung() {
        assert_eq!(classify("http://s/live/u/p/1.ts"), StreamKind::Live);
        assert_eq!(classify("http://s/movie/u/p/1.mkv"), StreamKind::Movie);
        assert_eq!(classify("http://s/series/u/p/1.mp4"), StreamKind::Series);
        assert_eq!(classify("http://s/u/p/1.ts"), StreamKind::Live); // klassisch
        assert_eq!(classify("http://s/vod/u/p/1.mkv"), StreamKind::Movie);
    }

    #[test]
    fn serientitel_zerlegen() {
        assert_eq!(parse_series_title("Breaking Bad S01 E05"), ("Breaking Bad".into(), Some(1), Some(5)));
        assert_eq!(parse_series_title("Breaking Bad S02E10"), ("Breaking Bad".into(), Some(2), Some(10)));
        assert_eq!(parse_series_title("Dark 1x05"), ("Dark".into(), Some(1), Some(5)));
        assert_eq!(parse_series_title("Irgendein Film"), ("Irgendein Film".into(), None, None));
    }
}
