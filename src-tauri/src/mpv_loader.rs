//! Beschafft `libmpv-2.dll` beim ersten Start.
//!
//! Entscheidung (mit dem Nutzer abgestimmt): Die ~40 MB große libmpv-DLL
//! wird NICHT in den Installer gepackt, sondern beim ersten Bedarf in das
//! App-Datenverzeichnis geladen — schlanker Installer.
//!
//! Ablauf:
//! 1. Offizielles mpv-Windows-Dev-Archiv (7z) von der bekannten Quelle laden.
//! 2. Mit reiner-Rust-7z-Bibliothek entpacken und nur `libmpv-2.dll` behalten.
//! 3. Größen-Plausibilitätsprüfung.
//! Windows findet die DLL anschließend über den erweiterten Prozess-PATH.

use std::path::{Path, PathBuf};

const MPV_DLL_NAME: &str = "libmpv-2.dll";

/// Offizielle mpv-Windows-Dev-Builds. Fest verdrahtet und dokumentiert;
/// kein dynamisches Quellen-Nachladen. Die Reihenfolge ist die
/// Fallback-Reihenfolge. GitHub-Release-Assets sind stabiler adressierbar
/// als SourceForge-Redirects.
const MPV_ARCHIVE_URLS: &[&str] = &[
    // zhongfly/mpv-winbuild – feste Release-Assets (libmpv inkl. libmpv-2.dll).
    "https://github.com/zhongfly/mpv-winbuild/releases/download/2026-04-24-4c92626/mpv-dev-x86_64-20260424-git-4c92626.7z",
    "https://github.com/zhongfly/mpv-winbuild/releases/download/2026-03-26-1a545fa/mpv-dev-x86_64-20260326-git-1a545fa.7z",
    // SourceForge-Spiegel als zusätzlicher Fallback.
    "https://sourceforge.net/projects/mpv-player-windows/files/libmpv/mpv-dev-x86_64-20240818-git-e9a53a3.7z/download",
];

const MIN_DLL_BYTES: u64 = 20 * 1024 * 1024;

pub fn runtime_dir(app_data: &Path) -> PathBuf {
    app_data.join("runtime")
}
pub fn dll_path(app_data: &Path) -> PathBuf {
    runtime_dir(app_data).join(MPV_DLL_NAME)
}

pub fn is_available(app_data: &Path) -> bool {
    std::fs::metadata(dll_path(app_data))
        .map(|m| m.is_file() && m.len() >= MIN_DLL_BYTES)
        .unwrap_or(false)
}

/// Ergänzt das Runtime-Verzeichnis vorne im Prozess-PATH (idempotent),
/// damit der dynamische Linker `libmpv-2.dll` findet.
pub fn ensure_on_path(app_data: &Path) {
    let dir = runtime_dir(app_data);
    let dir_str = dir.to_string_lossy().to_string();
    let current = std::env::var("PATH").unwrap_or_default();
    if !current.split(';').any(|p| p == dir_str) {
        let joined = if current.is_empty() { dir_str } else { format!("{dir_str};{current}") };
        std::env::set_var("PATH", joined);
    }
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MpvSetupStatus {
    pub stage: String, // vorhanden|laden|entpacken|fertig|fehler
    pub message: String,
}

/// Lädt und entpackt die DLL synchron (im Aufrufer über spawn_blocking nutzen).
/// `on_status` erhält Fortschrittsmeldungen für das Frontend.
pub fn ensure_dll_blocking(
    app_data: &Path,
    client: &reqwest::blocking::Client,
    mut on_status: impl FnMut(MpvSetupStatus),
) -> Result<PathBuf, String> {
    if is_available(app_data) {
        on_status(MpvSetupStatus { stage: "vorhanden".into(), message: "Wiedergabe-Komponente ist bereits installiert.".into() });
        return Ok(dll_path(app_data));
    }

    let rt = runtime_dir(app_data);
    std::fs::create_dir_all(&rt).map_err(|e| format!("Verzeichnis konnte nicht angelegt werden: {e}"))?;

    on_status(MpvSetupStatus { stage: "laden".into(), message: "Wiedergabe-Komponente wird geladen …".into() });

    // Archiv herunterladen (Fallback-Quellen der Reihe nach).
    let mut archive_bytes: Option<Vec<u8>> = None;
    let mut last_err = String::new();
    for url in MPV_ARCHIVE_URLS {
        match download(client, url) {
            Ok(bytes) if bytes.len() as u64 >= MIN_DLL_BYTES => { archive_bytes = Some(bytes); break; }
            Ok(_) => { last_err = "Heruntergeladene Datei ist unerwartet klein.".into(); }
            Err(e) => { last_err = e; }
        }
    }
    let archive_bytes = archive_bytes.ok_or_else(|| {
        format!("Die Wiedergabe-Komponente konnte nicht geladen werden. {last_err}")
    })?;

    on_status(MpvSetupStatus { stage: "entpacken".into(), message: "Wiedergabe-Komponente wird eingerichtet …".into() });

    // Archiv temporär auf Platte schreiben und mit der stabilen Helper-API
    // in ein Temp-Verzeichnis entpacken; danach nur die DLL übernehmen.
    let tmp_archive = rt.join("mpv-download.7z");
    std::fs::write(&tmp_archive, &archive_bytes)
        .map_err(|e| format!("Zwischenspeichern fehlgeschlagen: {e}"))?;
    let extract_dir = rt.join("extract");
    let _ = std::fs::remove_dir_all(&extract_dir);
    std::fs::create_dir_all(&extract_dir)
        .map_err(|e| format!("Entpackverzeichnis fehlgeschlagen: {e}"))?;

    let extracted_dll = extract_libmpv(&tmp_archive, &extract_dir)
        .map_err(|e| format!("Die Wiedergabe-Komponente konnte nicht entpackt werden: {e}"))?;

    // Prüfen und an den finalen Ort verschieben.
    let meta = std::fs::metadata(&extracted_dll)
        .map_err(|e| format!("Entpackte Datei nicht lesbar: {e}"))?;
    if meta.len() < MIN_DLL_BYTES {
        return Err("Die entpackte Wiedergabe-Komponente ist beschädigt.".into());
    }
    let target = dll_path(app_data);
    // rename kann über Laufwerksgrenzen scheitern -> Fallback auf copy.
    if std::fs::rename(&extracted_dll, &target).is_err() {
        std::fs::copy(&extracted_dll, &target)
            .map_err(|e| format!("Abschluss fehlgeschlagen: {e}"))?;
    }

    // Aufräumen (Fehler hier sind unkritisch).
    let _ = std::fs::remove_file(&tmp_archive);
    let _ = std::fs::remove_dir_all(&extract_dir);

    on_status(MpvSetupStatus { stage: "fertig".into(), message: "Wiedergabe-Komponente ist einsatzbereit.".into() });
    Ok(target)
}

fn download(client: &reqwest::blocking::Client, url: &str) -> Result<Vec<u8>, String> {
    let resp = client.get(url).send().map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("Server antwortete mit Status {}.", resp.status().as_u16()));
    }
    let bytes = resp.bytes().map_err(|e| e.to_string())?;
    Ok(bytes.to_vec())
}

/// Entpackt aus einem 7z-Archiv (Datei) die libmpv-DLL nach `dest` und
/// liefert deren Pfad. Nutzt die stabile Helper-API von sevenz-rust
/// (`decompress_with_extract_fn`, Signatur der Version 0.6).
fn extract_libmpv(archive_path: &Path, dest: &Path) -> Result<PathBuf, String> {
    use sevenz_rust::{default_entry_extract_fn, SevenZArchiveEntry};
    use std::io::Read;
    use std::path::PathBuf as StdPathBuf;
    use std::sync::{Arc, Mutex};

    let file = std::fs::File::open(archive_path).map_err(|e| e.to_string())?;
    let found: Arc<Mutex<Option<StdPathBuf>>> = Arc::new(Mutex::new(None));
    let found_cb = Arc::clone(&found);

    sevenz_rust::decompress_with_extract_fn(
        file,
        dest,
        move |entry: &SevenZArchiveEntry, reader: &mut dyn Read, out_path: &StdPathBuf| {
            let lower = entry.name().to_ascii_lowercase();
            let is_dll = lower.ends_with("libmpv-2.dll") || lower.ends_with("mpv-2.dll");
            if is_dll {
                // Standard-Extraktion schreibt die Datei nach out_path.
                let done = default_entry_extract_fn(entry, reader, out_path)?;
                *found_cb.lock().unwrap() = Some(out_path.clone());
                Ok(done)
            } else {
                // Uninteressante Einträge überspringen (Inhalt verwerfen).
                let mut sink = std::io::sink();
                std::io::copy(reader, &mut sink)
                    .map_err(sevenz_rust::Error::io)?;
                Ok(true)
            }
        },
    )
    .map_err(|e| e.to_string())?;

    let guard = found.lock().unwrap();
    guard.clone().ok_or_else(|| "libmpv-2.dll im Archiv nicht gefunden.".into())
}
