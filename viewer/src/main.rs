//! P2P Remote Viewer - Tauri GUI application
//!
//! Provides a graphical interface for:
//! - Entering pairing codes
//! - Pasting SDP offers/answers
//! - Displaying remote desktop
//! - Sending input events

use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use std::sync::Arc;
use tokio::sync::mpsc;

mod webrtc_client;
mod input_handler;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct PairingState {
    code: String,
    connected: bool,
}

#[tauri::command]
async fn start_host(pairing_code: String) -> Result<String, String> {
    // Start host mode - generate SDP offer
    webrtc_client::run_host(&pairing_code).await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn start_client(pairing_code: String, sdp_offer: String) -> Result<String, String> {
    // Start client mode - generate SDP answer
    webrtc_client::run_client(&pairing_code, &sdp_offer).await
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn set_fullscreen(app: AppHandle, enabled: bool) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.set_fullscreen(enabled);
    }
}

#[tauri::command]
fn send_input(app: AppHandle, input_type: String, x: Option<i32>, y: Option<i32>) {
    // Send input to remote host
    // In production, this would use the WebRTC data channel
    println!("Input: {} at ({:?}, {:?})", input_type, x, y);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            start_host,
            start_client,
            set_fullscreen,
            send_input
        ])
        .setup(|app| {
            // Create main window
            let window = WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
                .build()
                .map_err(|e| e.to_string())?;

            info!("P2P Remote Viewer started");
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
