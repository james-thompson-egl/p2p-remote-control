//! P2P Remote Control - Secure, serverless, portable remote desktop
//!
//! Features:
//! - No server required (manual SDP exchange via copy-paste/QR)
//! - End-to-end encrypted (WebRTC DTLS-SRTP)
//! - No admin required (user-mode DXGI + SendInput)
//! - Portable single binary

mod screen_capture;
#[cfg(feature = "dxgi")]
mod dxgi_capture;
mod input_injector;
pub mod pairing;
mod webrtc_conn;
mod video_encoder;
mod client_input;

use tracing::{info, error, warn};
use tracing_subscriber::FmtSubscriber;
use std::io::BufRead;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Setup logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .init();

    info!("P2P Remote Control starting...");

    // Parse command line
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        print_usage();
        return Ok(());
    }

    match args[1].as_str() {
        "--host" => {
            // Host mode: wait for connections
            host_mode().await?;
        }
        "--connect" => {
            // Client mode: connect to a host
            if args.len() < 3 {
                eprintln!("Usage: {} --connect <pairing-code>", args[0]);
                return Ok(());
            }
            client_mode(&args[2]).await?;
        }
        "--help" | "-h" => {
            print_usage();
        }
        _ => {
            eprintln!("Unknown option: {}", args[1]);
            print_usage();
        }
    }

    Ok(())
}

fn print_usage() {
    println!(
        r#"
P2P Remote Control - Secure, serverless remote desktop

Usage:
  {} --host              Start as host (wait for incoming connection)
  {} --connect <code>    Connect to a host using pairing code

Security:
  - End-to-end encrypted via WebRTC DTLS
  - No server required - manual SDP exchange
  - No admin rights needed
  - Single portable binary

For setup instructions, see README.md
"#,
        std::env::args().next().unwrap_or_else(|| "p2p-remote".to_string()),
        std::env::args().next().unwrap_or_else(|| "p2p-remote".to_string())
    );
}

async fn host_mode() -> Result<(), Box<dyn std::error::Error>> {
    info!("Running in HOST mode - waiting for connection");

    // Generate pairing code
    let pairing_code = pairing::generate_pairing_code();
    info!("Pairing code: {}", pairing_code);

    // Display QR code for easy pairing
    if let Ok(qr) = pairing::generate_qr_code(&pairing_code) {
        println!("\n=== PAIRING QR CODE ===");
        println!("{}", qr);
        println!("======================\n");
    }

    println!("Share this pairing code with the viewer: {}", pairing_code);
    println!("Press Ctrl+C to exit\n");

    // Initialize screen capture
    let screen_capturer = screen_capture::ScreenCapturer::new()?;
    info!("Screen capture initialized");

    // Initialize input injector
    let input_injector = input_injector::InputInjector::new();
    info!("Input injector ready");

    // Start WebRTC host
    webrtc_conn::run_host(pairing_code, screen_capturer, input_injector).await?;

    Ok(())
}

async fn client_mode(pairing_code: &str) -> Result<(), Box<dyn std::error::Error>> {
    info!("Running in CLIENT mode - connecting with code: {}", pairing_code);

    // Validate pairing code format
    if !pairing::validate_pairing_code(pairing_code) {
        error!("Invalid pairing code format");
        return Err("Invalid pairing code".into());
    }

    // Initialize viewer UI (for now, simple terminal-based)
    println!("Enter SDP offer from host (paste and press Enter twice when done):");
    let mut sdp_offer = String::new();
    let mut lines = std::io::stdin().lock().lines();

    loop {
        match lines.next() {
            Some(Ok(line)) => {
                if line.is_empty() && sdp_offer.contains("v=") {
                    break;
                }
                sdp_offer.push_str(&line);
                sdp_offer.push('\n');
            }
            Some(Err(e)) => {
                error!("Failed to read SDP: {}", e);
                return Err(e.into());
            }
            None => break,
        }
    }

    info!("SDP offer received, generating answer...");

    // Initialize WebRTC client
    webrtc_conn::run_client(pairing_code, &sdp_offer).await?;

    Ok(())
}
