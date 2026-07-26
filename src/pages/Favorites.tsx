import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { backend } from "../lib/backend";
import EmptyState from "../components/EmptyState";
import Player from "../components/Player";
import { IconTv } from "../components/Icons";
import type { Channel } from "../lib/types";

export default function Favorites() {
  const { t } = useTranslation();
  const [channels, setChannels] = useState<Channel[] | null>(null);
  const [activeIndex, setActiveIndex] = useState<number | null>(null);

  const reload = () => backend.favoriteChannels().then(setChannels);
  useEffect(() => { void reload(); }, []);

  const removeFav = async (e: React.MouseEvent, id: number) => {
    e.stopPropagation();
    await backend.removeFavorite("channel", id);
    await reload();
  };

  if (channels === null) return <><h1>{t("nav.favorites")}</h1><div className="skeleton" style={{ height: 80 }} /></>;

  if (channels.length === 0) {
    return <><h1>{t("nav.favorites")}</h1><EmptyState title={t("favorites.emptyTitle")} text={t("favorites.emptyText")} /></>;
  }

  return (
    <>
      <h1>{t("nav.favorites")}</h1>
      <div className="fav-grid">
        {channels.map((c, i) => (
          <div key={c.id} className="fav-card" role="button" tabIndex={0}
            onClick={() => setActiveIndex(i)}
            onKeyDown={(e) => { if (e.key === "Enter") setActiveIndex(i); }}
          >
            <div className="fav-logo">
              {c.logo_url ? <img src={c.logo_url} alt="" loading="lazy" /> : <IconTv />}
            </div>
            <span className="fav-name">{c.name}</span>
            <button className="fav-star" onClick={(e) => void removeFav(e, c.id)} title={t("favorites.remove")} aria-label={t("favorites.remove")}>★</button>
          </div>
        ))}
      </div>

      {activeIndex !== null && channels[activeIndex] && (
        <Player
          channel={channels[activeIndex]}
          onClose={() => setActiveIndex(null)}
          channels={channels}
          activeIndex={activeIndex}
          onSelectIndex={(i) => setActiveIndex(i)}
          onPrev={activeIndex > 0 ? () => setActiveIndex(activeIndex - 1) : undefined}
          onNext={activeIndex < channels.length - 1 ? () => setActiveIndex(activeIndex + 1) : undefined}
        />
      )}
    </>
  );
}
