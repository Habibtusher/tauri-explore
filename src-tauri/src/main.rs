// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[tauri::command]
fn process_text(text: String) -> String {
    if text.is_empty() {
        String::new()
    } else {
        format!("Hello, {}!", text)
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![process_text])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn main() {
    run();
}