import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { backend } from "../lib/backend";
import { useSettings } from "../state/settingsStore";
import type { Diagnostics } from "../lib/types";
import type { BufferMode, QualityMode, AccentTheme, StartView, PlaybackWay } from "../state/settingsStore";

export default function Settings() {
  const { t, i18n } = useTranslation();
  const { settings, update } = useSettings();
  const [diag, setDiag] = useState<Diagnostics | null>(null);
  const [showLog, setShowLog] = useState(false);
  const [logText, setLogText] = useState<string>("");
  const [cacheMsg, setCacheMsg] = useState<string>("");

  useEffect(() => {
    backend.appDiagnostics().then(setDiag).catch(() => setDiag(null));
  }, []);

  const setLang = async (lang: string) => {
    await i18n.changeLanguage(lang);
    await update("language", lang);
  };

  const clearCache = async () => {
    setCacheMsg(t("settings.cacheClearing"));
    try {
      await backend.clearImageCache();
      setCacheMsg(t("settings.cacheCleared"));
    } catch {
      setCacheMsg(t("settings.cacheError"));
    }
    setTimeout(() => setCacheMsg(""), 4000);
  };

  const openLog = async () => {
    setShowLog(true);
    try {
      setLogText(await backend.readLog());
    } catch {
      setLogText(t("settings.logError"));
    }
  };

  return (
    <>
      <h1>{t("settings.title")}</h1>

      {/* Allgemein */}
      <section className="card settings-section">
        <h2>{t("settings.general")}</h2>
        <Row label={t("settings.language")}>
          <select value={i18n.language.startsWith("en") ? "en" : "de"} onChange={(e) => void setLang(e.target.value)}>
            <option value="de">Deutsch</option>
            <option value="en">English</option>
          </select>
        </Row>
      </section>

      {/* Wiedergabe */}
      <section className="card settings-section">
        <h2>{t("settings.playbackSection")}</h2>

        <Row label={t("settings.bufferMode")} hint={t("settings.bufferHint")}>
          <select value={settings.bufferMode} onChange={(e) => void update("bufferMode", e.target.value as BufferMode)}>
            <option value="klein">{t("settings.bufferSmall")}</option>
            <option value="normal">{t("settings.bufferNormal")}</option>
            <option value="gross">{t("settings.bufferLarge")}</option>
          </select>
        </Row>

        <Slider
          label={t("settings.fineBuffer")}
          value={settings.fineBufferSeconds}
          min={0} max={30} step={1} unit="s"
          onChange={(v) => void update("fineBufferSeconds", v)}
        />

        <Slider
          label={t("settings.audioDelay")}
          value={settings.audioDelayMs}
          min={-2000} max={2000} step={50} unit="ms"
          onChange={(v) => void update("audioDelayMs", v)}
        />

        <Toggle label={t("settings.autoNextEpisode")} checked={settings.autoNextEpisode} onChange={(v) => void update("autoNextEpisode", v)} />
        <Toggle label={t("settings.autoBuffer")} checked={settings.autoBuffer} onChange={(v) => void update("autoBuffer", v)} />

        <Row label={t("settings.playbackWay")} hint={t("settings.playbackWayHint")}>
          <select value={settings.playbackWay} onChange={(e) => void update("playbackWay", e.target.value as PlaybackWay)}>
            <option value="direct">{t("settings.playbackDirect")}</option>
            <option value="auto-convert">{t("settings.playbackConvert")}</option>
          </select>
        </Row>

        <Row label={t("settings.quality")} hint={t("settings.qualityHint")}>
          <select value={settings.quality} onChange={(e) => void update("quality", e.target.value as QualityMode)}>
            <option value="auto">{t("settings.qualityAuto")}</option>
            <option value="high">{t("settings.qualityHigh")}</option>
            <option value="medium">{t("settings.qualityMedium")}</option>
            <option value="low">{t("settings.qualityLow")}</option>
          </select>
        </Row>

        <Toggle label={t("settings.imageEnhancement")} checked={settings.imageEnhancement} onChange={(v) => void update("imageEnhancement", v)} />
        <Toggle label={t("settings.volumeNormalization")} checked={settings.volumeNormalization} onChange={(v) => void update("volumeNormalization", v)} />
        <Toggle label={t("settings.hardwareDecoding")} hint={t("settings.hardwareHint")} checked={settings.hardwareDecoding} onChange={(v) => void update("hardwareDecoding", v)} />
        <Toggle label={t("settings.reconnect")} hint={t("settings.reconnectHint")} checked={settings.reconnect} onChange={(v) => void update("reconnect", v)} />
        <Toggle label={t("settings.deinterlace")} checked={settings.deinterlace} onChange={(v) => void update("deinterlace", v)} />

        <Row label={t("settings.audioLang")} hint={t("settings.langHint")}>
          <input style={{ width: 160 }} value={settings.preferredAudioLang} placeholder="deu, eng" onChange={(e) => void update("preferredAudioLang", e.target.value)} />
        </Row>
        <Row label={t("settings.subtitleLang")}>
          <input style={{ width: 160 }} value={settings.preferredSubtitleLang} placeholder="deu, eng" onChange={(e) => void update("preferredSubtitleLang", e.target.value)} />
        </Row>
        <p className="faint">{t("settings.playerRestartHint")}</p>
      </section>

      {/* Programm-Guide (EPG) */}
      <section className="card settings-section">
        <h2>{t("settings.epgSection")}</h2>
        <Slider
          label={t("settings.epgOffset")}
          value={settings.epgOffsetHours}
          min={-12} max={12} step={1} unit="h"
          onChange={(v) => void update("epgOffsetHours", v)}
        />
      </section>

      {/* Start */}
      <section className="card settings-section">
        <h2>{t("settings.startSection")}</h2>
        <Toggle label={t("settings.startSound")} checked={settings.startSound} onChange={(v) => void update("startSound", v)} />
        <Row label={t("settings.startView")}>
          <select value={settings.startView} onChange={(e) => void update("startView", e.target.value as StartView)}>
            <option value="home">{t("nav.home")}</option>
            <option value="livetv">{t("nav.live")}</option>
            <option value="movies">{t("nav.movies")}</option>
            <option value="series">{t("nav.series")}</option>
            <option value="favorites">{t("nav.favorites")}</option>
          </select>
        </Row>
      </section>

      {/* Oberfläche */}
      <section className="card settings-section">
        <h2>{t("settings.appearance")}</h2>
        <Row label={t("settings.accentColor")}>
          <select value={settings.accentTheme} onChange={(e) => void update("accentTheme", e.target.value as AccentTheme)}>
            <option value="violett-cyan">{t("settings.accentVioletCyan")}</option>
            <option value="magenta-blau">{t("settings.accentMagentaBlue")}</option>
            <option value="blau-cyan">{t("settings.accentBlueCyan")}</option>
            <option value="gruen">{t("settings.accentGreen")}</option>
          </select>
        </Row>
        <Toggle label={t("settings.showChannelNumbers")} checked={settings.showChannelNumbers} onChange={(v) => void update("showChannelNumbers", v)} />
        <Row label={t("settings.backgroundColor")} hint={t("settings.backgroundHint")}>
          <div className="row" style={{ gap: 10 }}>
            <input type="color" value={settings.backgroundColor} onChange={(e) => void update("backgroundColor", e.target.value)} style={{ width: 48, height: 34, padding: 2, cursor: "pointer" }} />
            <button onClick={() => void update("backgroundColor", "#060a18")}>{t("settings.resetDefault")}</button>
          </div>
        </Row>
        <Toggle label={t("settings.reducedMotion")} checked={settings.reducedMotion} onChange={(v) => void update("reducedMotion", v)} />
      </section>

      {/* Wartung */}
      <section className="card settings-section">
        <h2>{t("settings.maintenance")}</h2>
        <Row label={t("settings.listCache")} hint={t("settings.listCacheHint")}>
          <div className="row" style={{ gap: 10 }}>
            <button onClick={() => void clearCache()}>{t("settings.clearCache")}</button>
            {cacheMsg && <span className="faint">{cacheMsg}</span>}
          </div>
        </Row>
        <Row label={t("settings.diagnostics")}>
          <button onClick={() => void openLog()}>{t("settings.showLog")}</button>
        </Row>
      </section>

      {/* Info / Diagnose */}
      <section className="card settings-section">
        <h2>{t("settings.info")}</h2>
        {!diag && <div className="skeleton" style={{ height: 72 }} />}
        {diag && (
          <dl className="diag-grid">
            <dt className="dim">{t("settings.appVersion")}</dt><dd>{diag.app_version}</dd>
            <dt className="dim">{t("settings.os")}</dt><dd>{diag.os} ({diag.arch})</dd>
            <dt className="dim">{t("settings.dbVersion")}</dt><dd>v{diag.db_schema_version}</dd>
          </dl>
        )}
      </section>

      {/* Log-Overlay */}
      {showLog && (
        <div className="log-overlay" onClick={() => setShowLog(false)}>
          <div className="log-panel" onClick={(e) => e.stopPropagation()}>
            <div className="log-head">
              <span>{t("settings.showLog")}</span>
              <button className="icon-btn" onClick={() => setShowLog(false)}>✕</button>
            </div>
            <pre className="log-body">{logText || t("settings.logEmpty")}</pre>
          </div>
        </div>
      )}
    </>
  );
}

function Row({ label, hint, children }: { label: string; hint?: string; children: React.ReactNode }) {
  return (
    <div className="settings-row">
      <div className="settings-label">
        <span>{label}</span>
        {hint && <span className="faint">{hint}</span>}
      </div>
      <div>{children}</div>
    </div>
  );
}

function Toggle({ label, hint, checked, onChange }: {
  label: string; hint?: string; checked: boolean; onChange: (v: boolean) => void;
}) {
  return (
    <div className="settings-row">
      <div className="settings-label">
        <span>{label}</span>
        {hint && <span className="faint">{hint}</span>}
      </div>
      <button className={`switch ${checked ? "on" : ""}`} role="switch" aria-checked={checked} aria-label={label} onClick={() => onChange(!checked)}>
        <span className="knob" />
      </button>
    </div>
  );
}

function Slider({ label, value, min, max, step, unit, onChange }: {
  label: string; value: number; min: number; max: number; step: number; unit: string; onChange: (v: number) => void;
}) {
  return (
    <div className="settings-row">
      <div className="settings-label"><span>{label}</span></div>
      <div className="row" style={{ gap: 12, minWidth: 240 }}>
        <input type="range" min={min} max={max} step={step} value={value}
          onChange={(e) => onChange(Number(e.target.value))}
          style={{ flex: 1, accentColor: "var(--violet)" }} />
        <span className="slider-value">{value} {unit}</span>
      </div>
    </div>
  );
}
