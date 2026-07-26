# Entwicklungsphasen – Stand und Plan

## ✅ Phase 1 – Fundament (abgeschlossen)
- Projektstruktur (core / src-tauri / src / docs)
- Tauri-2-Konfiguration inkl. CSP, Fenster, Bundle (MSI + NSIS, DE/EN)
- Logo integriert (`public/assets/branding/`), App-Icons aus dem Logo generiert
- Designsystem: Tokens (`src/styles/tokens.css`), EX-Ring-Signaturmotiv,
  Skeleton-Loading, Fokuszustände, reduzierte Animationen (prefers-reduced-motion)
- Navigation (einklappbare Seitenleiste, Zustand persistiert), Routing, i18n DE/EN
- CI: Core-Tests + Windows-Installer-Build

## ✅ Phase 2 – Datenfundament (abgeschlossen)
- SQLite mit WAL, Foreign Keys, busy_timeout
- Migration v1: alle 23 Entitäten aus Abschnitt 26 inkl. Indizes und Löschregeln
- Einstellungsservice (Upsert, im UI: Sprache, Seitenleisten-Zustand)
- Provider-Datenmodell; Zugangsdaten über Windows Credential Manager
  (`secrets.rs`), DB hält nur `secret_ref`

## ✅ Phase 3 – M3U-Import (Kern abgeschlossen)
- Toleranter Parser: EXTINF-Attribute (tvg-id/-name/-logo/-chno, group-title,
  radio, catchup[-days/-source], timeshift, provider, audio-track,
  user-agent, referrer), #EXTGRP, #EXTVLCOPT, Kommentar-Direktiven
- Encoding: UTF-8 (±BOM), UTF-16 LE/BE, Windows-1252-Fallback
- Relative-URL-Auflösung gegen Playlist-Basis
- Import über URL (Retry/Backoff) und Datei (nativer Dateidialog)
- Staging-Verfahren: Altbestand bleibt bei jedem Fehlschlag erhalten
- Fortschritts-Events, Importprotokoll (`import_jobs`), Fehler-Warnliste
- UI: Anbieterverwaltung, Live-TV mit Gruppen + virtualisierter Liste,
  inkrementelles Nachladen, Logo-Fallback
- Offen in Phase 3 (bewusst): Duplikaterkennung über Anbieter hinweg,
  Kanal-Verstecken/-Sperren im UI (Datenmodell vorhanden)

## ✅ Phase 4 – Wiedergabe (abgeschlossen)
- libmpv-Integration über natives Child-Fenster (HWND als `wid`),
  Video hardwarebeschleunigt außerhalb der WebView
- `MpvEngine` implementiert das `PlaybackEngine`-Trait vollständig
  (Laden, Play/Pause, Seek, Lautstärke, Audio-/Untertitel-/Videospur,
  Seitenverhältnis, Deinterlace, Statistiken, Reconnect)
- Hardwaredecoding `hwdec=auto-safe` mit automatischem Software-Fallback;
  Video-Output `gpu` (API-Wahl durch mpv)
- `StreamHealthMonitor` mit gestufter Wiederherstellung: intern abwarten →
  URL neu laden (2×) → nutzerverständliche Meldung; kein störendes Fehlerfenster
- Player-UI: transparente Videofläche mit Bounds-Reporting, Lade-Overlay
  (EX-Ring), Steuerleiste (Play/Pause, Lautstärke), Tastatursteuerung
  (Leertaste, Pfeile, M, Esc, Bild auf/ab), nutzerverständliche Fehlermeldungen
- libmpv-DLL wird **im Installer gebündelt** (offline-fähiger erster Start,
  maximale Stabilität); Laufzeit-Download nur als Fallback. Die benötigte
  Import-Bibliothek `mpv.lib` wird im CI aus dem offiziellen mpv-Dev-Archiv
  erzeugt (drei Wege: `.def` → `libmpv.dll.a` → `dumpbin`-Rekonstruktion).
- **Lizenzhinweis:** libmpv steht unter LGPL/GPL; die DLL wird dynamisch
  gebunden (kein statisches Linken), siehe `DRITTANBIETER.md`.

## ✅ Zusatzfunktionen (nach Wunschliste, umgesetzt)
- **Einstellungen** ausgebaut: Netzwerkpuffer (klein/normal/groß), Bildqualität
  (auto/hoch/mittel/niedrig), Hardwarebeschleunigung, automatische
  Wiederverbindung, Deinterlacing, bevorzugte Ton-/Untertitelsprache,
  **Hintergrundfarbe** (wirkt sofort auf Design-Token + Logo),
  reduzierte Animationen. Player-Werte werden an mpv durchgereicht.
- **Mehrere Listen** mit **Ein/Aus-Schalter** je Liste und **Umbenennen**
  (inline); Kanalzahl je Liste; deaktivierte Listen werden in VOD/Serien
  ausgeblendet.
- **VOD-Player**: Fortschrittsleiste mit Zeitanzeige, **15-Sekunden-Sprünge**,
  **Minuten-Spulen** (±5 min), **Format-Umschalter** (auto/16:9/4:3/21:9/1:1),
  **Ton-/Untertitelspur-Auswahl** (Menü, sofern angeboten).
- **Gruppen-Sortierung**: Standard, A–Z, Z–A, zuletzt hinzugefügt, Kanalnummer.
- **Verstellbare Spaltenbreite** (Gruppen ↔ Kanäle), Breite wird persistiert.
- **Filme/Serien** mit Poster-Grid (Lazy-Loading, Fehler-Fallback),
  Detailansicht mit Metadaten; **Serien mit Staffel-Tabs und Episodenliste**;
  Klick auf Poster/Episode startet die Wiedergabe.
- **Logo** freigestellt (transparenter Hintergrund) — fügt sich ohne Ecken in
  jede Hintergrundfarbe ein.
- **Kanalwechsler oben links** im Live-TV-Player (▲/▼, auch Bild auf/ab).
- Hinweis: Filme/Serien-Listen sind erst nach einem Xtream-Import (Phase 5)
  gefüllt; M3U-Playlisten liefern keine VOD-Metadaten.

## 🔜 Phase 5 – Xtream-Codes
player_api-Client (get_live_streams, get_vod_streams, get_series,
Kategorien, Kontostatus/Ablauf/max_connections), Provider-Statusanzeige.
**Füllt die bereits fertige Filme/Serien-Oberfläche mit echten Daten.**

## 🔜 Phase 6 – EPG
Streaming-XMLTV-Parser (quick-xml, auch .gz), tvg-id-Auto-Mapping +
manuelle Zuordnung, Zeitversatz je Quelle/Kanal, virtualisierte
Zeitleisten-Ansicht, Jetzt/Danach, Erinnerungen-Datenpfad.

## 🔜 Phase 7 – VOD/Serien-UI + Poster-Cache
LRU-Bildcache auf Datenträger (Tabelle vorhanden), begrenzte
Parallel-Downloads, Platzhalter, Wiedergabefortschritt.

## 🔜 Phase 8 – Profile, Favoriten, Verlauf, globale Suche, Jugendschutz
PIN als Hash (argon2), Sperrlisten, Gastprofil.

## 🔜 Phase 9 – Catch-up, Aufnahme, Timeshift, Multi-View
Nur ungeschützte, anbieterseitig freigegebene Streams; Ringpuffer;
Ressourcenüberwachung für Multi-View.

## 🔜 Phase 10 – Härtung
E2E-Tests (WebDriver), Performance-Szenarien (10k Sender / 100k EPG),
Update-System (tauri-plugin-updater, signierte Pakete), Benutzerhandbuch-Ausbau.

## ✅ Phase 5 – Xtream-Codes (abgeschlossen)
- Vollständiger Xtream-Client (`core/parser/xtream.rs`, headless getestet):
  URL-Normalisierung (entfernt versehentlich mitkopierte `get.php`/`player_api.php`-
  Pfade, ergänzt fehlendes Schema), URL-Kodierung für Benutzer/Passwort,
  tolerante JSON-Deserialisierung (Zahlen mal als Zahl, mal als String).
- API-Endpunkte: Auth/Kontostatus, Live-Kategorien+Streams, VOD-Kategorien+
  Streams, Serien-Kategorien+Liste, Serien-Detail (Staffeln/Episoden).
- Import (`src-tauri/xtream_import.rs`): prüft zuerst die Authentifizierung und
  liefert bei 401/403/abgelaufen eine **klare** Meldung
  („Zugangsdaten wurden vom Anbieter abgelehnt"), lädt dann Live-TV, Filme und
  Serien und speichert sie über die Staging-Transaktionen.
- Live-Kanäle werden auf `M3uEntry` abgebildet und über den getesteten
  Kanal-Speicherpfad gesichert; Stream-URLs korrekt als
  `…/live/USER/PASS/{id}.ts`, Filme `…/movie/…`, Episoden `…/series/…`.
- Serien-Staffeln/Episoden werden **lazy** beim Öffnen der Serie nachgeladen
  (statt tausender Detail-Anfragen beim Import).
- **Root-Cause-Fix:** Zuvor rief „Aktualisieren" bei einem Xtream-Zugang den
  M3U-URL-Import mit der nackten Serveradresse auf → Server antwortete 401.
  Jetzt wird die vollständige Xtream-URL mit Zugangsdaten korrekt gebaut.
- HTTP-Standard-User-Agent auf `VLC/3.0.20` geändert (viele Panels weisen
  unbekannte User-Agents ab); aussagekräftige Import-Diagnose bei 0 Sendern
  (leer / HTML-Fehlerseite / keine EXTINF).

## Ergänzung – M3U-Fallback für Xtream-Zugänge
Manche Xtream-Panels sperren `player_api.php` (Antwort 401), liefern aber die
`get.php`-M3U-Playlist. Für diesen Fall importiert EXIPTV automatisch über den
M3U-Weg und **teilt die Playlist in Live-TV, Filme und Serien auf**
(`core/parser/m3u_classify.rs` erkennt den Typ am URL-Pfad `/live/`, `/movie/`,
`/series/` und zerlegt Serien-Episodentitel in Name/Staffel/Episode;
`src-tauri/m3u_split.rs` gruppiert und speichert getrennt). So funktionieren
Filme/Serien auch ohne nutzbare Xtream-API.
