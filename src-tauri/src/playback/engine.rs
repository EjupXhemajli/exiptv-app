//! `MpvEngine`: konkrete libmpv-Implementierung des PlaybackEngine-Traits.

use exiptv_core::playback::{
    PlaybackEngine, PlaybackError, PlaybackState, PlaybackStatistics, TrackInfo, TrackKind,
};
use libmpv2::Mpv;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

/// Wrappt eine libmpv-Instanz. mpv rendert in das per `wid` übergebene
/// Kindfenster.
pub struct MpvEngine {
    mpv: Mpv,
    state: Arc<AtomicU8>, // gespiegelter PlaybackState (siehe state_from_u8)
    current_url: Option<String>,
    current_headers: Vec<(String, String)>,
}

fn state_to_u8(s: PlaybackState) -> u8 {
    match s {
        PlaybackState::Idle => 0,
        PlaybackState::Loading => 1,
        PlaybackState::Playing => 2,
        PlaybackState::Paused => 3,
        PlaybackState::Buffering => 4,
        PlaybackState::Ended => 5,
        PlaybackState::Error => 6,
    }
}
fn state_from_u8(v: u8) -> PlaybackState {
    match v {
        1 => PlaybackState::Loading,
        2 => PlaybackState::Playing,
        3 => PlaybackState::Paused,
        4 => PlaybackState::Buffering,
        5 => PlaybackState::Ended,
        6 => PlaybackState::Error,
        _ => PlaybackState::Idle,
    }
}

impl MpvEngine {
    /// Erzeugt eine mpv-Instanz, die in `wid` (natives Fenster-Handle) rendert.
    ///
    /// Voraussetzung: `libmpv-2.dll` ist über den Prozess-PATH auffindbar
    /// (siehe `mpv_loader::ensure_on_path`).
    pub fn new(wid: i64, config: &crate::playback_controller::PlaybackConfig) -> Result<Self, PlaybackError> {
        let cfg = config.clone();
        let mpv = Mpv::with_initializer(|init| {
            // Einbettung: mpv zeichnet in unser Kindfenster.
            let _ = init.set_property("wid", wid);

            // Hardwarebeschleunigung (auto-safe fällt bei Problemen selbst
            // auf Software-Decoding zurück) – abschaltbar über Einstellungen.
            if cfg.hardware_decoding {
                let _ = init.set_property("hwdec", "auto-safe");
            } else {
                let _ = init.set_property("hwdec", "no");
            }
            // "gpu" ist der breit unterstützte Video-Output.
            let _ = init.set_property("vo", "gpu");

            // Netzwerkpuffer aus den Einstellungen (Latenz vs. Stabilität).
            let _ = init.set_property("cache", "yes");
            let _ = init.set_property("demuxer-max-bytes", "96MiB");
            let _ = init.set_property("demuxer-max-back-bytes", "48MiB");
            let _ = init.set_property("demuxer-readahead-secs", cfg.buffer_seconds.to_string());

            // Bildqualität als ABR-Obergrenze (bei HLS-Master-Playlists).
            match cfg.quality.as_str() {
                "high"   => { let _ = init.set_property("hls-bitrate", "max"); }
                "medium" => { let _ = init.set_property("hls-bitrate", "no"); }
                "low"    => { let _ = init.set_property("hls-bitrate", "min"); }
                _        => { /* auto: mpv entscheidet */ }
            }

            // Deinterlacing.
            let _ = init.set_property("deinterlace", if cfg.deinterlace { "yes" } else { "no" });

            // Bevorzugte Sprachen (falls gesetzt).
            if !cfg.preferred_audio_lang.is_empty() {
                let _ = init.set_property("alang", cfg.preferred_audio_lang.clone());
            }
            if !cfg.preferred_subtitle_lang.is_empty() {
                let _ = init.set_property("slang", cfg.preferred_subtitle_lang.clone());
            }

            // Robustheit bei instabilen Netzwerkstreams.
            let _ = init.set_property("keep-open", "yes");
            let _ = init.set_property("network-timeout", "20");
            if cfg.reconnect {
                let _ = init.set_property("stream-lavf-o", "reconnect=1,reconnect_streamed=1,reconnect_delay_max=5");
            }
            // Kein OSD/Tastatur/Maus von mpv selbst – wir steuern über die UI.
            // Damit gehen ALLE Eingaben ans WebView (unsere Steuerleiste), und
            // mpv blockiert nichts.
            let _ = init.set_property("input-default-bindings", "no");
            let _ = init.set_property("input-vo-keyboard", "no");
            let _ = init.set_property("input-cursor", "no");
            let _ = init.set_property("input-media-keys", "no");
            let _ = init.set_property("cursor-autohide", "no");
            let _ = init.set_property("osc", "no");
            let _ = init.set_property("input-builtin-bindings", "no");
            // Lautstärke-Normalisierung (dynamischer Kompressor) optional.
            if cfg.volume_normalization {
                let _ = init.set_property("af", "dynaudnorm=g=5:f=250:r=0.9:p=0.5");
            }
            // Ton-Versatz (Sekunden; positiv = Ton später).
            if cfg.audio_delay_ms != 0 {
                let secs = cfg.audio_delay_ms as f64 / 1000.0;
                let _ = init.set_property("audio-delay", &secs.to_string());
            }
            // Terminalausgabe unterdrücken.
            let _ = init.set_property("terminal", "no");
            let _ = init.set_property("msg-level", "all=no");
            Ok(())
        })
        .map_err(|e| PlaybackError::Engine(format!("mpv-Initialisierung fehlgeschlagen: {e}")))?;

        Ok(Self {
            mpv,
            state: Arc::new(AtomicU8::new(state_to_u8(PlaybackState::Idle))),
            current_url: None,
            current_headers: Vec::new(),
        })
    }

    fn set_state(&self, s: PlaybackState) {
        self.state.store(state_to_u8(s), Ordering::Relaxed);
    }

    /// Header (User-Agent/Referer/Cookies) als mpv-Optionen setzen.
    fn apply_headers(&self, headers: &[(String, String)]) {
        let mut user_agent: Option<&str> = None;
        let mut referer: Option<&str> = None;
        let mut extra: Vec<String> = Vec::new();
        for (k, v) in headers {
            match k.to_ascii_lowercase().as_str() {
                "user-agent" => user_agent = Some(v),
                "referer" | "referrer" => referer = Some(v),
                _ => extra.push(format!("{k}: {v}")),
            }
        }
        if let Some(ua) = user_agent {
            let _ = self.mpv.set_property("user-agent", ua);
        }
        if let Some(rf) = referer {
            let _ = self.mpv.set_property("referrer", rf);
        }
        if !extra.is_empty() {
            let _ = self.mpv.set_property("http-header-fields", extra.join(","));
        }
    }

    fn string_prop(&self, name: &str) -> Option<String> {
        self.mpv.get_property::<String>(name).ok().filter(|s| !s.is_empty())
    }
    fn i64_prop(&self, name: &str) -> Option<i64> {
        self.mpv.get_property::<i64>(name).ok()
    }
    fn f64_prop(&self, name: &str) -> Option<f64> {
        self.mpv.get_property::<f64>(name).ok()
    }
    fn flag_prop(&self, name: &str) -> Option<bool> {
        self.mpv.get_property::<bool>(name).ok()
    }
}

impl PlaybackEngine for MpvEngine {
    fn load(&mut self, url: &str, headers: &[(String, String)]) -> Result<(), PlaybackError> {
        self.set_state(PlaybackState::Loading);
        self.apply_headers(headers);
        self.current_url = Some(url.to_string());
        self.current_headers = headers.to_vec();
        // Datei laden und sofort abspielen.
        self.mpv
            .command("loadfile", &[url, "replace"])
            .map_err(|e| PlaybackError::Load(e.to_string()))?;
        let _ = self.mpv.set_property("pause", false);
        Ok(())
    }

    fn play(&mut self) -> Result<(), PlaybackError> {
        self.mpv.set_property("pause", false)
            .map_err(|e| PlaybackError::Engine(e.to_string()))?;
        self.set_state(PlaybackState::Playing);
        Ok(())
    }

    fn pause(&mut self) -> Result<(), PlaybackError> {
        self.mpv.set_property("pause", true)
            .map_err(|e| PlaybackError::Engine(e.to_string()))?;
        self.set_state(PlaybackState::Paused);
        Ok(())
    }

    fn stop(&mut self) -> Result<(), PlaybackError> {
        let _ = self.mpv.command("stop", &[]);
        self.set_state(PlaybackState::Idle);
        self.current_url = None;
        Ok(())
    }

    fn seek(&mut self, seconds: f64) -> Result<(), PlaybackError> {
        self.mpv
            .command("seek", &[&seconds.to_string(), "absolute"])
            .map_err(|e| PlaybackError::Engine(e.to_string()))
    }

    fn set_volume(&mut self, volume: u8) -> Result<(), PlaybackError> {
        self.mpv.set_property("volume", volume as i64)
            .map_err(|e| PlaybackError::Engine(e.to_string()))
    }

    fn select_audio_track(&mut self, id: i64) -> Result<(), PlaybackError> {
        self.mpv.set_property("aid", id)
            .map_err(|e| PlaybackError::Engine(e.to_string()))
    }

    fn select_subtitle_track(&mut self, id: Option<i64>) -> Result<(), PlaybackError> {
        match id {
            Some(i) => self.mpv.set_property("sid", i),
            None => self.mpv.set_property("sid", "no"),
        }
        .map_err(|e| PlaybackError::Engine(e.to_string()))
    }

    fn set_video_track(&mut self, id: i64) -> Result<(), PlaybackError> {
        self.mpv.set_property("vid", id)
            .map_err(|e| PlaybackError::Engine(e.to_string()))
    }

    fn set_aspect_ratio(&mut self, ratio: Option<&str>) -> Result<(), PlaybackError> {
        let value = ratio.unwrap_or("-1"); // -1 = automatisch
        self.mpv.set_property("video-aspect-override", value)
            .map_err(|e| PlaybackError::Engine(e.to_string()))
    }

    fn set_deinterlace(&mut self, on: bool) -> Result<(), PlaybackError> {
        self.mpv.set_property("deinterlace", if on { "yes" } else { "no" })
            .map_err(|e| PlaybackError::Engine(e.to_string()))
    }

    fn get_statistics(&self) -> PlaybackStatistics {
        PlaybackStatistics {
            video_codec: self.string_prop("video-codec"),
            audio_codec: self.string_prop("audio-codec"),
            width: self.i64_prop("dwidth").map(|v| v as u32),
            height: self.i64_prop("dheight").map(|v| v as u32),
            fps: self.f64_prop("estimated-vf-fps").or_else(|| self.f64_prop("container-fps")),
            dropped_frames: self.i64_prop("frame-drop-count").unwrap_or(0).max(0) as u64,
            bitrate_kbps: self.i64_prop("video-bitrate").map(|b| (b / 1000).max(0) as u64),
            buffer_seconds: self.f64_prop("demuxer-cache-duration"),
            hw_decoding: self.string_prop("hwdec-current")
                .map(|s| s != "no" && !s.is_empty())
                .unwrap_or(false),
            decoder: self.string_prop("hwdec-current").filter(|s| s != "no"),
        }
    }

    fn get_playback_position(&self) -> Option<f64> {
        self.f64_prop("time-pos")
    }

    fn state(&self) -> PlaybackState {
        // Live-Zustände aus mpv ableiten (Puffern/Pause), sonst gespiegelt.
        if self.flag_prop("paused-for-cache").unwrap_or(false) {
            return PlaybackState::Buffering;
        }
        if self.flag_prop("pause").unwrap_or(false) {
            return PlaybackState::Paused;
        }
        if self.flag_prop("eof-reached").unwrap_or(false) {
            return PlaybackState::Ended;
        }
        state_from_u8(self.state.load(Ordering::Relaxed))
    }

    fn tracks(&self) -> Vec<TrackInfo> {
        let mut out = Vec::new();
        let count = self.i64_prop("track-list/count").unwrap_or(0);
        for i in 0..count {
            let kind = match self.string_prop(&format!("track-list/{i}/type")).as_deref() {
                Some("video") => TrackKind::Video,
                Some("audio") => TrackKind::Audio,
                Some("sub") => TrackKind::Subtitle,
                _ => continue,
            };
            let id = self.i64_prop(&format!("track-list/{i}/id")).unwrap_or(0);
            let selected = self.flag_prop(&format!("track-list/{i}/selected")).unwrap_or(false);
            let language = self.string_prop(&format!("track-list/{i}/lang"));
            let title = self.string_prop(&format!("track-list/{i}/title"));
            out.push(TrackInfo { id, kind, language, title, selected });
        }
        out
    }

    fn reconnect(&mut self) -> Result<(), PlaybackError> {
        // Aktuelle URL erneut laden (gestufte Wiederherstellung, Stufe 2).
        let url = self.current_url.clone()
            .ok_or_else(|| PlaybackError::Engine("Keine aktive Wiedergabe.".into()))?;
        let headers = self.current_headers.clone();
        self.load(&url, &headers)
    }

    fn dispose(&mut self) {
        let _ = self.mpv.command("stop", &[]);
        self.set_state(PlaybackState::Idle);
        // Mpv wird beim Drop des Feldes freigegeben.
    }
}

impl MpvEngine {
    /// Nicht-blockierendes Abarbeiten anstehender mpv-Ereignisse.
    /// Wird periodisch aus dem Command-/Monitor-Layer gepumpt, um
    /// Property-Änderungen (Ende, Fehler) in den gespiegelten Zustand
    /// zu übernehmen.
    pub fn pump_events(&mut self) {
        // libmpv2 stellt Events über einen EventContext bereit; hier
        // beobachten wir minimal den Wiedergabe-Endzustand über Properties,
        // da der EventContext-Lebenszyklus an die Mpv-Instanz gebunden ist.
        if self.flag_prop("eof-reached").unwrap_or(false) {
            self.set_state(PlaybackState::Ended);
        }
    }

    /// Fortschrittsindikator für den StreamHealthMonitor: steigt bei
    /// gesundem Stream monoton. `estimated-frame-number` zählt die
    /// wiedergegebenen Frames; fällt sie aus, dient die Zeitposition als
    /// Ersatzsignal (auf Zehntelsekunden skaliert).
    pub fn decoded_frame_count(&self) -> i64 {
        if let Some(n) = self.i64_prop("estimated-frame-number") {
            return n;
        }
        self.f64_prop("time-pos").map(|t| (t * 10.0) as i64).unwrap_or(0)
    }

    /// Gesamtdauer (VOD); bei Live-Streams i. d. R. None.
    pub fn duration(&self) -> Option<f64> {
        self.f64_prop("duration").filter(|d| *d > 0.0)
    }

    /// Relatives Spulen um `delta` Sekunden (negativ = zurück).
    /// Für 15-Sekunden-Sprünge und minutenweises Spulen im VOD-Player.
    pub fn seek_relative(&self, delta: f64) -> Result<(), PlaybackError> {
        self.mpv
            .command("seek", &[&delta.to_string(), "relative"])
            .map_err(|e| PlaybackError::Engine(e.to_string()))
    }

    /// Schnelle Spuranzahl (nur eine Property) – für Änderungserkennung.
    pub fn track_count(&self) -> i64 {
        self.i64_prop("track-list/count").unwrap_or(0)
    }
}
