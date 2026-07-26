import { useEffect, useMemo, useRef, useState } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { useTranslation } from "react-i18next";
import { backend } from "../lib/backend";
import EmptyState from "../components/EmptyState";
import Player from "../components/Player";
import { IconTv } from "../components/Icons";
import { useResizable } from "../components/useResizable";
import type { Channel, ChannelGroup, Provider } from "../lib/types";

const PAGE = 200;
type SortMode = "default" | "name_asc" | "name_desc" | "recently_added" | "channel_number";

export default function LiveTV() {
  const { t } = useTranslation();
  const [providers, setProviders] = useState<Provider[] | null>(null);
  const [providerId, setProviderId] = useState<number | null>(null);
  const [groups, setGroups] = useState<ChannelGroup[]>([]);
  const [groupId, setGroupId] = useState<number | null>(null);
  const [channels, setChannels] = useState<Channel[]>([]);
  const [exhausted, setExhausted] = useState(false);
  const [activeIndex, setActiveIndex] = useState<number | null>(null);
  const [sort, setSort] = useState<SortMode>("default");
  const [favIds, setFavIds] = useState<Set<number>>(new Set());
  const listRef = useRef<HTMLDivElement>(null);
  const groupCol = useResizable("ui.livetv_group_width", 240, 160, 420);

  const reloadFavs = () => backend.favoriteChannelIds().then((ids) => setFavIds(new Set(ids)));
  useEffect(() => { void reloadFavs(); }, []);

  const toggleFav = async (e: React.MouseEvent, id: number) => {
    e.stopPropagation();
    if (favIds.has(id)) await backend.removeFavorite("channel", id);
    else await backend.addFavorite("channel", id);
    await reloadFavs();
  };

  useEffect(() => {
    backend.listProviders().then((ps) => {
      setProviders(ps);
      if (ps.length > 0) setProviderId(ps[0].id);
    });
  }, []);

  useEffect(() => {
    if (providerId == null) return;
    setChannels([]); setExhausted(false); setGroupId(null);
    backend.listGroups(providerId).then(setGroups);
  }, [providerId]);

  useEffect(() => {
    if (providerId == null) return;
    setChannels([]); setExhausted(false);
    void loadMore(providerId, groupId, []);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [providerId, groupId, sort]);

  async function loadMore(pid: number, gid: number | null, current: Channel[]) {
    const page = await backend.listChannels(pid, gid, PAGE, current.length, sort);
    setChannels([...current, ...page]);
    if (page.length < PAGE) setExhausted(true);
  }

  const virtualizer = useVirtualizer({
    count: channels.length,
    getScrollElement: () => listRef.current,
    estimateSize: () => 52,
    overscan: 12,
  });

  // Inkrementelles Nachladen am Listenende.
  const items = virtualizer.getVirtualItems();
  useEffect(() => {
    const last = items[items.length - 1];
    if (!last || exhausted || providerId == null) return;
    if (last.index >= channels.length - 20) {
      void loadMore(providerId, groupId, channels);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [items, exhausted]);

  const groupName = useMemo(
    () => (groupId == null ? t("live.allChannels") : groups.find((g) => g.id === groupId)?.name ?? ""),
    [groupId, groups, t]
  );

  if (providers !== null && providers.length === 0) {
    return (
      <>
        <h1>{t("nav.live")}</h1>
        <EmptyState title={t("live.emptyTitle")} text={t("live.emptyText")} />
      </>
    );
  }

  return (
    <>
      <header className="row" style={{ justifyContent: "space-between" }}>
        <div>
          <h1>{t("nav.live")}</h1>
          <p className="dim">{groupName} · {t("live.channelCount", { count: channels.length })}</p>
        </div>
        {providers && providers.length > 1 && (
          <select
            style={{ width: 220 }}
            value={providerId ?? ""}
            onChange={(e) => setProviderId(Number(e.target.value))}
            aria-label={t("nav.providers")}
          >
            {providers.map((p) => <option key={p.id} value={p.id}>{p.name}</option>)}
          </select>
        )}
      </header>

      {/* Sortierung */}
      <div className="row" style={{ gap: 10, alignItems: "center" }}>
        <span className="faint">{t("live.sortBy")}</span>
        <select value={sort} onChange={(e) => setSort(e.target.value as SortMode)} style={{ width: 220 }} aria-label={t("live.sortBy")}>
          <option value="default">{t("live.sortDefault")}</option>
          <option value="name_asc">{t("live.sortNameAsc")}</option>
          <option value="name_desc">{t("live.sortNameDesc")}</option>
          <option value="recently_added">{t("live.sortRecent")}</option>
          <option value="channel_number">{t("live.sortNumber")}</option>
        </select>
      </div>

      <div className="row" style={{ alignItems: "stretch", gap: 0, flex: 1, minHeight: 0 }}>
        {/* Gruppenliste (ziehbare Breite) */}
        <aside className="card" style={{ width: groupCol.width, flexShrink: 0, minHeight: 0, overflowY: "auto", display: "flex", flexDirection: "column", gap: 2, padding: 6 }}>
          <GroupButton active={groupId === null} onClick={() => setGroupId(null)} label={t("live.allChannels")} />
          {groups.map((g) => (
            <GroupButton key={g.id} active={groupId === g.id} onClick={() => setGroupId(g.id)} label={g.name} />
          ))}
        </aside>

        {/* Ziehbare Trennlinie */}
        <div
          className="col-resizer"
          onMouseDown={groupCol.onMouseDown}
          role="separator"
          aria-orientation="vertical"
          aria-label={t("live.resizeColumns")}
        />

        {/* Virtualisierte Kanalliste */}
        <div ref={listRef} className="card grow" style={{ overflowY: "auto", padding: 8 }}>
          {channels.length === 0 && !exhausted && (
            <div style={{ display: "grid", gap: 8 }}>
              {Array.from({ length: 8 }).map((_, i) => <div key={i} className="skeleton" style={{ height: 44 }} />)}
            </div>
          )}
          <div style={{ height: virtualizer.getTotalSize(), position: "relative" }}>
            {items.map((vi) => {
              const c = channels[vi.index];
              if (!c) return null;
              return (
                <div
                  key={c.id}
                  className="row channel-row"
                  style={{
                    position: "absolute", top: 0, left: 0, right: 0,
                    transform: `translateY(${vi.start}px)`,
                    height: vi.size, padding: "0 10px",
                    borderRadius: "var(--radius-m)",
                  }}
                  tabIndex={0}
                  role="button"
                  aria-label={c.name}
                  title={c.name}
                  onClick={() => setActiveIndex(vi.index)}
                  onKeyDown={(e) => { if (e.key === "Enter") setActiveIndex(vi.index); }}
                >
                  <span className="faint" style={{ width: 44, textAlign: "right" }}>
                    {c.channel_number ?? vi.index + 1}
                  </span>
                  <span className="channel-logo" aria-hidden="true">
                    {c.logo_url
                      ? <img src={c.logo_url} alt="" loading="lazy"
                          onError={(e) => { (e.target as HTMLImageElement).style.display = "none"; }} />
                      : <IconTv />}
                  </span>
                  <span className="grow" style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                    {c.name}
                  </span>
                  {c.is_radio && <span className="faint">Radio</span>}
                  <button
                    className={`chan-star ${favIds.has(c.id) ? "on" : ""}`}
                    onClick={(e) => void toggleFav(e, c.id)}
                    title={favIds.has(c.id) ? t("favorites.remove") : t("favorites.add")}
                    aria-label={favIds.has(c.id) ? t("favorites.remove") : t("favorites.add")}
                  >{favIds.has(c.id) ? "★" : "☆"}</button>
                </div>
              );
            })}
          </div>
        </div>
      </div>

      {activeIndex !== null && channels[activeIndex] && (
        <Player
          channel={channels[activeIndex]}
          onClose={() => setActiveIndex(null)}
          channels={channels}
          activeIndex={activeIndex}
          onSelectIndex={(i) => {
            setActiveIndex(i);
            // Am Listenende nachladen, damit weiter geblättert werden kann.
            if (i >= channels.length - 5 && !exhausted && providerId != null) {
              void loadMore(providerId, groupId, channels);
            }
          }}
          onPrev={activeIndex > 0 ? () => setActiveIndex(activeIndex - 1) : undefined}
          onNext={
            activeIndex < channels.length - 1
              ? () => {
                  const next = activeIndex + 1;
                  setActiveIndex(next);
                  if (next >= channels.length - 5 && !exhausted && providerId != null) {
                    void loadMore(providerId, groupId, channels);
                  }
                }
              : undefined
          }
        />
      )}
    </>
  );
}

function GroupButton({ active, onClick, label }: { active: boolean; onClick: () => void; label: string }) {
  return (
    <button
      className={`group-btn ${active ? "active" : ""}`}
      onClick={onClick}
      title={label}
    >
      {label}
    </button>
  );
}
