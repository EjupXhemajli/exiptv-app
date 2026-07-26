//! Sichere Ablage von Zugangsdaten über den Betriebssystem-Schlüsselbund
//! (Windows: Credential Manager / DPAPI-gestützt).
//!
//! In der Datenbank liegt nur ein Referenzschlüssel (`secret_ref`),
//! niemals das Passwort selbst.

const SERVICE: &str = "EXIPTV";

pub fn store(reference: &str, secret: &str) -> Result<(), String> {
    keyring::Entry::new(SERVICE, reference)
        .and_then(|e| e.set_password(secret))
        .map_err(|e| format!("Zugangsdaten konnten nicht sicher gespeichert werden: {e}"))
}

pub fn load(reference: &str) -> Result<Option<String>, String> {
    match keyring::Entry::new(SERVICE, reference).and_then(|e| e.get_password()) {
        Ok(p) => Ok(Some(p)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(format!("Zugangsdaten konnten nicht gelesen werden: {e}")),
    }
}

pub fn delete(reference: &str) -> Result<(), String> {
    match keyring::Entry::new(SERVICE, reference).and_then(|e| e.delete_credential()) {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(format!("Zugangsdaten konnten nicht entfernt werden: {e}")),
    }
}
