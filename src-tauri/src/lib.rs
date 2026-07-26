//! EXIPTV Tauri-Shell.
//!
//! Verantwortlich für: Fenster, IPC-Commands, Logging (rotierend, maskiert),
//! Datenbank-Lebenszyklus, sichere Zugangsdaten (Windows Credential Manager
//! über `keyring`), HTTP-Import und die libmpv-Wiedergabe (Phase 4).

mod commands;
mod http;
mod m3u_split;
mod mpv_loader;
mod playback;
mod playback_commands;
mod playback_controller;
mod secrets;
mod state;
mod xtream_import;

use playback_commands::PlaybackHandle;
use state::AppState;
use tauri::Manager;

/// Ergebnistyp für Tauri-Commands: Ok-Wert oder nutzerverständliche Meldung.
pub type CmdResult<T> = Result<T, String>;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(PlaybackHandle::default())
        .setup(|app| {
            init_logging(app)?;
            let state = AppState::init(app.handle())?;
            app.manage(state);
            tracing::info!("EXIPTV gestartet");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_providers,
            commands::add_provider,
            commands::delete_provider,
            commands::set_provider_enabled,
            commands::rename_provider,
            commands::count_channels,
            commands::import_m3u_from_file,
            commands::import_m3u_from_url,
            commands::list_groups,
            commands::list_channels,
            commands::search_channels,
            commands::get_setting,
            commands::set_setting,
            commands::app_diagnostics,
            // VOD (Phase 7-UI, gefüllt durch Phase 5)
            commands::movie_categories,
            commands::list_movies,
            commands::series_categories,
            commands::list_series,
            commands::list_seasons,
            commands::list_episodes,
            commands::count_movies,
            commands::count_series,
            commands::import_xtream,
            commands::load_series_seasons,
            commands::add_favorite,
            commands::remove_favorite,
            commands::is_favorite,
            commands::favorite_channels,
            commands::favorite_channel_ids,
            commands::add_history,
            commands::list_history,
            commands::clear_history,
            commands::read_log,
            commands::clear_image_cache,
            commands::cache_image,
            commands::quit_app,
            commands::fetch_news,
            commands::fetch_article_image,
            // Playback (Phase 4)
            playback_commands::ensure_playback_ready,
            playback_commands::playback_load,
            playback_commands::playback_toggle_pause,
            playback_commands::playback_stop,
            playback_commands::playback_seek,
            playback_commands::playback_seek_relative,
            playback_commands::playback_set_volume,
            playback_commands::playback_select_audio,
            playback_commands::playback_select_subtitle,
            playback_commands::playback_set_aspect,
            playback_commands::playback_set_deinterlace,
            playback_commands::playback_set_bounds,
            playback_commands::playback_show_video,
            playback_commands::playback_request_tracks,
        ])
        .run(tauri::generate_context!())
        .expect("EXIPTV konnte nicht gestartet werden");
}

fn init_logging(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let log_dir = app.path().app_log_dir()?;
    std::fs::create_dir_all(&log_dir)?;
    let appender = tracing_appender::rolling::daily(&log_dir, "exiptv.log");
    let (writer, guard) = tracing_appender::non_blocking(appender);
    Box::leak(Box::new(guard));
    tracing_subscriber::fmt()
        .with_writer(writer)
        .with_ansi(false)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();
    prune_old_logs(&log_dir, 14);
    Ok(())
}

fn prune_old_logs(dir: &std::path::Path, keep_days: u64) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let cutoff = std::time::SystemTime::now()
        - std::time::Duration::from_secs(keep_days * 24 * 3600);
    for e in entries.flatten() {
        if let Ok(meta) = e.metadata() {
            if meta.is_file() && meta.modified().map(|m| m < cutoff).unwrap_or(false) {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
}
