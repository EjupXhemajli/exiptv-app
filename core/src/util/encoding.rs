//! Tolerante Text-Dekodierung für Playlisten und XMLTV-Dateien.
//!
//! Reihenfolge:
//! 1. BOM-Erkennung (UTF-8, UTF-16 LE/BE)
//! 2. UTF-8-Validierung
//! 3. Fallback Windows-1252 (deckt Latin-1-Playlisten inkl. Umlauten ab)
//!
//! Es wird niemals abgebrochen: jede Bytefolge liefert einen String.

use encoding_rs::{UTF_16BE, UTF_16LE, WINDOWS_1252};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectedEncoding {
    Utf8,
    Utf8Bom,
    Utf16Le,
    Utf16Be,
    Windows1252,
}

pub struct DecodedText {
    pub text: String,
    pub encoding: DetectedEncoding,
    /// true, wenn beim Dekodieren Ersatzzeichen nötig waren.
    pub had_errors: bool,
}

pub fn decode_bytes(bytes: &[u8]) -> DecodedText {
    // BOMs
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        let (text, had_errors) = lossy_utf8(&bytes[3..]);
        return DecodedText { text, encoding: DetectedEncoding::Utf8Bom, had_errors };
    }
    if bytes.starts_with(&[0xFF, 0xFE]) {
        let (cow, _, had_errors) = UTF_16LE.decode(&bytes[2..]);
        return DecodedText { text: cow.into_owned(), encoding: DetectedEncoding::Utf16Le, had_errors };
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        let (cow, _, had_errors) = UTF_16BE.decode(&bytes[2..]);
        return DecodedText { text: cow.into_owned(), encoding: DetectedEncoding::Utf16Be, had_errors };
    }
    // Gültiges UTF-8?
    if std::str::from_utf8(bytes).is_ok() {
        let (text, _) = lossy_utf8(bytes);
        return DecodedText { text, encoding: DetectedEncoding::Utf8, had_errors: false };
    }
    // Fallback: Windows-1252 kann jede Bytefolge dekodieren.
    let (cow, _, had_errors) = WINDOWS_1252.decode(bytes);
    DecodedText { text: cow.into_owned(), encoding: DetectedEncoding::Windows1252, had_errors }
}

fn lossy_utf8(bytes: &[u8]) -> (String, bool) {
    match std::str::from_utf8(bytes) {
        Ok(s) => (s.to_owned(), false),
        Err(_) => (String::from_utf8_lossy(bytes).into_owned(), true),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn erkennt_utf8_mit_umlauten() {
        let d = decode_bytes("Öffentlich-Rechtlich".as_bytes());
        assert_eq!(d.encoding, DetectedEncoding::Utf8);
        assert!(d.text.contains('Ö'));
    }

    #[test]
    fn erkennt_utf8_bom() {
        let mut b = vec![0xEF, 0xBB, 0xBF];
        b.extend_from_slice("#EXTM3U".as_bytes());
        let d = decode_bytes(&b);
        assert_eq!(d.encoding, DetectedEncoding::Utf8Bom);
        assert!(d.text.starts_with("#EXTM3U"));
    }

    #[test]
    fn faellt_auf_windows_1252_zurueck() {
        // "München" in Latin-1: ü = 0xFC (ungültiges UTF-8 allein)
        let b = b"M\xFCnchen";
        let d = decode_bytes(b);
        assert_eq!(d.encoding, DetectedEncoding::Windows1252);
        assert_eq!(d.text, "München");
    }

    #[test]
    fn erkennt_utf16_le() {
        let mut b = vec![0xFF, 0xFE];
        for u in "#EXTM3U".encode_utf16() { b.extend_from_slice(&u.to_le_bytes()); }
        let d = decode_bytes(&b);
        assert_eq!(d.encoding, DetectedEncoding::Utf16Le);
        assert_eq!(d.text, "#EXTM3U");
    }
}
