//! Zentraler HTTP-Client: Pooling, Keep-Alive, Kompression, Redirects,
//! Timeouts. TLS über rustls; Zertifikatsfehler werden NICHT ignoriert.

use std::time::Duration;

/// Standard-User-Agent für Playlist-/EPG-Downloads.
///
/// Viele Xtream-Codes-Panels weisen unbekannte User-Agents ab und liefern
/// dann eine leere oder Fehlerseite statt der Playlist. Ein VLC-typischer
/// User-Agent wird von praktisch allen Panels akzeptiert und ist der
/// De-facto-Standard für IPTV-Clients.
pub const DEFAULT_USER_AGENT: &str = "VLC/3.0.20 LibVLC/3.0.20";

pub fn build_client() -> Result<reqwest::Client, reqwest::Error> {
    reqwest::Client::builder()
        .user_agent(DEFAULT_USER_AGENT)
        .connect_timeout(Duration::from_secs(15))
        // Sehr große Playlisten (mehrere hundert MB, langsame IPTV-Server)
        // brauchen deutlich mehr Zeit. 10 Minuten Gesamt-Timeout.
        .timeout(Duration::from_secs(600))
        // Kein Read-Timeout, das einzelne langsame Chunks killt – der
        // Gesamt-Timeout begrenzt bereits.
        .redirect(reqwest::redirect::Policy::limited(10))
        .pool_max_idle_per_host(4)
        .tcp_keepalive(Duration::from_secs(30))
        .cookie_store(true)
        .build()
}

/// Download mit Wiederholungsversuchen und exponentiellem Backoff.
pub async fn get_with_retry(
    client: &reqwest::Client,
    url: &str,
    user_agent: Option<&str>,
    max_retries: u32,
) -> Result<Vec<u8>, String> {
    let mut delay = Duration::from_millis(500);
    let mut last_err = String::new();
    for attempt in 0..=max_retries {
        let mut req = client.get(url);
        if let Some(ua) = user_agent {
            req = req.header(reqwest::header::USER_AGENT, ua);
        }
        match req.send().await {
            Ok(resp) => match resp.error_for_status() {
                Ok(mut ok) => {
                    // Große Playlisten (mehrere hundert MB) chunk-weise lesen,
                    // statt alles auf einmal zu dekodieren. Das vermeidet
                    // "error decoding response body" bei riesigen Antworten.
                    let mut buf: Vec<u8> = Vec::new();
                    let mut stream_err: Option<String> = None;
                    loop {
                        match ok.chunk().await {
                            Ok(Some(chunk)) => buf.extend_from_slice(&chunk),
                            Ok(None) => break, // fertig
                            Err(e) => {
                                stream_err = Some(format!("Übertragung abgebrochen: {e}"));
                                break;
                            }
                        }
                    }
                    match stream_err {
                        None if !buf.is_empty() => return Ok(buf),
                        None => last_err = "Der Server lieferte eine leere Antwort.".into(),
                        Some(e) => last_err = e,
                    }
                }
                Err(e) => {
                    last_err = format!(
                        "Server antwortete mit Status {}",
                        e.status().map(|s| s.as_u16()).unwrap_or(0)
                    );
                    // 4xx nicht wiederholen – das ändert sich nicht von allein.
                    if e.status().map(|s| s.is_client_error()).unwrap_or(false) {
                        break;
                    }
                }
            },
            Err(e) if e.is_timeout() => last_err = "Zeitüberschreitung".into(),
            Err(e) if e.is_connect() => last_err = "Verbindung fehlgeschlagen (DNS/Netzwerk)".into(),
            Err(e) => last_err = exiptv_core::util::sanitize::sanitize_text(&e.to_string()),
        }
        if attempt < max_retries {
            tokio::time::sleep(delay).await;
            delay = (delay * 2).min(Duration::from_secs(8));
        }
    }
    Err(last_err)
}
