//! Optional signalling relay client.
//!
//! Lets a host and a viewer exchange their SDP offer/answer through a dumb relay
//! (the Oversight Dashboard's `/api/signal` endpoint) instead of a human copying
//! and pasting them between two machines.
//!
//! The host mints a high-entropy one-time **token** and shows it; the viewer is
//! given the same token. Both derive:
//!
//!   - the **rendezvous id** = `sha256("p2p-remote-rendezvous|" + token)`, which is
//!     what the relay is keyed by and all it ever sees; and
//!   - the **pairing secret** = the token itself, fed into the existing pairing
//!     HMAC / DTLS-fingerprint binding (see `pairing.rs`).
//!
//! Because the relay only ever receives the hash, it cannot derive the token, so
//! it cannot forge the handshake or man-in-the-middle the session — a tampered
//! offer/answer fails the fingerprint check on the client. The relay is untrusted
//! by design; it only shuttles two blobs.

use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use rand::{rngs::OsRng, RngCore};
use tracing::{debug, info};

type BoxErr = Box<dyn std::error::Error>;

/// Mint a one-time connection token: 16 random bytes (128 bits), base64url.
pub fn generate_token() -> String {
    let mut bytes = [0u8; 16];
    OsRng.fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

/// Derive the relay rendezvous id from a token. This is the only thing the relay
/// sees; it is a one-way hash, so the relay cannot recover the token.
pub fn rendezvous_id(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"p2p-remote-rendezvous|");
    hasher.update(token.as_bytes());
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect()
}

/// A connection to the signalling relay for one rendezvous.
pub struct Relay {
    base_url: String,
    /// The shared one-time token. Also the pairing secret — never sent to the relay.
    pub token: String,
    rendezvous_id: String,
    client: reqwest::Client,
}

impl Relay {
    pub fn new(base_url: &str, token: String) -> Self {
        let rendezvous_id = rendezvous_id(&token);
        Relay {
            base_url: base_url.trim_end_matches('/').to_string(),
            token,
            rendezvous_id,
            client: reqwest::Client::new(),
        }
    }

    fn url(&self, kind: &str) -> String {
        format!("{}/api/signal/rendezvous/{}/{}", self.base_url, self.rendezvous_id, kind)
    }

    async fn put_sdp(&self, kind: &str, sdp: &str) -> Result<(), BoxErr> {
        let resp = self
            .client
            .put(self.url(kind))
            .json(&serde_json::json!({ "sdp": sdp }))
            .send()
            .await?;
        if !resp.status().is_success() {
            return Err(format!("relay rejected {} ({})", kind, resp.status()).into());
        }
        debug!("posted {} to relay", kind);
        Ok(())
    }

    /// GET an SDP blob; `None` when the relay has none yet (404).
    async fn get_sdp(&self, kind: &str, field: &str) -> Result<Option<String>, BoxErr> {
        let resp = self.client.get(self.url(kind)).send().await?;
        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !resp.status().is_success() {
            return Err(format!("relay error fetching {} ({})", kind, resp.status()).into());
        }
        let body: serde_json::Value = resp.json().await?;
        Ok(body.get(field).and_then(|v| v.as_str()).map(|s| s.to_string()))
    }

    pub async fn put_offer(&self, sdp: &str) -> Result<(), BoxErr> {
        self.put_sdp("offer", sdp).await
    }
    pub async fn put_answer(&self, sdp: &str) -> Result<(), BoxErr> {
        self.put_sdp("answer", sdp).await
    }

    /// Poll for the viewer's answer (host side) until it arrives or we time out.
    pub async fn poll_answer(&self, timeout: Duration) -> Result<String, BoxErr> {
        self.poll("answer", "answer", timeout, "viewer to answer").await
    }

    /// Poll for the host's offer (viewer side) until it arrives or we time out.
    pub async fn poll_offer(&self, timeout: Duration) -> Result<String, BoxErr> {
        self.poll("offer", "offer", timeout, "host to post its offer").await
    }

    async fn poll(&self, kind: &str, field: &str, timeout: Duration, waiting_for: &str) -> Result<String, BoxErr> {
        let start = Instant::now();
        info!("Waiting for {} via relay...", waiting_for);
        loop {
            if let Some(sdp) = self.get_sdp(kind, field).await? {
                return Ok(sdp);
            }
            if start.elapsed() > timeout {
                return Err(format!("timed out waiting for {}", waiting_for).into());
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rendezvous_id_is_64_hex_and_stable() {
        let id = rendezvous_id("some-token");
        assert_eq!(id.len(), 64);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(id, rendezvous_id("some-token"));
    }

    #[test]
    fn different_tokens_give_different_ids() {
        assert_ne!(rendezvous_id("token-a"), rendezvous_id("token-b"));
    }

    #[test]
    fn the_id_does_not_reveal_the_token() {
        // The relay is keyed by the hash; it must not contain the token text.
        let token = "super-secret-token";
        assert!(!rendezvous_id(token).contains(token));
    }

    #[test]
    fn generated_tokens_are_high_entropy_and_unique() {
        let a = generate_token();
        let b = generate_token();
        assert_ne!(a, b);
        assert!(a.len() >= 20); // 16 bytes base64url, no padding ≈ 22 chars
    }
}
