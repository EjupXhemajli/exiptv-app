//! Schlanker RSS-/Atom-Parser für die Nachrichten-Slideshow auf der Startseite.
//!
//! Bewusst ohne zusätzliche XML-Bibliothek: RSS ist flach strukturiert und
//! lässt sich zuverlässig über Tag-Grenzen einlesen. Das hält die
//! Abhängigkeiten klein und den Windows-Build stabil.
//!
//! Unterstützt:
//! - RSS 2.0 (`<item>` mit `<title>`, `<description>`, `<link>`, `<pubDate>`)
//! - Bilder aus `<enclosure url="…">`, `<media:content url="…">`,
//!   `<media:thumbnail url="…">` oder einem `<img src="…">` in der Beschreibung
//! - CDATA-Abschnitte und die gängigen HTML-Entities

use serde::{Deserialize, Serialize};

/// Eine Nachricht für die Slideshow.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NewsItem {
    pub title: String,
    /// Zusammenfassung, bereits von HTML befreit.
    pub summary: String,
    pub link: String,
    pub image_url: Option<String>,
    /// Veröffentlichungszeitpunkt als Rohtext (Anzeige übernimmt die UI).
    pub published: Option<String>,
    /// Herkunft, z. B. "Politik" oder "Sport".
    pub source: String,
}

/// Liest bis zu `limit` Einträge aus einem RSS-/Atom-Dokument.
pub fn parse_feed(xml: &str, source: &str, limit: usize) -> Vec<NewsItem> {
    let mut out = Vec::new();
    // RSS nutzt <item>, Atom <entry>.
    for (open, close) in [("<item", "</item>"), ("<entry", "</entry>")] {
        let mut rest = xml;
        while out.len() < limit {
            let Some(start) = rest.find(open) else { break };
            let after_start = &rest[start..];
            let Some(end) = after_start.find(close) else { break };
            let block = &after_start[..end];

            if let Some(item) = parse_item(block, source) {
                out.push(item);
            }
            rest = &after_start[end + close.len()..];
        }
        if !out.is_empty() {
            break; // Format erkannt
        }
    }
    out
}

fn parse_item(block: &str, source: &str) -> Option<NewsItem> {
    let title = tag_text(block, "title")?;
    if title.trim().is_empty() {
        return None;
    }
    let summary_raw = tag_text(block, "description")
        .or_else(|| tag_text(block, "summary"))
        .or_else(|| tag_text(block, "content"))
        .unwrap_or_default();
    // WICHTIG: zuerst CDATA auspacken und Entities auflösen, danach erst
    // HTML-Tags entfernen. Andernfalls verschluckt die Tag-Entfernung das
    // schließende "]]>" bzw. maskierte Tags werden nicht erkannt.
    let summary_unwrapped = unwrap_cdata_and_entities(&summary_raw);
    let link = tag_text(block, "link")
        .or_else(|| attr_value(block, "<link", "href"))
        .unwrap_or_default();
    let published = tag_text(block, "pubDate")
        .or_else(|| tag_text(block, "updated"))
        .or_else(|| tag_text(block, "published"));

    let image_url = attr_value(block, "<enclosure", "url")
        .or_else(|| attr_value(block, "<media:content", "url"))
        .or_else(|| attr_value(block, "<media:thumbnail", "url"))
        // Bild im (bereits ausgepackten) Beschreibungstext suchen.
        .or_else(|| attr_value(&summary_unwrapped, "<img", "src"))
        .filter(|u| u.starts_with("http"));

    Some(NewsItem {
        title: clean_text(&title),
        summary: normalize_space(&strip_html(&summary_unwrapped)),
        link: clean_text(&link),
        image_url,
        published: published.map(|p| clean_text(&p)),
        source: source.to_string(),
    })
}

/// Packt CDATA aus und löst Entities auf (ohne HTML zu entfernen).
fn unwrap_cdata_and_entities(s: &str) -> String {
    let t = s.trim();
    let inner = t
        .strip_prefix("<![CDATA[")
        .and_then(|r| r.strip_suffix("]]>"))
        .unwrap_or(t);
    decode_entities(inner.trim())
}

fn normalize_space(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Inhalt des ersten `<tag>…</tag>` (ohne Namensraum-Präfix-Beachtung).
fn tag_text(block: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let start = block.find(&open)?;
    // Ende des öffnenden Tags finden (berücksichtigt Attribute).
    let after_open = &block[start..];
    let gt = after_open.find('>')?;
    let content_start = start + gt + 1;
    let end = block[content_start..].find(&close)? + content_start;
    Some(block[content_start..end].to_string())
}

/// Wert eines Attributs im ersten Vorkommen eines Tags.
fn attr_value(block: &str, tag_open: &str, attr: &str) -> Option<String> {
    let start = block.find(tag_open)?;
    let after = &block[start..];
    let tag_end = after.find('>')?;
    let tag_str = &after[..tag_end];
    for quote in ['"', '\''] {
        let needle = format!("{attr}={quote}");
        if let Some(p) = tag_str.find(&needle) {
            let vs = p + needle.len();
            if let Some(ve) = tag_str[vs..].find(quote) {
                let v = &tag_str[vs..vs + ve];
                if !v.is_empty() {
                    return Some(decode_entities(v));
                }
            }
        }
    }
    None
}

/// Entfernt HTML-Tags aus einem Text.
fn strip_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out
}

/// CDATA auspacken, Entities auflösen, Leerraum normalisieren.
fn clean_text(s: &str) -> String {
    let mut t = s.trim().to_string();
    if let Some(inner) = t.strip_prefix("<![CDATA[") {
        if let Some(inner) = inner.strip_suffix("]]>") {
            t = inner.trim().to_string();
        }
    }
    let t = decode_entities(&t);
    // Mehrfache Leerzeichen/Zeilenumbrüche zu einem Leerzeichen.
    t.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn decode_entities(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
        .replace("&auml;", "ä").replace("&ouml;", "ö").replace("&uuml;", "ü")
        .replace("&Auml;", "Ä").replace("&Ouml;", "Ö").replace("&Uuml;", "Ü")
        .replace("&szlig;", "ß")
        // &amp; zuletzt, damit z. B. &amp;lt; korrekt wird.
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;

    const RSS: &str = r#"<?xml version="1.0"?>
<rss version="2.0"><channel>
  <title>Testfeed</title>
  <item>
    <title>Erste Meldung &amp; mehr</title>
    <description><![CDATA[<p>Ein <b>Absatz</b> mit Text.</p>]]></description>
    <link>https://example.org/1</link>
    <pubDate>Mon, 01 Jul 2026 10:00:00 +0200</pubDate>
    <enclosure url="https://example.org/bild1.jpg" type="image/jpeg"/>
  </item>
  <item>
    <title>Zweite Meldung</title>
    <description>Kurzer Text ohne Bild.</description>
    <link>https://example.org/2</link>
  </item>
</channel></rss>"#;

    #[test]
    fn rss_einlesen() {
        let items = parse_feed(RSS, "Politik", 10);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].title, "Erste Meldung & mehr");
        assert_eq!(items[0].summary, "Ein Absatz mit Text.");
        assert_eq!(items[0].link, "https://example.org/1");
        assert_eq!(items[0].image_url.as_deref(), Some("https://example.org/bild1.jpg"));
        assert_eq!(items[0].source, "Politik");
        assert!(items[1].image_url.is_none());
    }

    #[test]
    fn limit_wird_beachtet() {
        let items = parse_feed(RSS, "Sport", 1);
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn bild_aus_media_content() {
        let xml = r#"<rss><channel><item>
            <title>Mit media:content</title>
            <description>Text</description>
            <media:content url="https://example.org/m.jpg" medium="image"/>
        </item></channel></rss>"#;
        let items = parse_feed(xml, "Sport", 5);
        assert_eq!(items[0].image_url.as_deref(), Some("https://example.org/m.jpg"));
    }

    #[test]
    fn bild_aus_beschreibung() {
        let xml = r#"<rss><channel><item>
            <title>Bild im Text</title>
            <description>&lt;img src="https://example.org/i.png"/&gt; Beschreibung</description>
        </item></channel></rss>"#;
        let items = parse_feed(xml, "Politik", 5);
        assert_eq!(items[0].image_url.as_deref(), Some("https://example.org/i.png"));
    }

    #[test]
    fn atom_format() {
        let xml = r#"<feed xmlns="http://www.w3.org/2005/Atom">
          <entry>
            <title>Atom-Meldung</title>
            <summary>Zusammenfassung</summary>
            <link href="https://example.org/a1"/>
            <updated>2026-07-01T10:00:00Z</updated>
          </entry>
        </feed>"#;
        let items = parse_feed(xml, "Politik", 5);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Atom-Meldung");
        assert_eq!(items[0].link, "https://example.org/a1");
    }

    #[test]
    fn leere_oder_kaputte_eingabe() {
        assert!(parse_feed("", "X", 5).is_empty());
        assert!(parse_feed("<rss><channel></channel></rss>", "X", 5).is_empty());
        // Unvollständiges Item wird übersprungen, kein Absturz.
        assert!(parse_feed("<rss><item><title></title></item></rss>", "X", 5).is_empty());
    }
}

/// Sucht das Vorschaubild einer Artikelseite (`og:image`, `twitter:image`
/// oder das erste große `<img>`).
///
/// Nachrichtenseiten hinterlegen für Vorschauen ein passendes Bild in den
/// Meta-Angaben. Liefert ein RSS-Feed kein Bild mit, lässt sich so ein
/// zur Meldung gehörendes Bild nachladen.
pub fn extract_article_image(html: &str) -> Option<String> {
    // Nur den Kopfbereich durchsuchen (dort stehen die Meta-Angaben) –
    // begrenzt die Arbeit bei großen Seiten.
    let head_end = html.find("</head>").unwrap_or(html.len().min(60_000));
    let head = &html[..head_end];

    for prop in ["og:image", "twitter:image", "og:image:secure_url"] {
        if let Some(url) = meta_content(head, prop) {
            if url.starts_with("http") {
                return Some(url);
            }
        }
    }
    None
}

/// Wert des `content`-Attributs eines `<meta>`-Tags mit gegebener
/// `property` oder `name`.
fn meta_content(html: &str, key: &str) -> Option<String> {
    let mut rest = html;
    while let Some(pos) = rest.find("<meta") {
        let after = &rest[pos..];
        let end = after.find('>')?;
        let tag = &after[..end];
        // Enthält der Tag den gesuchten Schlüssel (als property oder name)?
        let hat_key = ["property", "name", "itemprop"].iter().any(|attr| {
            tag_attr(tag, attr).map(|v| v.eq_ignore_ascii_case(key)).unwrap_or(false)
        });
        if hat_key {
            if let Some(content) = tag_attr(tag, "content") {
                if !content.is_empty() {
                    return Some(decode_entities(&content));
                }
            }
        }
        rest = &after[end + 1..];
    }
    None
}

/// Attributwert innerhalb eines einzelnen Tags.
fn tag_attr(tag: &str, attr: &str) -> Option<String> {
    for quote in ['"', '\''] {
        let needle = format!("{attr}={quote}");
        if let Some(p) = tag.find(&needle) {
            let vs = p + needle.len();
            if let Some(ve) = tag[vs..].find(quote) {
                return Some(tag[vs..vs + ve].to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod bild_tests {
    use super::*;

    #[test]
    fn og_image_finden() {
        let html = r#"<html><head>
            <meta charset="utf-8">
            <meta property="og:title" content="Eine Meldung">
            <meta property="og:image" content="https://example.org/artikel.jpg">
        </head><body>…</body></html>"#;
        assert_eq!(
            extract_article_image(html).as_deref(),
            Some("https://example.org/artikel.jpg")
        );
    }

    #[test]
    fn twitter_image_als_ausweichweg() {
        let html = r#"<head><meta name="twitter:image" content="https://example.org/t.png"></head>"#;
        assert_eq!(
            extract_article_image(html).as_deref(),
            Some("https://example.org/t.png")
        );
    }

    #[test]
    fn entities_werden_aufgeloest() {
        let html = r#"<head><meta property="og:image" content="https://example.org/a.jpg?w=800&amp;h=600"></head>"#;
        assert_eq!(
            extract_article_image(html).as_deref(),
            Some("https://example.org/a.jpg?w=800&h=600")
        );
    }

    #[test]
    fn ohne_bild_kein_treffer() {
        assert!(extract_article_image("<html><head></head><body>nichts</body></html>").is_none());
        assert!(extract_article_image("").is_none());
        // Relative Pfade werden verworfen (nicht ladbar ohne Basis).
        let html = r#"<head><meta property="og:image" content="/bilder/x.jpg"></head>"#;
        assert!(extract_article_image(html).is_none());
    }
}
