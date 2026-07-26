import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { backend } from "../lib/backend";
import EmptyState from "../components/EmptyState";
import Poster from "../components/Poster";
import Player from "../components/Player";
import { useResizable } from "../components/useResizable";
import type { Channel, Movie, Provider } from "../lib/types";

const PAGE = 60;

export default function Movies() {
  const { t } = useTranslation();
  const [providers, setProviders] = useState<Provider[] | null>(null);
  const [providerId, setProviderId] = useState<number | null>(null);
  const [categories, setCategories] = useState<string[]>([]);
  const [category, setCategory] = useState<string | null>(null);
  const [movies, setMovies] = useState<Movie[]>([]);
  const [exhausted, setExhausted] = useState(false);
  const [detail, setDetail] = useState<Movie | null>(null);
  const [playing, setPlaying] = useState<Movie | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const catCol = useResizable("ui.movies_cat_width", 260, 170, 460);

  useEffect(() => {
    backend.listProviders().then((ps) => {
      // Nur aktivierte Anbieter berücksichtigen.
      const active = ps.filter((p) => p.enabled);
      setProviders(active);
      if (active.length > 0) setProviderId(active[0].id);
    });
  }, []);

  useEffect(() => {
    if (providerId == null) return;
    setCategory(null);
    backend.movieCategories(providerId).then(setCategories);
  }, [providerId]);

  useEffect(() => {
    if (providerId == null) return;
    setMovies([]); setExhausted(false);
    void loadMore(providerId, category, []);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [providerId, category]);

  async function loadMore(pid: number, cat: string | null, current: Movie[]) {
    const page = await backend.listMovies(pid, cat, PAGE, current.length);
    setMovies([...current, ...page]);
    if (page.length < PAGE) setExhausted(true);
  }

  // Infinite Scroll.
  const onScroll = () => {
    const el = scrollRef.current;
    if (!el || exhausted || providerId == null) return;
    if (el.scrollTop + el.clientHeight >= el.scrollHeight - 400) {
      void loadMore(providerId, category, movies);
    }
  };

  const asChannel = (m: Movie): Channel => ({
    id: m.id, provider_id: m.provider_id, group_id: null, name: m.name, url: m.url,
    tvg_id: null, tvg_name: null, logo_url: m.poster_url, channel_number: null,
    is_radio: false, hidden: false, locked: false, sort_index: 0,
  });

  if (providers !== null && providers.length === 0) {
    return (<><h1>{t("nav.movies")}</h1><EmptyState title={t("movies.emptyTitle")} text={t("empty.movies")} /></>);
  }

  return (
    <>
      <header className="row" style={{ justifyContent: "space-between" }}>
        <div>
          <h1>{t("nav.movies")}</h1>
          <p className="dim">{category ?? t("movies.allCategories")} · {t("movies.count", { count: movies.length })}</p>
        </div>
        {providers && providers.length > 1 && (
          <select style={{ width: 220 }} value={providerId ?? ""} onChange={(e) => setProviderId(Number(e.target.value))}>
            {providers.map((p) => <option key={p.id} value={p.id}>{p.name}</option>)}
          </select>
        )}
      </header>

      <div className="row" style={{ alignItems: "stretch", gap: 0, flex: 1, minHeight: 0 }}>
        {/* Kategorienliste links (ziehbare Breite) – wie bei Live-TV */}
        <aside
          className="card"
          style={{ width: catCol.width, flexShrink: 0, minHeight: 0, overflowY: "auto", display: "flex", flexDirection: "column", gap: 2, padding: 6 }}
        >
          <button
            className={`group-btn ${category === null ? "active" : ""}`}
            onClick={() => setCategory(null)}
          >
            {t("movies.allCategories")}
          </button>
          {categories.map((c) => (
            <button
              key={c}
              className={`group-btn ${category === c ? "active" : ""}`}
              onClick={() => setCategory(c)}
              title={c}
            >
              {c}
            </button>
          ))}
        </aside>

        {/* Ziehbare Trennlinie */}
        <div
          className="col-resizer"
          onMouseDown={catCol.onMouseDown}
          role="separator"
          aria-orientation="vertical"
          aria-label={t("live.resizeColumns")}
        />

        {/* Filmraster rechts */}
        <div ref={scrollRef} onScroll={onScroll} className="card grow" style={{ overflowY: "auto", padding: 12 }}>
          {movies.length === 0 && !exhausted && (
            <div className="vod-grid">
              {Array.from({ length: 12 }).map((_, i) => (
                <div key={i} className="skeleton" style={{ aspectRatio: "2 / 3", borderRadius: "var(--radius-m)" }} />
              ))}
            </div>
          )}

          {movies.length === 0 && exhausted && (
            <EmptyState title={t("movies.emptyTitle")} text={t("empty.movies")} />
          )}

          <div className="vod-grid">
            {movies.map((m) => (
              <div
                key={m.id} className="vod-card" tabIndex={0} role="button"
                onClick={() => setDetail(m)}
                onKeyDown={(e) => { if (e.key === "Enter") setDetail(m); }}
              >
                <Poster src={m.poster_url} alt={m.name} />
                <span className="title">{m.name}</span>
                {(m.year || m.rating) && (
                  <span className="meta">
                    {m.year ?? ""}{m.year && m.rating ? " · " : ""}{m.rating ? `★ ${m.rating.toFixed(1)}` : ""}
                  </span>
                )}
              </div>
            ))}
          </div>
        </div>
      </div>

      {/* Detail-Overlay */}
      {detail && (
        <div className="vod-detail-overlay" onClick={() => setDetail(null)}>
          <div className="vod-detail" onClick={(e) => e.stopPropagation()}>
            <Poster src={detail.poster_url} alt={detail.name} />
            <div>
              <h2>{detail.name}</h2>
              <div className="detail-meta">
                {[detail.year, detail.genre, detail.duration_s ? `${Math.round(detail.duration_s / 60)} min` : null,
                  detail.age_rating, detail.rating ? `★ ${detail.rating.toFixed(1)}` : null]
                  .filter(Boolean).join(" · ")}
              </div>
              {detail.plot && <p className="plot">{detail.plot}</p>}
              {detail.director && <p className="meta">{t("movies.director")}: {detail.director}</p>}
              {detail.cast && <p className="meta">{t("movies.cast")}: {detail.cast}</p>}
              <div className="row" style={{ gap: 10, marginTop: 16 }}>
                <button className="primary" onClick={() => { setPlaying(detail); setDetail(null); }}>
                  ▶ {t("movies.play")}
                </button>
                <button onClick={() => setDetail(null)}>{t("player.close")}</button>
              </div>
            </div>
          </div>
        </div>
      )}

      {playing && (
        <Player channel={asChannel(playing)} onClose={() => setPlaying(null)} />
      )}
    </>
  );
}
