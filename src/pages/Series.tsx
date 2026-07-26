import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { backend } from "../lib/backend";
import EmptyState from "../components/EmptyState";
import Poster from "../components/Poster";
import Player from "../components/Player";
import { useResizable } from "../components/useResizable";
import type { Channel, Episode, Season, Series as SeriesT, Provider } from "../lib/types";

const PAGE = 60;

export default function Series() {
  const { t } = useTranslation();
  const [providers, setProviders] = useState<Provider[] | null>(null);
  const [providerId, setProviderId] = useState<number | null>(null);
  const [categories, setCategories] = useState<string[]>([]);
  const [category, setCategory] = useState<string | null>(null);
  const [series, setSeries] = useState<SeriesT[]>([]);
  const [exhausted, setExhausted] = useState(false);
  const [detail, setDetail] = useState<SeriesT | null>(null);
  const [seasons, setSeasons] = useState<Season[]>([]);
  const [activeSeason, setActiveSeason] = useState<number | null>(null);
  const [episodes, setEpisodes] = useState<Episode[]>([]);
  const [playing, setPlaying] = useState<{ ep: Episode; poster: string | null; seriesName: string } | null>(null);
  const scrollRef = useRef<HTMLDivElement>(null);
  const catCol = useResizable("ui.series_cat_width", 260, 170, 460);

  useEffect(() => {
    backend.listProviders().then((ps) => {
      const active = ps.filter((p) => p.enabled);
      setProviders(active);
      if (active.length > 0) setProviderId(active[0].id);
    });
  }, []);

  useEffect(() => {
    if (providerId == null) return;
    setCategory(null);
    backend.seriesCategories(providerId).then(setCategories);
  }, [providerId]);

  useEffect(() => {
    if (providerId == null) return;
    setSeries([]); setExhausted(false);
    void loadMore(providerId, category, []);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [providerId, category]);

  async function loadMore(pid: number, cat: string | null, current: SeriesT[]) {
    const page = await backend.listSeries(pid, cat, PAGE, current.length);
    setSeries([...current, ...page]);
    if (page.length < PAGE) setExhausted(true);
  }

  const onScroll = () => {
    const el = scrollRef.current;
    if (!el || exhausted || providerId == null) return;
    if (el.scrollTop + el.clientHeight >= el.scrollHeight - 400) {
      void loadMore(providerId, category, series);
    }
  };

  const [loadingSeasons, setLoadingSeasons] = useState(false);

  // Serie öffnen → Staffeln laden (bei Xtream ggf. erst nachladen).
  const openDetail = async (s: SeriesT) => {
    setDetail(s);
    setSeasons([]); setEpisodes([]); setActiveSeason(null);
    setLoadingSeasons(true);
    try {
      let sn = await backend.listSeasons(s.id);
      // Noch nicht nachgeladen? Dann vom Xtream-Server holen.
      if (sn.length === 0 && s.external_id && providerId != null) {
        await backend.loadSeriesSeasons(providerId, s.id, Number(s.external_id));
        sn = await backend.listSeasons(s.id);
      }
      setSeasons(sn);
      if (sn.length > 0) await selectSeason(sn[0]);
    } finally {
      setLoadingSeasons(false);
    }
  };

  const selectSeason = async (season: Season) => {
    setActiveSeason(season.number);
    const eps = await backend.listEpisodes(season.id);
    setEpisodes(eps);
  };

  const asChannel = (ep: Episode, poster: string | null, name: string): Channel => ({
    id: ep.id, provider_id: providerId ?? 0, group_id: null,
    name: `${name} · ${ep.name ?? `E${ep.number}`}`, url: ep.url,
    tvg_id: null, tvg_name: null, logo_url: poster, channel_number: null,
    is_radio: false, hidden: false, locked: false, sort_index: 0,
  });

  if (providers !== null && providers.length === 0) {
    return (<><h1>{t("nav.series")}</h1><EmptyState title={t("series.emptyTitle")} text={t("empty.series")} /></>);
  }

  return (
    <>
      <header className="row" style={{ justifyContent: "space-between" }}>
        <div>
          <h1>{t("nav.series")}</h1>
          <p className="dim">{category ?? t("movies.allCategories")} · {t("series.count", { count: series.length })}</p>
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
          style={{ width: catCol.width, flexShrink: 0, overflowY: "auto", display: "flex", flexDirection: "column", gap: 2 }}
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

        {/* Serienraster rechts */}
        <div ref={scrollRef} onScroll={onScroll} className="card grow" style={{ overflowY: "auto", padding: 12 }}>
          {series.length === 0 && !exhausted && (
            <div className="vod-grid">
              {Array.from({ length: 12 }).map((_, i) => (
                <div key={i} className="skeleton" style={{ aspectRatio: "2 / 3", borderRadius: "var(--radius-m)" }} />
              ))}
            </div>
          )}

          {series.length === 0 && exhausted && (
            <EmptyState title={t("series.emptyTitle")} text={t("empty.series")} />
          )}

          <div className="vod-grid">
            {series.map((s) => (
              <div
                key={s.id} className="vod-card" tabIndex={0} role="button"
                onClick={() => void openDetail(s)}
                onKeyDown={(e) => { if (e.key === "Enter") void openDetail(s); }}
              >
                <Poster src={s.poster_url} alt={s.name} />
                <span className="title">{s.name}</span>
                {(s.year || s.rating) && (
                  <span className="meta">
                    {s.year ?? ""}{s.year && s.rating ? " · " : ""}{s.rating ? `★ ${s.rating.toFixed(1)}` : ""}
                  </span>
                )}
              </div>
            ))}
          </div>
        </div>
      </div>

      {/* Detail mit Staffelauswahl */}
      {detail && (
        <div className="vod-detail-overlay" onClick={() => setDetail(null)}>
          <div className="vod-detail" onClick={(e) => e.stopPropagation()}>
            <Poster src={detail.poster_url} alt={detail.name} />
            <div>
              <h2>{detail.name}</h2>
              <div className="detail-meta">
                {[detail.year, detail.genre, detail.age_rating,
                  detail.rating ? `★ ${detail.rating.toFixed(1)}` : null]
                  .filter(Boolean).join(" · ")}
              </div>
              {detail.plot && <p className="plot">{detail.plot}</p>}

              {loadingSeasons ? (
                <div style={{ display: "grid", gap: 6 }}>
                  {Array.from({ length: 4 }).map((_, i) => <div key={i} className="skeleton" style={{ height: 38 }} />)}
                </div>
              ) : seasons.length === 0 ? (
                <p className="faint">{t("series.noSeasons")}</p>
              ) : (
                <>
                  {/* Staffel-Tabs */}
                  <div className="season-tabs">
                    {seasons.map((sn) => (
                      <button
                        key={sn.id}
                        className={activeSeason === sn.number ? "active" : ""}
                        onClick={() => void selectSeason(sn)}
                      >
                        {sn.name || t("series.season", { n: sn.number })}
                        <span className="faint"> ({sn.episode_count})</span>
                      </button>
                    ))}
                  </div>

                  {/* Episoden */}
                  <div className="episode-list">
                    {episodes.map((ep) => (
                      <div
                        key={ep.id} className="episode-row" tabIndex={0} role="button"
                        onClick={() => { setPlaying({ ep, poster: detail.poster_url, seriesName: detail.name }); setDetail(null); }}
                        onKeyDown={(e) => { if (e.key === "Enter") { setPlaying({ ep, poster: detail.poster_url, seriesName: detail.name }); setDetail(null); } }}
                      >
                        <span className="num">{ep.number}</span>
                        <span className="grow">{ep.name || t("series.episode", { n: ep.number })}</span>
                        {ep.duration_s && <span className="faint">{Math.round(ep.duration_s / 60)} min</span>}
                        <span className="primary-text">▶</span>
                      </div>
                    ))}
                  </div>
                </>
              )}

              <div className="row" style={{ marginTop: 16 }}>
                <button onClick={() => setDetail(null)}>{t("player.close")}</button>
              </div>
            </div>
          </div>
        </div>
      )}

      {playing && (
        <Player
          channel={asChannel(playing.ep, playing.poster, playing.seriesName)}
          onClose={() => setPlaying(null)}
        />
      )}
    </>
  );
}
