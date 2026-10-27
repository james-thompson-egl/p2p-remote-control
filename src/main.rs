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
pub mod cert;
pub mod signaling;
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

    // Parse common options
    let auth_type = parse_auth_type(&args);
    let relay_url = parse_relay(&args);

    match args[1].as_str() {
        "--host" => {
            // Host mode: wait for connections
            host_mode(auth_type, relay_url).await?;
        }
        "--connect" => {
            if let Some(url) = relay_url {
                // Relay mode: the host's one-time token is both the rendezvous key
                // and the pairing secret. Take it from --token, or prompt for it.
                let token = match parse_token(&args) {
                    Some(t) => t,
                    None => prompt_line("Enter the connection token from the host: ")?,
                };
                if token.is_empty() {
                    eprintln!("A connection token is required in relay mode.");
                    return Ok(());
                }
                client_mode_relay(url, token).await?;
            } else {
                // Client mode: connect to a host via manual SDP copy-paste
                let auth_value = if args.len() > 2 { &args[2] } else { "" };

                // Check if auth_value is a pairing code or "cert"
                let auth = if auth_value == "cert" || auth_type == webrtc_conn::AuthType::Cert {
                    webrtc_conn::AuthType::Cert
                } else if validate_auth_arg(auth_value) {
                    webrtc_conn::AuthType::Pairing(auth_value.to_string())
                } else {
                    eprintln!("Usage: {} --connect <pairing-code> | cert [--relay <url>]", args[0]);
                    return Ok(());
                };

                client_mode(auth).await?;
            }
        }
        "--gen-cert" => {
            // Generate certificate for authentication
            let validity = parse_validity(&args[2..]);
            cert::generate_client_cert(validity)?;
            println!("\nCertificate generated successfully!");
            println!("Valid for {} days", validity);
            println!("\nTo use with connection:");
            println!("  p2p-remote --connect <pairing-code> --auth cert");
        }
        "--list-certs" => {
            // List certificates
            let certs = cert::list_certs()?;
            if certs.is_empty() {
                println!("No certificates found");
            } else {
                println!("Certificates:");
                for c in certs {
                    println!("  - {}", c);
                }
            }
        }
        "--show-cert" => {
            // Show certificate info
            let (cert, _) = cert::load_client_cert()?;
            println!("{}", cert::get_cert_info(&cert)?);
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
  {} --gen-cert [days]   Generate client certificate (default: 30 days)
  {} --list-certs        List available certificates
  {} --show-cert         Show current certificate info

Signalling relay (optional — removes manual SDP copy-paste):
  --relay <url>          Exchange SDP via a relay instead of copy-paste.
                         Host prints a one-time token; run the viewer with
                         --relay <url> and enter (or pass --token) that token.
  --token <token>        Viewer: the host's one-time token (else prompted)

Authentication:
  --auth <type>          Auth type: "pairing" (default) or "cert"
  --validity <days>      Certificate validity in days (for --gen-cert)

Security:
  - End-to-end encrypted via WebRTC DTLS
  - No server required - manual SDP exchange
  - Certificate-based auth for persistent access
  - No admin rights needed
  - Single portable binary

For setup instructions, see README.md
"#,
        std::env::args().next().unwrap_or_else(|| "p2p-remote".to_string()),
        std::env::args().next().unwrap_or_else(|| "p2p-remote".to_string()),
        std::env::args().next().unwrap_or_else(|| "p2p-remote".to_string()),
        std::env::args().next().unwrap_or_else(|| "p2p-remote".to_string()),
        std::env::args().next().unwrap_or_else(|| "p2p-remote".to_string())
    );
}

fn parse_validity(args: &[String]) -> u64 {
    if args.len() > 1 {
        if let Ok(days) = args[1].parse() {
            return days;
        }
    }
    30 // Default 30 days
}

fn parse_auth_type(args: &[String]) -> webrtc_conn::AuthType {
    // Look for --auth flag
    for i in 0..args.len() {
        if args[i] == "--auth" && i + 1 < args.len() {
            return match args[i + 1].as_str() {
                "cert" => webrtc_conn::AuthType::Cert,
                "pairing" => webrtc_conn::AuthType::Pairing("000000".to_string()), // Placeholder
                _ => webrtc_conn::AuthType::Pairing("000000".to_string()),
            };
        }
    }
    // Default to pairing (placeholder, will be set by --connect arg)
    webrtc_conn::AuthType::Pairing("000000".to_string())
}

fn validate_auth_arg(arg: &str) -> bool {
    pairing::validate_pairing_code(arg) || arg == "cert"
}

async fn host_mode(
    auth_type: webrtc_conn::AuthType,
    relay_url: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Running in HOST mode - waiting for connection");

    // Initialize screen capture
    let screen_capturer = screen_capture::ScreenCapturer::new()?;
    info!("Screen capture initialized");

    // Initialize input injector
    let input_injector = input_injector::InputInjector::new();
    info!("Input injector ready");

    // In relay mode the host mints a one-time token; it doubles as the pairing
    // secret, so the auth type is overridden to use it.
    let relay = relay_url.map(|url| signaling::Relay::new(&url, signaling::generate_token()));
    let auth_type = match &relay {
        Some(r) => webrtc_conn::AuthType::Pairing(r.token.clone()),
        None => auth_type,
    };

    // Start WebRTC host
    webrtc_conn::run_host(auth_type, screen_capturer, input_injector, relay).await?;

    Ok(())
}

/// Viewer side of relay mode: fetch the host's offer from the relay, answer it.
async fn client_mode_relay(url: String, token: String) -> Result<(), Box<dyn std::error::Error>> {
    info!("Connecting via relay...");
    let relay = signaling::Relay::new(&url, token);
    let offer = relay.poll_offer(std::time::Duration::from_secs(120)).await?;
    info!("Received offer via relay, generating answer...");
    let auth = webrtc_conn::AuthType::Pairing(relay.token.clone());
    webrtc_conn::run_client(auth, &offer, Some(relay)).await?;
    Ok(())
}

fn parse_relay(args: &[String]) -> Option<String> {
    args.iter().position(|a| a == "--relay").and_then(|i| args.get(i + 1)).map(|s| s.to_string())
}

fn parse_token(args: &[String]) -> Option<String> {
    args.iter().position(|a| a == "--token").and_then(|i| args.get(i + 1)).map(|s| s.to_string())
}

fn prompt_line(prompt: &str) -> Result<String, Box<dyn std::error::Error>> {
    use std::io::Write;
    print!("{}", prompt);
    std::io::stdout().flush()?;
    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;
    Ok(line.trim().to_string())
}

async fn client_mode(auth: webrtc_conn::AuthType) -> Result<(), Box<dyn std::error::Error>> {
    // For certificate auth, verify cert exists
    if let webrtc_conn::AuthType::Cert = &auth {
        info!("Certificate authentication mode");
        match cert::load_client_cert() {
            Ok(_) => {
                info!("Client certificate loaded");
            }
            Err(_) => {
                error!("No client certificate found. Generate one with: p2p-remote --gen-cert");
                return Err("No client certificate found".into());
            }
        }
    }

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

    // Initialize WebRTC client (manual copy-paste mode: no relay)
    webrtc_conn::run_client(auth, &sdp_offer, None).await?;

    Ok(())
}
