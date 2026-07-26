//! Maskierung sensibler Daten vor jeder Protokollierung oder Anzeige.
//!
//! Regeln:
//! - `user:pass@host` → `user:***@host`
//! - Query-Parameter `password`, `pass`, `pwd`, `token`, `api_key`, `apikey`,
//!   `auth`, `secret` → Wert wird durch `***` ersetzt
//! - Xtream-Pfadmuster `/live/<user>/<pass>/<id>` → Passwortsegment maskiert
//!
//! Diese Funktionen werden von Logging, Diagnosebericht und Fehlermeldungen
//! verwendet. Rohdaten mit Zugangsdaten dürfen den Core nie verlassen.

const SENSITIVE_QUERY_KEYS: &[&str] = &[
    "password", "pass", "pwd", "token", "api_key", "apikey", "auth", "secret",
];

/// Maskiert Zugangsdaten in einer URL. Arbeitet rein textbasiert und
/// verändert die URL-Struktur nicht (Logs bleiben nachvollziehbar).
pub fn sanitize_url(url: &str) -> String {
    let mut out = mask_userinfo(url);
    out = mask_query(&out);
    out = mask_xtream_path(&out);
    out
}

/// Maskiert Zugangsdaten in freiem Text (z. B. Fehlermeldungen von Servern),
/// indem alle URL-artigen Teilstrings behandelt werden.
pub fn sanitize_text(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(pos) = rest.find("http") {
        let (before, from) = rest.split_at(pos);
        result.push_str(before);
        let end = from
            .find(|c: char| c.is_whitespace() || c == '"' || c == '\'' || c == '>')
            .unwrap_or(from.len());
        let (url, tail) = from.split_at(end);
        result.push_str(&sanitize_url(url));
        rest = tail;
    }
    result.push_str(rest);
    result
}

fn mask_userinfo(url: &str) -> String {
    let Some(scheme_end) = url.find("://") else { return url.to_owned() };
    let after = &url[scheme_end + 3..];
    let authority_end = after.find('/').unwrap_or(after.len());
    let authority = &after[..authority_end];
    if let Some(at) = authority.rfind('@') {
        let userinfo = &authority[..at];
        let masked = match userinfo.find(':') {
            Some(c) => format!("{}:***", &userinfo[..c]),
            None => "***".to_owned(),
        };
        format!(
            "{}://{}@{}",
            &url[..scheme_end],
            masked,
            &after[at + 1..]
        )
    } else {
        url.to_owned()
    }
}

fn mask_query(url: &str) -> String {
    let Some(qpos) = url.find('?') else { return url.to_owned() };
    let (base, query) = url.split_at(qpos);
    let query = &query[1..];
    let masked: Vec<String> = query
        .split('&')
        .map(|pair| {
            let mut it = pair.splitn(2, '=');
            let key = it.next().unwrap_or("");
            match it.next() {
                Some(_) if SENSITIVE_QUERY_KEYS.contains(&key.to_ascii_lowercase().as_str()) => {
                    format!("{key}=***")
                }
                Some(v) => format!("{key}={v}"),
                None => key.to_owned(),
            }
        })
        .collect();
    format!("{}?{}", base, masked.join("&"))
}

/// Xtream-Streams haben die Form
/// `http://host:port/{live|movie|series}/USER/PASS/STREAMID.ext`.
/// Das dritte Segment nach dem Typ ist das Passwort.
fn mask_xtream_path(url: &str) -> String {
    for kind in ["/live/", "/movie/", "/series/"] {
        if let Some(pos) = url.find(kind) {
            let head_end = pos + kind.len();
            let tail = &url[head_end..];
            let segs: Vec<&str> = tail.split('/').collect();
            if segs.len() >= 3 {
                let mut masked = segs.to_vec();
                masked[1] = "***";
                return format!("{}{}", &url[..head_end], masked.join("/"));
            }
        }
    }
    url.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maskiert_userinfo() {
        assert_eq!(
            sanitize_url("http://max:geheim@srv.tld/x.m3u8"),
            "http://max:***@srv.tld/x.m3u8"
        );
    }

    #[test]
    fn maskiert_query_parameter() {
        let s = sanitize_url("https://srv.tld/get.php?username=max&password=geheim&type=m3u");
        assert!(s.contains("username=max"));
        assert!(s.contains("password=***"));
        assert!(!s.contains("geheim"));
    }

    #[test]
    fn maskiert_xtream_pfad() {
        assert_eq!(
            sanitize_url("http://srv.tld:8080/live/max/geheim/123.ts"),
            "http://srv.tld:8080/live/max/***/123.ts"
        );
    }

    #[test]
    fn maskiert_urls_in_freitext() {
        let t = sanitize_text("Fehler bei http://a:b@x.tld/1 und https://y.tld/get.php?password=p");
        assert!(!t.contains(":b@"));
        assert!(!t.contains("password=p"));
    }

    #[test]
    fn laesst_unkritische_urls_unveraendert() {
        let u = "https://example.org/logo.png?v=2";
        assert_eq!(sanitize_url(u), u);
    }
}
