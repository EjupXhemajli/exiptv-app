// EXIPTV – Einstiegspunkt (Windows: kein Konsolenfenster im Release).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    exiptv_lib::run()
}
