import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { backend, isTauri } from "../lib/backend";
import type { Channel } from "../lib/types";
import type { PlaybackStatus } from "../lib/types";
import { IconTv } from "./Icons";
import "./player.css";

interface PlayerProps {
  channel: Channel | null;
  onClose: () => void;
  onNext?: () => void;
  onPrev?: () => void;
  /** Kanäle der aktuellen Gruppe für den Umschalter (optional). */
  channels?: Channel[];
  /** Index des aktiven Kanals in `channels`. */
  activeIndex?: number;
  /** Kanal per Index wählen (Umschalter). */
  onSelectIndex?: (index: number) => void;
}

/**
 * Video-Overlay. Der eigentliche Videobereich ist ein transparenter
 * Platzhalter: das native mpv-Fenster (Rust-Seite) wird exakt über diesen
 * Bereich positioniert. Wir melden dessen Bildschirm-Bounds bei jeder
 * Größen-/Positionsänderung an das Backend.
 */
export default function Player({ channel, onClose, onNext, onPrev, channels, activeIndex, onSelectIndex }: PlayerProps) {
  const { t } = useTranslation();
  const surfaceRef = useRef<HTMLDivElement>(null);
  const [status, setStatus] = useState<PlaybackStatus | null>(null);
  const [setupMsg, setSetupMsg] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [volume, setVolume] = useState(100);
  const [tracks, setTracks] = useState<PlaybackStatus["tracks"]>([]);
  const [aspect, setAspect] = useState<string>("auto");
  const [showTrackMenu, setShowTrackMenu] = useState(false);
  const [showChannelList, setShowChannelList] = useState(false);
  const channelListRef = useRef<HTMLDivElement>(null);
  const hasChannelList = Array.isArray(channels) && channels.length > 0 && !!onSelectIndex;

  // Bounds des Platzhalters an das native Videofenster melden.
  const reportBounds = useCallback(() => {
    if (!isTauri) return;
    const el = surfaceRef.current;
    if (!el) return;
    const r = el.getBoundingClientRect();
    const dpr = window.devicePixelRatio || 1;
    void backend.playbackSetBounds(
      Math.round(r.left * dpr),
      Math.round(r.top * dpr),
      Math.round(r.width * dpr),
      Math.round(r.height * dpr),
    );
  }, []);

  // Status- und Fehler-Listener.
  useEffect(() => {
    let unStatus: (() => void) | undefined;
    let unError: (() => void) | undefined;
    let unSetup: (() => void) | undefined;
    void backend.onPlaybackStatus((s) => {
      setStatus(s);
      setError(null);
      // Spuren kommen nur bei Änderung mit (nicht-leer) – dann übernehmen.
      if (s.tracks && s.tracks.length > 0) setTracks(s.tracks);
    }).then((u) => (unStatus = u));
    void backend.onPlaybackError((m) => setError(m)).then((u) => (unError = u));
    void backend.onMpvSetup((s) => {
      setSetupMsg(s.stage === "bereit" || s.stage === "fertig" || s.stage === "vorhanden" ? null : s.message);
    }).then((u) => (unSetup = u));
    return () => { unStatus?.(); unError?.(); unSetup?.(); };
  }, []);

  // Kanal starten.
  useEffect(() => {
    if (!channel) return;
    setError(null);
    setStatus({ state: "loading", position: null, duration: null, volume, recovering: false, recoveryStage: 0, title: channel.name, tracks: [] });
    (async () => {
      try {
        await backend.ensurePlaybackReady();
        reportBounds();
        await backend.playbackShowVideo(true);
        await backend.playbackLoad({
          url: channel.url,
          title: channel.name,
        });
        reportBounds();
        // In den Wiedergabeverlauf eintragen (nur bei echten Kanälen mit ID).
        if (channel.id > 0) {
          void backend.addHistory("channel", channel.id, channel.name, channel.url, channel.logo_url ?? null);
        }
      } catch (e) {
        setError(String(e));
      }
    })();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [channel?.id]);

  // Bounds bei Resize/Scroll aktualisieren.
  useEffect(() => {
    reportBounds();
    const ro = new ResizeObserver(reportBounds);
    if (surfaceRef.current) ro.observe(surfaceRef.current);
    window.addEventListener("resize", reportBounds);
    return () => { ro.disconnect(); window.removeEventListener("resize", reportBounds); };
  }, [reportBounds]);

  // Steuerung automatisch ausblenden.
  // Die Steuerleisten liegen jetzt in festen Bereichen ober-/unterhalb des
  // Videos und bleiben dauerhaft sichtbar (kein Ausblenden nötig – so ist die
  // Bedienung immer erreichbar).
  const wakeControls = useCallback(() => {}, []);
  useEffect(() => {}, []);

  const close = useCallback(async () => {
    try { await backend.playbackStop(); await backend.playbackShowVideo(false); } catch { /* egal */ }
    onClose();
  }, [onClose]);

  // Beim Öffnen der Senderliste zum aktiven Kanal scrollen.
  useEffect(() => {
    if (showChannelList && channelListRef.current && activeIndex != null) {
      const el = channelListRef.current.querySelector<HTMLElement>(`[data-idx="${activeIndex}"]`);
      el?.scrollIntoView({ block: "center" });
    }
  }, [showChannelList, activeIndex]);

  // Vollbild des gesamten Fensters umschalten.
  // Im Vollbild werden Titel- und Steuerleiste ausgeblendet, damit nur noch
  // das Bild zu sehen ist. Escape kehrt zurück (schließt NICHT die Wiedergabe).
  const [isFullscreen, setIsFullscreen] = useState(false);

  const setFullscreen = useCallback(async (on: boolean) => {
    if (!isTauri) { setIsFullscreen(on); return; }
    try {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      await getCurrentWindow().setFullscreen(on);
      setIsFullscreen(on);
    } catch {
      setIsFullscreen(on);
    }
  }, []);

  const toggleFullscreen = useCallback(async () => {
    await setFullscreen(!isFullscreen);
  }, [isFullscreen, setFullscreen]);

  // Beim Verlassen der Wiedergabe immer den Vollbildmodus beenden.
  useEffect(() => {
    return () => {
      if (isTauri) {
        void (async () => {
          try {
            const { getCurrentWindow } = await import("@tauri-apps/api/window");
            await getCurrentWindow().setFullscreen(false);
          } catch { /* egal */ }
        })();
      }
    };
  }, []);

  // Tastatursteuerung.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      switch (e.key) {
        case "Escape":
          // Im Vollbild: zurück zur normalen Ansicht (Steuerung wieder da),
          // damit Sender/Film gewechselt werden können.
          if (isFullscreen) { void setFullscreen(false); }
          else { void close(); }
          break;
        case " ": e.preventDefault(); void backend.playbackTogglePause(); break;
        case "ArrowUp": setVol(Math.min(150, volume + 5)); break;
        case "ArrowDown": setVol(Math.max(0, volume - 5)); break;
        case "ArrowLeft": void backend.playbackSeekRelative(-15); break;
        case "ArrowRight": void backend.playbackSeekRelative(15); break;
        case "PageUp": onPrev?.(); break;
        case "PageDown": onNext?.(); break;
        case "f": case "F": void toggleFullscreen(); break;
        case "m": case "M": setVol(volume === 0 ? 100 : 0); break;
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [volume, close, onPrev, onNext, toggleFullscreen, isFullscreen, setFullscreen]);

  const setVol = (v: number) => { setVolume(v); void backend.playbackSetVolume(v); };

  if (!channel) return null;

  const st = status?.state ?? "loading";
  const loading = st === "loading" || st === "buffering";
  const recovering = status?.recovering ?? false;
  // VOD erkannt, sobald eine endliche Dauer bekannt ist (Live-Streams: keine).
  const dur = status?.duration ?? 0;
  const pos = status?.position ?? 0;
  // VOD-Erkennung: Live-Streams melden über den Puffer oft eine kurze
  // "Dauer" (wenige Minuten). Echte Filme/Episoden sind deutlich länger.
  // Erst ab 5 Minuten zeigen wir die Fortschritts-/Spulleiste.
  const isVod = dur > 300;
  const audioTracks = tracks.filter((tr) => tr.kind === "audio");
  const subtitleTracks = tracks.filter((tr) => tr.kind === "subtitle");

  return (
    <div className={`player-overlay ${isFullscreen ? "is-fullscreen" : ""}`} onMouseMove={wakeControls}>
      {/* OBEN: Titelleiste – liegt außerhalb des Videofensters, immer klickbar */}
      <div className="player-topbar">
        <button className="icon-btn" onClick={() => void close()} aria-label={t("player.close")} title={t("player.close")}>
          ← {t("player.close")}
        </button>
        {/* Sender-Umschalter: öffnet die Liste der Gruppe */}
        {hasChannelList && (
          <button
            className={`icon-btn ${showChannelList ? "active" : ""}`}
            onClick={() => setShowChannelList((s) => !s)}
            aria-label={t("player.channelList")}
            title={t("player.channelList")}
          >☰ {t("player.channelList")}</button>
        )}
        {(onPrev || onNext) && (
          <div className="channel-switcher" role="group" aria-label={t("player.channelSwitch")}>
            <button className="icon-btn" onClick={() => onPrev?.()} disabled={!onPrev} aria-label={t("player.prevChannel")} title={t("player.prevChannel")}>▲</button>
            <button className="icon-btn" onClick={() => onNext?.()} disabled={!onNext} aria-label={t("player.nextChannel")} title={t("player.nextChannel")}>▼</button>
          </div>
        )}
        <span className="player-title">
          <span className="player-logo" aria-hidden="true">
            {channel.logo_url ? <img src={channel.logo_url} alt="" /> : <IconTv />}
          </span>
          {channel.name}
        </span>
        <span className="grow" />
        <span className="player-state faint">
          {st === "playing" && t("player.live")}
          {st === "paused" && t("player.paused")}
          {(st === "loading" || st === "buffering") && t("player.buffering")}
        </span>
        <button className="icon-btn" onClick={() => void close()} aria-label={t("player.close")} title={t("player.close")}>✕</button>
      </div>

      {/* MITTE: Videofläche – native mpv-Ausgabe liegt exakt hier darüber.
          Doppelklick schaltet Vollbild. */}
      <div
        className="player-surface"
        ref={surfaceRef}
        onDoubleClick={() => void toggleFullscreen()}
      >
        {/* Kurzer Hinweis nach dem Wechsel in den Vollbildmodus */}
        {isFullscreen && (
          <div className="fs-hint" key={`fs-${channel.id}`}>{t("player.fullscreenHint")}</div>
        )}

        {/* Hotspot OBEN LINKS: nur wenn die Maus hier hinfährt, öffnet sich
            der Sender-Umschalter. So bleibt er nicht dauerhaft offen. */}
        {hasChannelList && !showChannelList && (
          <div
            className="channel-hotspot"
            onMouseEnter={() => setShowChannelList(true)}
            aria-hidden="true"
          />
        )}

        {/* Sender-Umschalter-Panel */}
        {showChannelList && hasChannelList && (
          <div className="channel-panel" ref={channelListRef}
            onMouseLeave={() => setShowChannelList(false)}
          >
            <div className="channel-panel-head">
              <span>{t("player.channelList")}</span>
              <button className="icon-btn" onClick={() => setShowChannelList(false)} aria-label={t("player.close")}>✕</button>
            </div>
            <div className="channel-panel-list">
              {channels!.map((c, i) => (
                <button
                  key={c.id}
                  data-idx={i}
                  className={`channel-panel-item ${i === activeIndex ? "active" : ""}`}
                  onClick={() => { onSelectIndex!(i); setShowChannelList(false); }}
                >
                  <span className="cp-num">{c.channel_number ?? i + 1}</span>
                  <span className="cp-logo">
                    {c.logo_url ? <img src={c.logo_url} alt="" loading="lazy"
                      onError={(e) => { (e.target as HTMLImageElement).style.display = "none"; }} /> : <IconTv />}
                  </span>
                  <span className="cp-name">{c.name}</span>
                </button>
              ))}
            </div>
          </div>
        )}

        {!isTauri && (
          <div className="player-mock">
            <div className="ring" aria-hidden="true" />
            <p>{t("player.browserPreview")}</p>
          </div>
        )}

        {/* Ladeanzeige */}
        {(loading || recovering) && !error && (
          <div className="player-loading" role="status">
            <div className="ring spinning" aria-hidden="true" />
            <p>
              {setupMsg ? setupMsg : recovering ? t("player.reconnecting") : t("player.loading", { name: channel.name })}
            </p>
          </div>
        )}

        {/* Fehlermeldung */}
        {error && (
          <div className="player-error card" role="alert">
            <h3>{t("player.errorTitle")}</h3>
            <p>{error}</p>
            <div className="row" style={{ justifyContent: "center", gap: "var(--gap-s)" }}>
              <button className="primary" onClick={() => { setError(null); void backend.playbackLoad({ url: channel.url, title: channel.name }); }}>
                {t("player.retry")}
              </button>
              <button onClick={() => void close()}>{t("player.close")}</button>
            </div>
          </div>
        )}
      </div>

      {/* UNTEN: Steuerleiste – außerhalb des Videofensters, immer klickbar */}
      <div className="player-bottombar">
        <div className="player-bottom">
          {/* VOD-Fortschrittsleiste (nur wenn eine Dauer bekannt ist) */}
          {isVod && (
            <div className="player-progress">
              <span className="player-time">{fmtTime(pos)}</span>
              <input
                type="range"
                className="progress-bar"
                min={0}
                max={Math.max(1, dur)}
                step={1}
                value={Math.min(pos, dur)}
                onChange={(e) => void backend.playbackSeek(Number(e.target.value))}
                aria-label={t("player.seek")}
              />
              <span className="player-time">{fmtTime(dur)}</span>
            </div>
          )}

          <div className="player-buttons">
            {/* Spulen (VOD) */}
            {isVod && (
              <>
                <button className="icon-btn" onClick={() => void backend.playbackSeekRelative(-300)} aria-label={t("player.back5min")} title={t("player.back5min")}>«</button>
                <button className="icon-btn" onClick={() => void backend.playbackSeekRelative(-15)} aria-label={t("player.back15")} title={t("player.back15")}>−15</button>
              </>
            )}
            <button className="icon-btn primary-btn" onClick={() => void backend.playbackTogglePause()} aria-label={t("player.playPause")}>
              {st === "paused" ? "▶" : "❚❚"}
            </button>
            {isVod && (
              <>
                <button className="icon-btn" onClick={() => void backend.playbackSeekRelative(15)} aria-label={t("player.fwd15")} title={t("player.fwd15")}>+15</button>
                <button className="icon-btn" onClick={() => void backend.playbackSeekRelative(300)} aria-label={t("player.fwd5min")} title={t("player.fwd5min")}>»</button>
              </>
            )}

            <div className="player-volume">
              <button className="icon-btn" onClick={() => setVol(volume === 0 ? 100 : 0)} aria-label={t("player.mute")}>
                {volume === 0 ? "🔇" : "🔊"}
              </button>
              <input
                type="range" min={0} max={150} value={volume}
                onChange={(e) => setVol(Number(e.target.value))}
                aria-label={t("player.volume")}
              />
            </div>

            <div className="player-spacer" />

            {/* Ton-/Untertitelspur */}
            {(audioTracks.length > 1 || subtitleTracks.length > 0) && (
              <div className="track-menu-wrap">
                <button
                  className="icon-btn"
                  onClick={() => setShowTrackMenu((s) => !s)}
                  aria-label={t("player.tracks")}
                  title={t("player.tracks")}
                >CC</button>
                {showTrackMenu && (
                  <div className="track-menu card">
                    {audioTracks.length > 1 && (
                      <>
                        <div className="track-menu-title">{t("player.audio")}</div>
                        {audioTracks.map((tr) => (
                          <button
                            key={`a${tr.id}`}
                            className={tr.selected ? "sel" : ""}
                            onClick={() => { void backend.playbackSelectAudio(tr.id); setShowTrackMenu(false); }}
                          >
                            {trackLabel(tr, t)}
                          </button>
                        ))}
                      </>
                    )}
                    {subtitleTracks.length > 0 && (
                      <>
                        <div className="track-menu-title">{t("player.subtitles")}</div>
                        <button
                          className={!subtitleTracks.some((s) => s.selected) ? "sel" : ""}
                          onClick={() => { void backend.playbackSelectSubtitle(null); setShowTrackMenu(false); }}
                        >{t("player.subtitlesOff")}</button>
                        {subtitleTracks.map((tr) => (
                          <button
                            key={`s${tr.id}`}
                            className={tr.selected ? "sel" : ""}
                            onClick={() => { void backend.playbackSelectSubtitle(tr.id); setShowTrackMenu(false); }}
                          >
                            {trackLabel(tr, t)}
                          </button>
                        ))}
                      </>
                    )}
                  </div>
                )}
              </div>
            )}

            {/* Format-Umschalter */}
            <button
              className="icon-btn"
              onClick={() => {
                const next = nextAspect(aspect);
                setAspect(next);
                void backend.playbackSetAspect(next === "auto" ? null : next);
              }}
              aria-label={t("player.aspect")}
              title={`${t("player.aspect")}: ${aspectLabel(aspect)}`}
            >⛶ {aspectLabel(aspect)}</button>

            {/* Vollbild */}
            <button
              className="icon-btn"
              onClick={() => void toggleFullscreen()}
              aria-label={t("player.fullscreen")}
              title={t("player.fullscreen")}
            >⛶</button>
          </div>
        </div>
      </div>
    </div>
  );
}

// ===== Hilfsfunktionen =====

function fmtTime(sec: number): string {
  if (!isFinite(sec) || sec < 0) return "0:00";
  const h = Math.floor(sec / 3600);
  const m = Math.floor((sec % 3600) / 60);
  const s = Math.floor(sec % 60);
  const mm = h > 0 ? String(m).padStart(2, "0") : String(m);
  return h > 0 ? `${h}:${mm}:${String(s).padStart(2, "0")}` : `${mm}:${String(s).padStart(2, "0")}`;
}

const ASPECTS = ["auto", "16:9", "4:3", "21:9", "1:1"] as const;
function nextAspect(current: string): string {
  const i = ASPECTS.indexOf(current as (typeof ASPECTS)[number]);
  return ASPECTS[(i + 1) % ASPECTS.length];
}
function aspectLabel(a: string): string {
  return a === "auto" ? "Auto" : a;
}

function trackLabel(tr: { language: string | null; title: string | null; id: number }, t: (k: string) => string): string {
  if (tr.title && tr.language) return `${tr.title} (${tr.language})`;
  if (tr.title) return tr.title;
  if (tr.language) return tr.language;
  return `${t("player.track")} ${tr.id}`;
}
