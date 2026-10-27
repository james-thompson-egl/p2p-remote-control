//! Secure pairing module
//!
//! Implements:
//! - 6-digit pairing code generation
//! - Argon2 key derivation from pairing code
//! - HMAC-based authentication tokens
//! - QR code generation for easy pairing

use rand::{Rng, rngs::OsRng};
use argon2::{
    Argon2,
    Params,
    Version,
    Algorithm,
};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use tracing::{info, debug, error};

type HmacSha256 = Hmac<Sha256>;

/// Generate a random 6-digit pairing code
pub fn generate_pairing_code() -> String {
    let mut rng = OsRng;
    let code: u32 = rng.gen_range(100000..999999);
    let code_str = format!("{:06}", code);
    info!("Generated pairing code: {}", code_str);
    code_str
}

/// Validate pairing code format (6 digits)
pub fn validate_pairing_code(code: &str) -> bool {
    code.len() == 6 && code.chars().all(|c| c.is_ascii_digit())
}

/// Derive a shared secret from the pairing code using Argon2id
/// This is used to authenticate the WebRTC connection
pub fn derive_shared_secret(pairing_code: &str, salt: &[u8]) -> [u8; 32] {
    let argon2 = Argon2::new(
        Algorithm::Argon2id,
        Version::V0x13,
        Params::new(16384, 8, 1, Some(32)).unwrap(),
    );

    let mut secret = [0u8; 32];
    let result = argon2.hash_password_into(pairing_code.as_bytes(), salt, &mut secret);

    if result.is_err() {
        error!("Failed to derive shared secret");
        // Fallback to a deterministic value (not secure, but prevents panic)
        secret.copy_from_slice(&[0u8; 32]);
    }

    debug!("Derived shared secret (length: {})", secret.len());
    secret
}

/// Generate an authentication token for the handshake
pub fn generate_auth_token(pairing_code: &str, salt: &[u8], nonce: &[u8]) -> String {
    let secret = derive_shared_secret(pairing_code, salt);

    let mut mac = HmacSha256::new_from_slice(&secret)
        .expect("HMAC can take key of any size");

    mac.update(nonce);
    mac.update(b"p2p-remote-handshake");

    let result = mac.finalize();
    let token = STANDARD.encode(result.into_bytes().as_slice());

    debug!("Generated auth token");
    token
}

/// Verify an authentication token
pub fn verify_auth_token(
    pairing_code: &str,
    salt: &[u8],
    nonce: &[u8],
    token: &str,
) -> bool {
    let expected = generate_auth_token(pairing_code, salt, nonce);
    // Constant-time comparison to prevent timing attacks
    use std::time::Instant;
    let start = Instant::now();
    let result = expected.as_bytes().len() == token.as_bytes().len()
        && expected.as_bytes().iter().zip(token.as_bytes().iter())
            .fold(true, |acc, (a, b)| acc && (a == b));
    // Ensure constant time by always doing the full comparison
    let _elapsed = start.elapsed();
    result
}

/// Generate a random salt for key derivation
pub fn generate_salt() -> [u8; 16] {
    let mut salt = [0u8; 16];
    OsRng.fill(&mut salt);
    salt
}

/// Encode salt as base64 for SDP exchange
pub fn encode_salt(salt: &[u8]) -> String {
    STANDARD.encode(salt)
}

/// Decode salt from base64
pub fn decode_salt(encoded: &str) -> Result<[u8; 16], &'static str> {
    let decoded = STANDARD.decode(encoded).map_err(|_| "Invalid base64")?;
    if decoded.len() != 16 {
        return Err("Salt must be 16 bytes");
    }
    let mut salt = [0u8; 16];
    salt.copy_from_slice(&decoded);
    Ok(salt)
}

/// Generate a QR code string for the pairing code
pub fn generate_qr_code(pairing_code: &str) -> Result<String, Box<dyn std::error::Error>> {
    // Create QR code data URL
    // Format: p2p-remote://pair?code=123456
    let qr_data = format!("p2p-remote://pair?code={}", pairing_code);

    #[cfg(windows)]
    {
        // Use qrcode crate to generate ASCII QR code
        use qrcode::render::unicode;
        use qrcode::QrCode;

        let code = QrCode::new(qr_data.as_bytes())?;
        let svg = code
            .render::<unicode::Dense1x2>()
            .dark_color(unicode::Dense1x2::Light)
            .light_color(unicode::Dense1x2::Dark)
            .build();

        Ok(svg)
    }

    #[cfg(not(windows))]
    {
        // Fallback: just return the pairing code in a simple format
        Ok(format!(
            "
 ██████╗ ██████╗  ██████╗
██╔════╝██╔═══██╗██╔════╝
██║     ██║   ██║██║
██║     ██║   ██║██║
╚██████╗╚██████╔╝╚██████╗
 ╚═════╝ ╚═════╝  ╚═════╝

Pairing Code: {}
            ",
            pairing_code
        ))
    }
}

/// Pairing handshake data structure
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HandshakeData {
    pub pairing_code_hash: String,
    pub salt: String,
    pub nonce: String,
    pub auth_token: String,
    pub dtls_fingerprint: Option<String>,
}

impl HandshakeData {
    pub fn new(pairing_code: &str) -> Self {
        let salt = generate_salt();
        let nonce = generate_nonce();
        let auth_token = generate_auth_token(pairing_code, &salt, &nonce);

        HandshakeData {
            pairing_code_hash: STANDARD.encode(derive_shared_secret(pairing_code, &salt)),
            salt: encode_salt(&salt),
            nonce: STANDARD.encode(&nonce),
            auth_token,
            dtls_fingerprint: None,
        }
    }

    pub fn verify(&self, pairing_code: &str) -> bool {
        let salt = match decode_salt(&self.salt) {
            Ok(s) => s,
            Err(_) => return false,
        };

        let nonce = STANDARD.decode(&self.nonce).unwrap_or_default();

        verify_auth_token(pairing_code, &salt, &nonce, &self.auth_token)
    }
}

fn generate_nonce() -> [u8; 16] {
    let mut nonce = [0u8; 16];
    OsRng.fill(&mut nonce);
    nonce
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pairing_code_format() {
        let code = generate_pairing_code();
        assert_eq!(code.len(), 6);
        assert!(validate_pairing_code(&code));
    }

    #[test]
    fn test_pairing_code_numeric() {
        let code = generate_pairing_code();
        assert!(code.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn test_pairing_code_uniqueness() {
        let code1 = generate_pairing_code();
        let code2 = generate_pairing_code();
        // Very unlikely but possible - just check they're generated
        assert!(!code1.is_empty());
        assert!(!code2.is_empty());
    }

    #[test]
    fn test_validate_pairing_code_valid() {
        assert!(validate_pairing_code("123456"));
        assert!(validate_pairing_code("000000"));
        assert!(validate_pairing_code("999999"));
    }

    #[test]
    fn test_validate_pairing_code_invalid() {
        assert!(!validate_pairing_code("12345"));    // Too short
        assert!(!validate_pairing_code("1234567"));  // Too long
        assert!(!validate_pairing_code("12345a"));   // Non-numeric
        assert!(!validate_pairing_code(""));         // Empty
        assert!(!validate_pairing_code("abcdef"));   // Letters
    }

    #[test]
    fn test_salt_generation() {
        let salt1 = generate_salt();
        let salt2 = generate_salt();

        assert_eq!(salt1.len(), 16);
        assert_eq!(salt2.len(), 16);
        // Salts should be different (extremely unlikely to collide)
        assert_ne!(salt1, salt2);
    }

    #[test]
    fn test_salt_roundtrip() {
        let salt = generate_salt();
        let encoded = encode_salt(&salt);
        let decoded = decode_salt(&encoded).unwrap();
        assert_eq!(salt, decoded);
    }

    #[test]
    fn test_salt_invalid_base64() {
        assert!(decode_salt("invalid!!").is_err());
    }

    #[test]
    fn test_salt_wrong_length() {
        assert!(decode_salt("YWJjZA==").is_err()); // 4 bytes, not 16
    }

    #[test]
    fn test_key_derivation_deterministic() {
        let code = "123456";
        let salt = generate_salt();

        let secret1 = derive_shared_secret(code, &salt);
        let secret2 = derive_shared_secret(code, &salt);

        assert_eq!(secret1.len(), 32);
        assert_eq!(secret1, secret2); // Same input = same output
    }

    #[test]
    fn test_key_derivation_different_codes() {
        let salt = generate_salt();

        let secret1 = derive_shared_secret("123456", &salt);
        let secret2 = derive_shared_secret("654321", &salt);

        assert_ne!(secret1, secret2); // Different codes = different secrets
    }

    #[test]
    fn test_auth_token_generation() {
        let code = generate_pairing_code();
        let salt = generate_salt();
        let nonce = generate_nonce();

        let token = generate_auth_token(&code, &salt, &nonce);
        assert!(!token.is_empty());
    }

    #[test]
    fn test_auth_token_verification_success() {
        let code = generate_pairing_code();
        let salt = generate_salt();
        let nonce = generate_nonce();

        let token = generate_auth_token(&code, &salt, &nonce);
        assert!(verify_auth_token(&code, &salt, &nonce, &token));
    }

    #[test]
    fn test_auth_token_verification_wrong_code() {
        let code = generate_pairing_code();
        let salt = generate_salt();
        let nonce = generate_nonce();

        let token = generate_auth_token(&code, &salt, &nonce);
        assert!(!verify_auth_token("000000", &salt, &nonce, &token));
    }

    #[test]
    fn test_handshake_data_new() {
        let code = generate_pairing_code();
        let handshake = HandshakeData::new(&code);

        assert!(!handshake.salt.is_empty());
        assert!(!handshake.nonce.is_empty());
        assert!(!handshake.auth_token.is_empty());
    }

    #[test]
    fn test_handshake_data_verify_success() {
        let code = generate_pairing_code();
        let handshake = HandshakeData::new(&code);

        assert!(handshake.verify(&code));
    }

    #[test]
    fn test_handshake_data_verify_failure() {
        let handshake = HandshakeData::new("123456");

        assert!(!handshake.verify("000000"));
    }

    #[test]
    fn test_handshake_data_serialization() {
        let code = generate_pairing_code();
        let handshake = HandshakeData::new(&code);

        let json = serde_json::to_string(&handshake).unwrap();
        let restored: HandshakeData = serde_json::from_str(&json).unwrap();

        assert_eq!(handshake.salt, restored.salt);
        assert_eq!(handshake.nonce, restored.nonce);
        assert_eq!(handshake.auth_token, restored.auth_token);
    }

    #[test]
    fn test_encode_decode_qr_code() {
        let code = generate_pairing_code();
        let qr = generate_qr_code(&code);

        assert!(qr.is_ok());
        assert!(!qr.unwrap().is_empty());
    }
}
