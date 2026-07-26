import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { backend } from "../lib/backend";
import type { NewsItem } from "../lib/types";

/**
 * Nachrichten-Slideshow für die Startseite.
 *
 * Zeigt abwechselnd Meldungen aus Politik und Sport: großes Bild,
 * Überschrift und eine kurze Zusammenfassung. Der Wechsel erfolgt
 * automatisch alle 10 Sekunden; per Klick auf die Punkte oder die Pfeile
 * lässt sich manuell blättern.
 */
const INTERVAL_MS = 10_000;

export default function NewsSlideshow() {
  const { t } = useTranslation();
  const [items, setItems] = useState<NewsItem[] | null>(null);
  const [index, setIndex] = useState(0);
  const [failed, setFailed] = useState(false);
  const [paused, setPaused] = useState(false);
  const timer = useRef<number>();

  // Einmal beim Öffnen laden.
  useEffect(() => {
    let aktiv = true;
    backend.fetchNews(6)
      .then((n) => { if (aktiv) { setItems(n); setFailed(n.length === 0); } })
      .catch(() => { if (aktiv) setFailed(true); });
    return () => { aktiv = false; };
  }, []);

  // Automatischer Wechsel alle 10 Sekunden.
  useEffect(() => {
    if (!items || items.length < 2 || paused) return;
    timer.current = window.setTimeout(
      () => setIndex((i) => (i + 1) % items.length),
      INTERVAL_MS
    );
    return () => window.clearTimeout(timer.current);
  }, [items, index, paused]);

  if (failed) {
    return (
      <div className="news-box news-empty">
        <p className="faint">{t("news.unavailable")}</p>
      </div>
    );
  }

  if (!items) {
    return <div className="news-box skeleton" style={{ minHeight: 320 }} />;
  }

  const current = items[index];
  const go = (delta: number) => setIndex((i) => (i + delta + items.length) % items.length);

  return (
    <div
      className="news-box"
      onMouseEnter={() => setPaused(true)}
      onMouseLeave={() => setPaused(false)}
    >
      <div className="news-media">
        {current.image_url ? (
          <img
            src={current.image_url}
            alt=""
            onError={(e) => { (e.target as HTMLImageElement).style.visibility = "hidden"; }}
          />
        ) : (
          <div className="news-media-fallback" aria-hidden="true" />
        )}
        <span className={`news-tag ${current.source === "Sport" ? "sport" : "politik"}`}>
          {current.source}
        </span>
      </div>

      <div className="news-text">
        <h3>{current.title}</h3>
        <p>{current.summary}</p>
        {current.published && <span className="faint news-date">{current.published}</span>}
      </div>

      {items.length > 1 && (
        <div className="news-controls">
          <button className="icon-btn" onClick={() => go(-1)} aria-label={t("news.prev")}>‹</button>
          <div className="news-dots" role="tablist">
            {items.map((_, i) => (
              <button
                key={i}
                className={`news-dot ${i === index ? "active" : ""}`}
                onClick={() => setIndex(i)}
                aria-label={t("news.goTo", { n: i + 1 })}
                aria-selected={i === index}
                role="tab"
              />
            ))}
          </div>
          <button className="icon-btn" onClick={() => go(1)} aria-label={t("news.next")}>›</button>
        </div>
      )}
    </div>
  );
}
