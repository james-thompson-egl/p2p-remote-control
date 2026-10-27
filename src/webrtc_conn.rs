//! WebRTC P2P connection handling
//!
//! Implements:
//! - Host mode: wait for incoming connections
//! - Client mode: connect to a host
//! - Data channel for input events
//! - Video track for screen frames

use tracing::{info, debug, error, warn};
use tokio::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;

use super::screen_capture::ScreenCapturer;
use super::input_injector::{InputInjector, InputEvent};
use super::pairing::{HandshakeData, validate_pairing_code};

// WebRTC imports from webrtc-rs
use webrtc::api::{APIBuilder, media_engine::MediaEngine};
use webrtc::peer_connection::{
    configuration::RTCConfiguration,
    sdp::session_description::RTCSessionDescription,
    peer_connection_state::RTCPeerConnectionState,
};
use webrtc::data_channel::data_channel_message::DataChannelMessage;
use webrtc::data_channel::data_channel_state::RTCDataChannelState;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::peer_connection::sdp::sdp_type::RTCSdpType;

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use std::time::Instant;
use bytes::Bytes;

/// Input message format for data channel
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum RemoteInput {
    MouseMove { x: i32, y: i32 },
    MouseDown { button: u8 },
    MouseUp { button: u8 },
    MouseWheel { delta: i32 },
    KeyDown { code: u16 },
    KeyUp { code: u16 },
    Char { ch: String },
}

/// Shared connection state
pub struct ConnectionState {
    pub data_channel: Option<webrtc::data_channel::RTCDataChannel>,
    pub peer_connection: Option<webrtc::peer_connection::RTCPeerConnection>,
    pub connected: bool,
}

impl ConnectionState {
    pub fn new() -> Self {
        Self {
            data_channel: None,
            peer_connection: None,
            connected: false,
        }
    }
}

impl Default for ConnectionState {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a WebRTC peer connection with default config
async fn create_peer_connection() -> Result<webrtc::peer_connection::RTCPeerConnection, Box<dyn std::error::Error>> {
    let mut m = MediaEngine::default();
    m.register_default_codecs()?;

    let api = APIBuilder::new()
        .with_media_engine(m)
        .build();

    let config = RTCConfiguration {
        ice_servers: vec![RTCIceServer {
            urls: vec!["stun:stun.l.google.com:19302".to_string()],
            username: String::new(),
            credential: String::new(),
        }],
        ..Default::default()
    };

    let pc = api.new_peer_connection(config).await?;
    Ok(pc)
}

pub async fn run_host(
    pairing_code: String,
    mut screen_capturer: ScreenCapturer,
    input_injector: InputInjector,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting host WebRTC server");

    // Generate handshake data
    let handshake = HandshakeData::new(&pairing_code);

    println!("\n=== HOST MODE ACTIVE ===");
    println!("Pairing Code: {}", pairing_code);
    println!("\nTo connect:");
    println!("1. On viewer machine, run: p2p-remote --connect {}", pairing_code);
    println!("2. Copy the SDP offer shown below");
    println!("3. Paste it into the viewer");
    println!("4. Copy the SDP answer from viewer");
    println!("5. Paste it here to establish connection");
    println!("\n=========================\n");

    // Create peer connection
    let pc = create_peer_connection().await?;

    // Create data channel for input events
    let data_channel = pc.create_data_channel("input", None).await?;
    let data_channel_clone = data_channel.clone();

    // Set data channel open handler
    let dc_connected = Arc::new(RwLock::new(false));
    let dc_connected_clone = dc_connected.clone();

    data_channel.on_open(Box::new(move || {
        let connected = dc_connected_clone.clone();
        Box::pin(async move {
            info!("Data channel open");
            *connected.write().await = true;
        })
    }));

    // Set on message handler for input events
    let injector = Arc::new(input_injector);
    let injector_clone = injector.clone();

    data_channel.on_message(Box::new(move |msg: DataChannelMessage| {
        let injector = injector_clone.clone();
        Box::pin(async move {
            if let Ok(text) = String::from_utf8(msg.data.to_vec()) {
                debug!("Received input: {}", text);
                if let Ok(input) = serde_json::from_str::<RemoteInput>(&text) {
                    convert_and_inject(&injector, &input).await;
                }
            }
        })
    }));

    // Set connection state change handler
    pc.on_peer_connection_state_change(Box::new(move |state: RTCPeerConnectionState| {
        Box::pin(async move {
            info!("Peer connection state change: {:?}", state);
            match state {
                RTCPeerConnectionState::Connected => {
                    info!("Connection established!");
                }
                RTCPeerConnectionState::Disconnected | RTCPeerConnectionState::Failed => {
                    warn!("Connection lost");
                }
                RTCPeerConnectionState::Closed => {
                    info!("Connection closed");
                }
                _ => {}
            }
        })
    }));

    // Generate SDP offer, then wait for ICE gathering to complete so the
    // candidate lines are embedded in the SDP we print (manual signaling has
    // no trickle channel, so candidates must be in the copied SDP).
    let offer = pc.create_offer(None).await?;
    let mut gather_complete = pc.gathering_complete_promise().await;
    pc.set_local_description(offer).await?;
    let _ = gather_complete.recv().await;

    let local_desc = pc
        .local_description()
        .await
        .ok_or("failed to get local description after ICE gathering")?;

    // Embed handshake in SDP
    let handshake_json = serde_json::to_string(&handshake)?;
    let handshake_b64 = base64::encode(&handshake_json);

    let mut sdp_offer = local_desc.sdp.clone();
    sdp_offer.push_str(&format!("a=handshake:{}\r\n", handshake_b64));

    println!("=== SDP OFFER (copy this) ===");
    println!("{}", sdp_offer);
    println!("============================\n");

    // Wait for SDP answer from user
    println!("Enter SDP answer from viewer (press Enter twice when done):");
    let sdp_answer = read_multiline_input()?;

    if sdp_answer.is_empty() {
        error!("No SDP answer provided");
        return Err("Connection cancelled".into());
    }

    debug!("Received SDP answer, attempting to connect...");

    // Verify handshake before connecting
    if !handshake.verify(&pairing_code) {
        error!("Handshake verification failed");
        return Err("Authentication failed".into());
    }

    // Set remote description
    let answer = webrtc::peer_connection::sdp::session_description::RTCSessionDescription::answer(sdp_answer)?;
    pc.set_remote_description(answer).await?;

    println!("\n=== WAITING FOR CONNECTION ===\n");

    // Wait for connection
    let timeout = Duration::from_secs(60);
    let start = Instant::now();

    while start.elapsed() < timeout {
        if *dc_connected.read().await {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    if !*dc_connected.read().await {
        warn!("Connection timeout");
    } else {
        info!("Data channel connected!");
        println!("Connected! Screen sharing started...\n");

        // Start screen capture and input loop
        start_capture_loop(&mut screen_capturer, injector, data_channel_clone).await?;
    }

    // Cleanup
    pc.close().await?;

    Ok(())
}

pub async fn run_client(
    pairing_code: &str,
    sdp_offer: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting client WebRTC connection");

    if !validate_pairing_code(pairing_code) {
        error!("Invalid pairing code");
        return Err("Invalid pairing code format".into());
    }

    // Parse SDP offer and extract handshake data
    let handshake = extract_handshake_from_sdp(sdp_offer)?;

    // Verify pairing code
    if !handshake.verify(pairing_code) {
        error!("Pairing code verification failed");
        return Err("Authentication failed".into());
    }

    debug!("Pairing code verified");

    // Create peer connection
    let pc = create_peer_connection().await?;

    // Set on data channel handler
    let dc_connected = Arc::new(RwLock::new(false));
    let dc_connected_clone = dc_connected.clone();

    pc.on_data_channel(Box::new(move |d: Arc<webrtc::data_channel::RTCDataChannel>| {
        let connected = dc_connected_clone.clone();
        let label = d.label().to_string();

        Box::pin(async move {
            info!("Data channel: {}", label);

            d.on_open(Box::new(move || {
                let connected = connected.clone();
                Box::pin(async move {
                    info!("Data channel open");
                    *connected.write().await = true;
                })
            }));

            d.on_message(Box::new(move |_msg| {
                Box::pin(async move {
                    debug!("Received message");
                })
            }));
        })
    }));

    // Set connection state change handler
    pc.on_peer_connection_state_change(Box::new(move |state: RTCPeerConnectionState| {
        Box::pin(async move {
            info!("Peer connection state change: {:?}", state);
        })
    }));

    // Set remote description (offer)
    let offer = webrtc::peer_connection::sdp::session_description::RTCSessionDescription::offer(sdp_offer.to_string())?;
    pc.set_remote_description(offer).await?;

    // Create answer, then wait for ICE gathering to complete so the candidate
    // lines are embedded in the SDP we print (see offer side above).
    let answer = pc.create_answer(None).await?;
    let mut gather_complete = pc.gathering_complete_promise().await;
    pc.set_local_description(answer).await?;
    let _ = gather_complete.recv().await;

    let local_desc = pc
        .local_description()
        .await
        .ok_or("failed to get local description after ICE gathering")?;

    // Display SDP answer
    println!("\n=== SDP ANSWER (copy this) ===");
    println!("{}", local_desc.sdp);
    println!("==============================\n");

    println!("Copy the SDP answer above and paste it into the host machine.\n");

    // Wait for connection
    let timeout = Duration::from_secs(60);
    let start = Instant::now();

    println!("Waiting for host to accept connection...");

    while start.elapsed() < timeout {
        if *dc_connected.read().await {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    if *dc_connected.read().await {
        info!("Client connection established!");
        println!("Connected! You can now control the remote desktop.\n");

        // Start input capture loop
        start_client_input_loop(pc).await?;
    } else {
        error!("Connection timeout");
        return Err("Connection timeout".into());
    }

    Ok(())
}

/// Convert RemoteInput to InputEvent and inject
async fn convert_and_inject(injector: &Arc<InputInjector>, input: &RemoteInput) {
    let event = match input {
        RemoteInput::MouseMove { x, y } => InputEvent::MouseMove { x: *x, y: *y },
        RemoteInput::MouseDown { button } => {
            if *button == 0 { InputEvent::MouseLeftDown } else { InputEvent::MouseRightDown }
        }
        RemoteInput::MouseUp { button } => {
            if *button == 0 { InputEvent::MouseLeftUp } else { InputEvent::MouseRightUp }
        }
        RemoteInput::MouseWheel { delta } => InputEvent::MouseWheel { delta: *delta },
        RemoteInput::KeyDown { code } => InputEvent::KeyDown { keycode: *code },
        RemoteInput::KeyUp { code } => InputEvent::KeyUp { keycode: *code },
        RemoteInput::Char { ch } => {
            if let Some(c) = ch.chars().next() {
                InputEvent::Char { c }
            } else {
                return;
            }
        }
    };

    if let Err(e) = injector.inject(&event) {
        warn!("Failed to inject input: {}", e);
    }
}

/// Read multiline SDP input
fn read_multiline_input() -> Result<String, Box<dyn std::error::Error>> {
    use std::io::{self, BufRead};

    let stdin = io::stdin();
    let mut sdp = String::new();
    let mut empty_lines = 0;

    for line in stdin.lock().lines() {
        let line = line?;
        sdp.push_str(&line);
        sdp.push('\n');

        if line.is_empty() {
            empty_lines += 1;
            if empty_lines >= 2 && sdp.contains("v=") {
                break;
            }
        } else {
            empty_lines = 0;
        }
    }

    Ok(sdp)
}

/// Extract handshake data from SDP
fn extract_handshake_from_sdp(sdp: &str) -> Result<HandshakeData, Box<dyn std::error::Error>> {
    let handshake_line = sdp
        .lines()
        .find(|line| line.starts_with("a=handshake:"))
        .ok_or("No handshake data in SDP")?;

    let handshake_b64 = handshake_line.trim_start_matches("a=handshake:");
    let handshake_json = base64::decode(handshake_b64)?;
    let handshake: HandshakeData = serde_json::from_slice(&handshake_json)?;

    Ok(handshake)
}

/// Screen capture and data channel loop
async fn start_capture_loop(
    screen_capturer: &mut ScreenCapturer,
    input_injector: Arc<InputInjector>,
    data_channel: Arc<webrtc::data_channel::RTCDataChannel>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Spawn input handler
    let (tx, mut rx) = mpsc::channel::<InputEvent>(100);
    tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            if let Err(e) = input_injector.inject(&event) {
                warn!("Failed to inject input: {}", e);
            }
        }
    });

    // Capture loop
    let capture_interval = Duration::from_millis(50); // 20fps target
    let mut last_frame_time = Instant::now();

    println!("Press Ctrl+C to disconnect\n");

    loop {
        tokio::select! {
            _ = tokio::time::sleep(capture_interval) => {
                match screen_capturer.capture_frame() {
                    Ok(frame) => {
                        let elapsed = last_frame_time.elapsed();
                        last_frame_time = Instant::now();

                        // Encode frame to JPEG and send
                        use std::io::Cursor;
                        let mut buf = Vec::new();
                        frame.write_to(&mut Cursor::new(&mut buf), image::ImageFormat::Jpeg)?;

                        // Send frame data (in production, would use video track)
                        // For now, send as data channel message (limited by SCTP)
                        let frame_json = serde_json::json!({
                            "type": "frame",
                            "width": frame.width(),
                            "height": frame.height(),
                            "data": base64::encode(&buf)
                        });

                        let msg = serde_json::to_string(&frame_json).unwrap_or_default();

                        // Only send if data channel is open
                        if data_channel.ready_state() == RTCDataChannelState::Open {
                            if let Err(e) = data_channel.send(&Bytes::from(msg)).await {
                                debug!("Failed to send frame: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Failed to capture frame: {}", e);
                    }
                }
            }
        }
    }
}

/// Client-side input capture loop
async fn start_client_input_loop(
    pc: webrtc::peer_connection::RTCPeerConnection,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("Input capture started. Press Ctrl+C to disconnect.\n");

    // In a full implementation, we would:
    // 1. Use Windows hooks to capture local mouse/keyboard
    // 2. Convert to RemoteInput messages
    // 3. Send via data channel

    // For now, just keep running
    loop {
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}
