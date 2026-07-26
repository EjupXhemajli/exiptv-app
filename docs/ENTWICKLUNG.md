# Entwicklerdokumentation

## Einrichtung (Windows)
1. Rust (stable, ≥ 1.77) über rustup installieren
2. Node.js ≥ 20 installieren
3. `npm install`
4. `npx tauri dev` — startet Vite + Rust-Backend mit Hot Reload

## Nützliche Befehle
| Zweck | Befehl |
|---|---|
| Core-Tests | `cargo test -p exiptv-core` |
| Frontend-Typprüfung + Bundle | `npm run build` |
| UI im Browser (Mock-Backend) | `npm run dev` |
| Release-Build + Installer | `npx tauri build` |

## Konventionen
- Fehlermeldungen Richtung UI sind nutzerverständlich (Deutsch);
  technische Details gehen maskiert ins Log (`tracing`).
- Keine Zugangsdaten in DB, Logs oder Fehlermeldungen — immer über
  `exiptv_core::util::sanitize` bzw. `secrets.rs`.
- Neue Tabellen/Spalten ausschließlich über eine neue Migration in
  `core/src/db/migrations.rs` (Version erhöhen, niemals v1 ändern).
- IPC-Commands bleiben dünn; Logik gehört in `exiptv-core`, damit sie
  testbar ist.
- UI-Texte ausschließlich über i18n (`src/i18n/*.json`).

## Verzeichnisse zur Laufzeit (Windows)
- Datenbank: `%APPDATA%\app.exiptv.desktop\exiptv.db`
- Logs: `%APPDATA%\app.exiptv.desktop\logs\exiptv.log.YYYY-MM-DD`
- Zugangsdaten: Windows-Anmeldeinformationsspeicher, Dienst „EXIPTV"
