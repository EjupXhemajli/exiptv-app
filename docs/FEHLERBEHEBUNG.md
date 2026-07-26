# Fehlerbehebung

## „Die Playlist konnte nicht geladen werden"
- URL im Browser prüfen (liefert sie eine Textdatei, die mit `#EXTM3U`
  beginnt?).
- Bei „Server antwortete mit Status 401/403": Zugangsdaten bzw. in der URL
  enthaltene Tokens prüfen.
- Bei „Zeitüberschreitung"/„Verbindung fehlgeschlagen (DNS/Netzwerk)":
  Internetverbindung, VPN/Firewall prüfen. EXIPTV wiederholt Downloads
  automatisch mit steigenden Wartezeiten.
- Wichtig: Ein fehlgeschlagener Import löscht nie die bestehende Senderliste.

## „Die Datei konnte nicht gelesen werden"
Pfad existiert? Datei nicht durch ein anderes Programm gesperrt?
Leserechte vorhanden?

## Import meldet übersprungene Einträge
Normal bei unsauberen Playlisten: Zeilen ohne gültige Stream-URL werden
ausgelassen. Details stehen im Importprotokoll (Datenbanktabelle
`import_jobs`) und im Log.

## Sonderzeichen werden falsch angezeigt
EXIPTV erkennt UTF-8 (auch mit BOM), UTF-16 und fällt auf Windows-1252
zurück. Zeigt der Import „Windows-1252" an, obwohl die Datei UTF-8 sein
sollte, ist die Quelldatei beschädigt gespeichert — Datei neu in UTF-8
exportieren.

## Logs für eine Fehlermeldung finden
`%APPDATA%\app.exiptv.desktop\logs\` — Zugangsdaten sind darin automatisch
maskiert. Logs rotieren täglich und werden nach 14 Tagen entfernt.

## App startet nicht
- WebView2-Runtime installiert? (Auf Windows 10/11 üblicherweise vorhanden;
  sonst von Microsoft „Evergreen Bootstrapper" installieren.)
- Datenbank testweise umbenennen (`exiptv.db` → `exiptv.db.bak`):
  startet EXIPTV dann, war die DB beschädigt; bitte Log beilegen.
