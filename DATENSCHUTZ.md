# Datenschutzhinweise – EXIPTV

EXIPTV arbeitet vollständig lokal auf deinem Gerät.

- **Keine Telemetrie.** EXIPTV sendet keine Nutzungsdaten, Absturzberichte
  oder Kennungen an die Entwickler oder Dritte. Eine optionale, ausdrücklich
  zustimmungspflichtige Diagnoseübermittlung ist vorgesehen, aber
  standardmäßig deaktiviert und derzeit nicht implementiert.
- **Zugangsdaten.** Passwörter von Anbietern werden ausschließlich im
  Windows-Anmeldeinformationsspeicher (Credential Manager) abgelegt –
  niemals in der Datenbank, in Konfigurationsdateien oder Logs.
- **Protokolle.** Logdateien liegen lokal unter `%APPDATA%\EXIPTV\logs`,
  rotieren täglich, werden nach 14 Tagen gelöscht und maskieren
  Zugangsdaten in URLs automatisch.
- **Netzwerkzugriffe.** EXIPTV verbindet sich ausschließlich mit den von dir
  eingetragenen Quellen (Playlisten, EPG, Streams, Logos/Poster).
- **Löschen.** Beim Entfernen eines Anbieters werden zugehörige Daten aus der
  Datenbank sowie das zugehörige Geheimnis aus dem Credential Manager entfernt.

EXIPTV ist ausschließlich für die Wiedergabe legal bereitgestellter Streams
und eigener bzw. autorisierter IPTV-Zugänge vorgesehen. Die Software enthält
keine Funktionen zur Umgehung von DRM, Bezahlschranken, Geo-Sperren oder
sonstigen Zugangs- und Schutzmechanismen.
