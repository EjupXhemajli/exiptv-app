use exiptv_core::db::Database;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::Manager;

pub struct AppState {
    pub db: Mutex<Database>,
    pub http: reqwest::Client,
    /// App-Datenverzeichnis (für DB, Logs, Runtime-DLL, Cache).
    pub app_data_dir: PathBuf,
}

impl AppState {
    pub fn init(app: &tauri::AppHandle) -> Result<Self, Box<dyn std::error::Error>> {
        let data_dir = app.path().app_data_dir()?;
        std::fs::create_dir_all(&data_dir)?;
        let db = Database::open(&data_dir.join("exiptv.db"))?;
        let http = crate::http::build_client()?;
        Ok(Self { db: Mutex::new(db), http, app_data_dir: data_dir })
    }
}
