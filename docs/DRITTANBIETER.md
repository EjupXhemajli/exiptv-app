# Drittanbieter-Komponenten

## libmpv (mpv media player)
EXIPTV nutzt zur Videowiedergabe **libmpv**, die Bibliothek des
mpv-Media-Players (https://mpv.io). libmpv steht unter der GNU Lesser
General Public License (LGPL) bzw. GPL, je nach Build.

Wichtig für die Verteilung:
- Die Bibliothek `libmpv-2.dll` wird **mit dem EXIPTV-Installer gebündelt**
  (offline-fähiger erster Start, maximale Stabilität). Sie stammt aus dem
  offiziellen mpv-Windows-Dev-Archiv (zhongfly/mpv-winbuild, Spiegel von
  shinchiro/mpv-winbuild-cmake) und wird im CI zur Build-Zeit eingebunden.
  Als Fallback (falls die gebündelte Datei fehlt) lädt EXIPTV die DLL beim
  ersten Start automatisch von derselben offiziellen Quelle nach.
- EXIPTV bindet libmpv **ausschließlich dynamisch** über deren öffentliche
  C-API ein (kein statisches Linken). Der EXIPTV-Quellcode enthält keinen
  mpv-/FFmpeg-Code. Damit sind die Bedingungen der LGPL erfüllt: Die
  dynamisch gebundene Bibliothek kann durch den Nutzer ausgetauscht werden.
- Der mpv-Quellcode ist unter https://github.com/mpv-player/mpv verfügbar.

## Rust-Bibliotheken (Auswahl)
- tauri (MIT/Apache-2.0) – Anwendungsrahmen
- libmpv2 (MIT) – sichere Rust-Bindung an libmpv
- sevenz-rust (Apache-2.0) – Entpacken des mpv-Archivs
- rusqlite (MIT) – SQLite-Anbindung
- reqwest (MIT/Apache-2.0) – HTTP-Client
- keyring (MIT/Apache-2.0) – Windows-Anmeldeinformationsspeicher
- windows-sys (MIT/Apache-2.0) – Windows-API

Die vollständige Abhängigkeitsliste inkl. Lizenzen lässt sich mit
`cargo tree` bzw. `cargo license` erzeugen.
