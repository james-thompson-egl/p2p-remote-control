//! WebRTC client for the viewer
//!
//! Handles:
//! - Creating peer connections
//! - Receiving video frames
//! - Sending input events

use tracing::{info, debug, error};
use webrtc::api::{APIBuilder, media_engine::MediaEngine};
use webrtc::peer_connection::{
    configuration::RTCConfiguration,
    sdp::session_description::RTCSessionDescription,
    peer_connection_state::RTCPeerConnectionState,
};
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::data_channel::data_channel_state::RTCDataChannelState;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::input_handler::RemoteInput;

/// WebRTC client for remote viewer
pub struct WebRTCClient {
    peer_connection: Option<webrtc::peer_connection::RTCPeerConnection>,
    data_channel: Arc<RwLock<Option<webrtc::data_channel::RTCDataChannel>>>,
}

impl WebRTCClient {
    pub fn new() -> Self {
        Self {
            peer_connection: None,
            data_channel: Arc::new(RwLock::new(None)),
        }
    }

    /// Create a peer connection
    async fn create_peer_connection(&mut self) -> Result<(), Box<dyn std::error::Error>> {
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

        // Set up data channel handler
        let data_channel = self.data_channel.clone();

        pc.on_data_channel(Box::new(move |d: Arc<webrtc::data_channel::RTCDataChannel>| {
            let dc = data_channel.clone();
            async move {
                info!("Data channel created: {}", d.label());

                d.on_open(Box::new(move || {
                    let dc = dc.clone();
                    async move {
                        *dc.write().await = Some(d.clone());
                        info!("Data channel open");
                    }
                }));

                d.on_message(Box::new(move |msg| {
                    async move {
                        if let Ok(text) = String::from_utf8(msg.data) {
                            debug!("Received frame data: {} bytes", text.len());
                        }
                    }
                }));
            }
        }));

        // Set connection state handler
        pc.on_peer_connection_state_change(Box::new(|state: RTCPeerConnectionState| {
            info!("Connection state: {:?}", state);
            async {}
        }));

        self.peer_connection = Some(pc);
        Ok(())
    }

    /// Run as host (waiting for connection)
    pub async fn run_host(pairing_code: &str) -> Result<String, Box<dyn std::error::Error>> {
        info!("Host mode with pairing code: {}", pairing_code);

        let mut client = Self::new();
        client.create_peer_connection().await?;

        let pc = client.peer_connection.as_ref().unwrap();

        // Create data channel
        let _dc = pc.create_data_channel("input", None).await?;

        // Create offer
        let offer = pc.create_offer(None).await?;
        pc.set_local_description(offer.clone()).await?;

        Ok(offer.sdp)
    }

    /// Run as client (connecting to host)
    pub async fn run_client(
        pairing_code: &str,
        sdp_offer: &str,
    ) -> Result<String, Box<dyn std::error::Error>> {
        info!("Client mode with pairing code: {}", pairing_code);

        let mut client = Self::new();
        client.create_peer_connection().await?;

        let pc = client.peer_connection.as_ref().unwrap();

        // Set remote description
        let offer = RTCSessionDescription {
            r#type: webrtc::sdp::SessionDescriptionType::Offer,
            sdp: sdp_offer.to_string(),
        };
        pc.set_remote_description(offer).await?;

        // Create answer
        let answer = pc.create_answer(None).await?;
        pc.set_local_description(answer.clone()).await?;

        Ok(answer.sdp)
    }

    /// Send input event to remote host
    pub async fn send_input(&self, input: &RemoteInput) -> Result<(), Box<dyn std::error::Error>> {
        let dc_guard = self.data_channel.read().await;
        if let Some(dc) = dc_guard.as_ref() {
            if dc.read_state() == RTCDataChannelState::Open {
                let json = serde_json::to_string(input)?;
                dc.send_text(&json).await?;
                debug!("Sent input: {:?}", input);
            } else {
                error!("Data channel not open");
            }
        } else {
            error!("No data channel available");
        }
        Ok(())
    }
}

impl Default for WebRTCClient {
    fn default() -> Self {
        Self::new()
    }
}
