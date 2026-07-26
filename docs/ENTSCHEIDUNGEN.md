# Architektur-Entscheidungen (ADRs)

## ADR-001: Zwei-Crate-Schnitt (core / src-tauri)
**Entscheidung:** `exiptv-core` ist ein eigenständiges, Tauri-freies Crate;
`src-tauri` ist bewusst KEIN Workspace-Mitglied (eigener Lockfile).
**Grund:** Parser-, DB- und Sicherheitstests laufen so auf jedem Runner ohne
WebView2/GTK-Toolchain (CI-Job „core-tests" auf ubuntu-latest in Sekunden);
zudem entkoppelt es MSRV-Anforderungen (Tauri 2 verlangt Rust ≥ 1.77).

## ADR-002: libmpv statt libVLC als primäre Engine
**Entscheidung:** Phase 4 implementiert `PlaybackEngine` mit libmpv.
**Grund:** libmpv bietet die robusteste HLS-/MPEG-TS-Behandlung, natives
D3D11VA, feingranulare Property-Beobachtung (Grundlage des
StreamHealthMonitor) und saubere Einbettung über `--wid`. Der Trait hält
eine spätere libVLC-Alternative offen.

## ADR-003: Windows-native Typografie
**Entscheidung:** „Segoe UI Variable" als Display-/Textschrift, keine
Web-Fonts. **Grund:** DPI-scharf auf dem Zielsystem, null Netzwerkabruf,
CSP ohne `font-src`-Ausnahmen; Markenwirkung entsteht über Farbwelt und
das EX-Ring-Signaturmotiv, nicht über exotische Schriften.

## ADR-004: Suche über normalisierte Spalte statt FTS5
**Entscheidung:** `channels.search_name` (Kleinschreibung, Diakritika-
Faltung, Trennzeichen-Normalisierung) mit Index + LIKE.
**Grund:** Für Kanalnamen (kurze Strings, Teilwortsuche) ausreichend schnell
bis weit über 10.000 Einträge und ohne FTS5-Tokenizer-Sonderfälle. FTS5
bleibt Option für EPG-Volltext (Phase 6).

## ADR-005: Fortschritt über Tauri-Events
**Entscheidung:** Import meldet Stufen (`laden/verarbeiten/speichern/fertig`)
als `import-progress`-Event statt Polling. **Grund:** ereignisbasierte
Kommunikation (Anforderung Abschnitt 4), kein Timer-Overhead, UI bleibt
reaktiv auch bei sehr großen Listen.

## ADR-006: Browser-Mock im Frontend
**Entscheidung:** `lib/backend.ts` erkennt die Tauri-Runtime; im reinen
Browser springt ein In-Memory-Mock ein. **Grund:** UI-Entwicklung und
-Review ohne native Toolchain; der Mock ist klar gekennzeichnet und
persistiert nichts — kein „falsches" Verhalten in der echten App.

## ADR-007: Konservative Crate-Pins im Core
**Entscheidung:** rusqlite 0.31 (bundled), thiserror 1.x, tempfile =3.10.1.
**Grund:** Der Core bleibt mit Rust 1.75 (Ubuntu-LTS-Paket) baubar —
relevant für schlanke CI-Umgebungen; auf Windows/stable identisch lauffähig.
