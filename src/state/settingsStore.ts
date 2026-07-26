/**
 * Zentraler Einstellungs-Store (zustand).
 *
 * Lädt Einstellungen beim Start aus dem Backend (persistiert in SQLite über
 * `set_setting`) und stellt sie global bereit. Player-relevante Werte
 * (Puffer, Qualität, Deinterlace) werden beim Start der Wiedergabe an mpv
 * übergeben; visuelle Werte (Hintergrundfarbe) wirken sofort auf die
 * CSS-Variablen.
 */
import { create } from "zustand";
import { backend } from "../lib/backend";

export type QualityMode = "auto" | "high" | "medium" | "low";
export type BufferMode = "klein" | "normal" | "gross";
export type AccentTheme = "violett-cyan" | "magenta-blau" | "blau-cyan" | "gruen";
export type StartView = "home" | "livetv" | "movies" | "series" | "favorites";
export type PlaybackWay = "direct" | "auto-convert";

export interface Settings {
  // Player / Wiedergabe
  bufferMode: BufferMode;         // Netzwerkpuffer (Latenz vs. Stabilität)
  fineBufferSeconds: number;      // Feinpuffer in Sekunden (0–30)
  audioDelayMs: number;           // Ton-Versatz in ms (-2000..2000)
  quality: QualityMode;           // bevorzugte Bildqualität (ABR-Grenze)
  hardwareDecoding: boolean;      // HW-Decoding an/aus
  deinterlace: boolean;
  reconnect: boolean;             // automatische Wiederverbindung
  autoNextEpisode: boolean;       // nächste Folge automatisch starten
  autoBuffer: boolean;            // Puffer automatisch anpassen
  playbackWay: PlaybackWay;       // Direktwiedergabe vs. Umwandeln
  imageEnhancement: boolean;      // Bildverbesserung
  volumeNormalization: boolean;   // Lautstärke-Normalisierung
  preferredAudioLang: string;     // z. B. "deu", "eng"
  preferredSubtitleLang: string;
  // Programm-Guide (EPG)
  epgOffsetHours: number;         // Zeitversatz in Stunden (-12..12)
  // Start
  startSound: boolean;            // Startsound abspielen
  startView: StartView;           // Startansicht
  // Oberfläche
  accentTheme: AccentTheme;       // Akzentfarben-Schema
  showChannelNumbers: boolean;    // Sendernummern anzeigen
  backgroundColor: string;        // Haupt-Hintergrundfarbe (#rrggbb)
  reducedMotion: boolean;
  // Sprache
  language: string;               // "de" | "en"
}

const DEFAULTS: Settings = {
  bufferMode: "normal",
  fineBufferSeconds: 9,
  audioDelayMs: 0,
  quality: "auto",
  hardwareDecoding: true,
  deinterlace: false,
  reconnect: true,
  autoNextEpisode: true,
  autoBuffer: true,
  playbackWay: "direct",
  imageEnhancement: false,
  volumeNormalization: true,
  preferredAudioLang: "",
  preferredSubtitleLang: "",
  epgOffsetHours: 0,
  startSound: false,
  startView: "home",
  accentTheme: "violett-cyan",
  showChannelNumbers: true,
  backgroundColor: "#060a18",
  reducedMotion: false,
  language: "de",
};

interface SettingsState {
  settings: Settings;
  loaded: boolean;
  load: () => Promise<void>;
  update: <K extends keyof Settings>(key: K, value: Settings[K]) => Promise<void>;
}

// Schlüssel-Präfix in der Settings-Tabelle.
const KEY = (k: string) => `pref.${k}`;

function applyBackground(color: string) {
  // Wirkt auf die zentrale Design-Token-Variable.
  document.documentElement.style.setProperty("--bg-0", color);
}

/** Akzentfarben-Schema auf die Design-Tokens anwenden. */
function applyAccent(theme: AccentTheme) {
  const map: Record<AccentTheme, { primary: string; accent: string; ring: string }> = {
    "violett-cyan": { primary: "#8b5cf6", accent: "#29c2f6", ring: "linear-gradient(120deg, #d946ef, #8b5cf6 45%, #29c2f6)" },
    "magenta-blau": { primary: "#d946ef", accent: "#2d7df6", ring: "linear-gradient(120deg, #d946ef, #a855f7 45%, #2d7df6)" },
    "blau-cyan": { primary: "#2d7df6", accent: "#29c2f6", ring: "linear-gradient(120deg, #2d7df6, #29c2f6)" },
    "gruen": { primary: "#10b981", accent: "#34d399", ring: "linear-gradient(120deg, #059669, #10b981 45%, #34d399)" },
  };
  const c = map[theme] ?? map["violett-cyan"];
  const root = document.documentElement.style;
  root.setProperty("--primary", c.primary);
  root.setProperty("--violet", c.primary);
  root.setProperty("--accent", c.accent);
  root.setProperty("--cyan", c.accent);
  root.setProperty("--ring-gradient", c.ring);
}

export const useSettings = create<SettingsState>((set, get) => ({
  settings: DEFAULTS,
  loaded: false,
  load: async () => {
    const entries = await Promise.all(
      (Object.keys(DEFAULTS) as (keyof Settings)[]).map(async (k) => {
        const v = await backend.getSetting(KEY(k)).catch(() => null);
        return [k, v] as const;
      })
    );
    const next: Settings = { ...DEFAULTS };
    for (const [k, v] of entries) {
      if (v == null) continue;
      const def = DEFAULTS[k];
      if (typeof def === "boolean") {
        (next[k] as boolean) = v === "1" || v === "true";
      } else if (typeof def === "number") {
        const n = Number(v);
        if (!Number.isNaN(n)) (next[k] as number) = n;
      } else {
        (next[k] as string) = v;
      }
    }
    applyBackground(next.backgroundColor);
    applyAccent(next.accentTheme);
    if (next.reducedMotion) document.documentElement.classList.add("reduced-motion");
    set({ settings: next, loaded: true });
  },
  update: async (key, value) => {
    const next = { ...get().settings, [key]: value };
    set({ settings: next });
    const stored =
      typeof value === "boolean" ? (value ? "1" : "0") : String(value);
    await backend.setSetting(KEY(String(key)), stored);
    if (key === "backgroundColor") applyBackground(String(value));
    if (key === "accentTheme") applyAccent(value as AccentTheme);
    if (key === "reducedMotion") {
      document.documentElement.classList.toggle("reduced-motion", Boolean(value));
    }
  },
}));

/** Puffer-Modus → mpv-Parameter (Sekunden Readahead). */
export function bufferSeconds(mode: BufferMode): number {
  switch (mode) {
    case "klein": return 6;
    case "gross": return 40;
    default: return 20;
  }
}
