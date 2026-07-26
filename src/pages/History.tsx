import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { backend } from "../lib/backend";
import EmptyState from "../components/EmptyState";
import Player from "../components/Player";
import { IconTv } from "../components/Icons";
import type { Channel, HistoryEntry } from "../lib/types";

export default function History() {
  const { t } = useTranslation();
  const [items, setItems] = useState<HistoryEntry[] | null>(null);
  const [playing, setPlaying] = useState<Channel | null>(null);

  const reload = () => backend.listHistory().then(setItems);
  useEffect(() => { void reload(); }, []);

  const clear = async () => {
    if (!window.confirm(t("history.clearConfirm"))) return;
    await backend.clearHistory();
    await reload();
  };

  const play = (h: HistoryEntry) => {
    setPlaying({
      id: h.item_id, provider_id: 0, group_id: null, name: h.name, url: h.url,
      tvg_id: null, tvg_name: null, logo_url: h.logo_url, channel_number: null,
      is_radio: false, hidden: false, locked: false, sort_index: 0,
    });
  };

  if (items === null) return <><h1>{t("nav.history")}</h1><div className="skeleton" style={{ height: 80 }} /></>;

  if (items.length === 0) {
    return <><h1>{t("nav.history")}</h1><EmptyState title={t("history.emptyTitle")} text={t("history.emptyText")} /></>;
  }

  return (
    <>
      <header className="row" style={{ justifyContent: "space-between" }}>
        <h1>{t("nav.history")}</h1>
        <button className="danger" onClick={() => void clear()}>{t("history.clear")}</button>
      </header>
      <div className="history-list">
        {items.map((h) => (
          <div key={`${h.item_type}:${h.item_id}`} className="history-row" role="button" tabIndex={0}
            onClick={() => play(h)}
            onKeyDown={(e) => { if (e.key === "Enter") play(h); }}
          >
            <div className="history-logo">
              {h.logo_url ? <img src={h.logo_url} alt="" loading="lazy" /> : <IconTv />}
            </div>
            <span className="grow">{h.name}</span>
            <span className="faint">{new Date(h.watched_at * 1000).toLocaleString()}</span>
            <span className="primary-text">▶</span>
          </div>
        ))}
      </div>

      {playing && <Player channel={playing} onClose={() => setPlaying(null)} />}
    </>
  );
}
