# EXIPTV – Architektur

## Überblick

```
┌─────────────────────────────────────────────────────────────┐
│ Frontend (React + TypeScript, WebView2)                     │
│  Seiten · Design-Tokens · i18n · virtualisierte Listen      │
│  lib/backend.ts  ←→  Tauri-invoke / Events                  │
└───────────────▲─────────────────────────────────────────────┘
                │ IPC (JSON, typisiert)
┌───────────────┴─────────────────────────────────────────────┐
│ src-tauri (Tauri-Shell, Rust)                               │
│  commands.rs   dünne IPC-Schicht, nutzerverständliche Fehler│
│  http.rs       reqwest-Client: Pooling, Retry, Backoff, TLS │
│  secrets.rs    Windows Credential Manager (keyring)         │
│  state.rs      DB-Handle, HTTP-Client                       │
│  lib.rs        Logging (rotierend, maskiert), Setup         │
└───────────────▲─────────────────────────────────────────────┘
                │ Rust-API
┌───────────────┴─────────────────────────────────────────────┐
│ exiptv-core (headless, ohne Tauri-Abhängigkeit)             │
│  parser/m3u    toleranter Extended-M3U-Parser               │
│  db            SQLite (WAL), Migrationen, Staging-Import,   │
│                Pagination, normalisierte Suche              │
│  models        23 Entitäten (Abschnitt 26)                  │
│  playback      PlaybackEngine-Trait (Impl. in Phase 4)      │
│  util          Encoding-Erkennung, Credential-Maskierung    │
└─────────────────────────────────────────────────────────────┘
```

## Threading-Modell

- Alle IPC-Commands sind `async`; CPU-/IO-lastige Arbeit (Parsen,
  Massen-Insert) läuft über `spawn_blocking`. Die UI blockiert nie.
- SQLite läuft im WAL-Modus mit `busy_timeout`; Schreibpfade sind
  transaktional. Ein `Mutex<Database>` serialisiert Schreibzugriffe —
  bewusst einfach; bei Bedarf (EPG-Massenimport in Phase 6) wird auf einen
  dedizierten DB-Worker-Thread mit Kanal umgestellt.

## Datenfluss Playlist-Import (Staging, Abschnitt 13)

1. Download (Retry + exponentielles Backoff, 4xx bricht sofort ab)
2. Encoding-Erkennung (BOM → UTF-8 → Windows-1252-Fallback)
3. Tolerantes Parsen (fehlerhafte Zeilen → Warnungsliste, kein Abbruch)
4. Validierung (leeres Ergebnis ⇒ Abbruch, Altbestand bleibt)
5. Eine Transaktion: Löschen + Neuaufbau von Gruppen/Kanälen,
   `import_jobs`-Protokoll, `last_refresh_at`
6. Commit — erst jetzt ist der Altbestand ersetzt
7. Fortschritts-Events (`import-progress`) an das Frontend in jeder Stufe

## Playback (Phase 4, geplant)

`exiptv_core::playback::PlaybackEngine` ist der Vertrag. Die
libmpv-Implementierung (Fensterintegration über HWND/`--wid`,
D3D11VA-Hardwaredecoding mit Software-Fallback) und der
`StreamHealthMonitor` (Frame-/Bitraten-/HTTP-Überwachung mit gestufter
Wiederherstellung) entstehen in der Tauri-Shell, da sie native Handles
benötigen. Die UI programmiert ausschließlich gegen den Trait.

## Sicherheit

- Passwörter: nur im Credential Manager, DB speichert `secret_ref`.
- Logs/Fehlermeldungen: `util::sanitize` maskiert `user:pass@`,
  sensible Query-Parameter und Xtream-Pfadsegmente.
- SQL: ausschließlich parametrisierte Abfragen.
- CSP in `tauri.conf.json`; keine Remote-Skripte; TLS über rustls,
  Zertifikatsfehler werden nicht unterdrückt.
