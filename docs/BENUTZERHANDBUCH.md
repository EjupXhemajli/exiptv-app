# EXIPTV – Benutzeranleitung

## Erste Schritte
1. EXIPTV starten. Beim ersten Start ist die Startseite leer und bietet
   „Anbieter hinzufügen" an.
2. Im Bereich **Anbieter** auf „Anbieter hinzufügen" klicken.
3. Typ wählen:
   - **M3U-Playlist (URL):** Adresse der Playlist eintragen.
   - **M3U-Playlist (Datei):** über „Datei wählen" eine .m3u/.m3u8 öffnen.
   - **Xtream-Zugang:** Server, Benutzername und Passwort eintragen.
     Das Passwort wird sicher im Windows-Anmeldeinformationsspeicher abgelegt.
4. Speichern — der Import startet automatisch und zeigt den Fortschritt
   (Laden → Verarbeiten → Speichern). Fehlerhafte Einträge werden
   übersprungen und gezählt, der Import bricht dadurch nicht ab.
5. Unter **Live-TV** erscheinen die Sender, links die Gruppen.

## Aktualisieren
Im Bereich Anbieter „Aktualisieren" wählen. Schlägt die Aktualisierung fehl,
bleibt die zuletzt funktionierende Senderliste vollständig erhalten.

## Suche
Bereich **Suche** (oder Strg+F, ab Phase 4 global): ab zwei Zeichen wird
während der Eingabe gesucht; Umlaute und Sonderzeichen werden tolerant
behandelt („Munchen" findet „München TV").

## Sprache
**Einstellungen → Allgemein → Sprache**: Deutsch oder Englisch, wird sofort
übernommen und gespeichert.

## Wiedergabe
Auf einen Sender klicken (oder mit den Pfeiltasten auswählen und Enter
drücken) startet die Wiedergabe im Vollbild-Overlay.

**Beim allerersten Start** lädt EXIPTV einmalig die Wiedergabe-Komponente
(libmpv, ca. 40 MB) automatisch herunter – das dauert je nach Verbindung
wenige Sekunden bis eine Minute und ist danach dauerhaft vorhanden. Der
Fortschritt wird im Ladebereich angezeigt.

Steuerung während der Wiedergabe:
- **Leertaste:** Pause / Weiter
- **Pfeil hoch/runter:** Lautstärke
- **M:** Stummschalten
- **Esc:** Wiedergabe schließen
- Die Steuerleiste blendet sich bei Inaktivität aus und erscheint bei
  Mausbewegung wieder.

Bei einer Stream-Störung versucht EXIPTV zunächst selbstständig, die
Verbindung wiederherzustellen (dezenter Hinweis „Verbindung wird
wiederhergestellt …"). Erst wenn das mehrfach nicht gelingt, erscheint eine
verständliche Meldung mit der Möglichkeit, es erneut zu versuchen.

Die Hardwarebeschleunigung wird automatisch genutzt und fällt bei
Inkompatibilität selbsttätig auf Software-Decoding zurück.
