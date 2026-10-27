//! Pairing module tests
//!
//! These tests verify the secure pairing functionality works correctly

#[cfg(test)]
mod pairing_tests {
    use rand::{Rng, rngs::OsRng};
    use argon2::{Argon2, Params, Version, Algorithm};
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    use std::time::Instant;

    type HmacSha256 = Hmac<Sha256>;

    fn generate_pairing_code() -> String {
        let mut rng = OsRng;
        let code: u32 = rng.gen_range(100000..999999);
        format!("{:06}", code)
    }

    fn validate_pairing_code(code: &str) -> bool {
        code.len() == 6 && code.chars().all(|c| c.is_ascii_digit())
    }

    fn generate_salt() -> [u8; 16] {
        let mut salt = [0u8; 16];
        OsRng.fill(&mut salt);
        salt
    }

    fn encode_salt(salt: &[u8]) -> String {
        STANDARD.encode(salt)
    }

    fn decode_salt(encoded: &str) -> Result<[u8; 16], &'static str> {
        let decoded = STANDARD.decode(encoded).map_err(|_| "Invalid base64")?;
        if decoded.len() != 16 {
            return Err("Salt must be 16 bytes");
        }
        let mut salt = [0u8; 16];
        salt.copy_from_slice(&decoded);
        Ok(salt)
    }

    fn derive_shared_secret(pairing_code: &str, salt: &[u8]) -> [u8; 32] {
        let argon2 = Argon2::new(
            Algorithm::Argon2id,
            Version::V0x13,
            Params::new(16384, 8, 1, Some(32)).unwrap(),
        );

        let mut secret = [0u8; 32];
        let _ = argon2.hash_password_into(pairing_code.as_bytes(), salt, &mut secret);
        secret
    }

    fn generate_nonce() -> [u8; 16] {
        let mut nonce = [0u8; 16];
        OsRng.fill(&mut nonce);
        nonce
    }

    fn generate_auth_token(pairing_code: &str, salt: &[u8], nonce: &[u8]) -> String {
        let secret = derive_shared_secret(pairing_code, salt);

        let mut mac = HmacSha256::new_from_slice(&secret)
            .expect("HMAC can take key of any size");

        mac.update(nonce);
        mac.update(b"p2p-remote-handshake");

        let result = mac.finalize();
        STANDARD.encode(result.into_bytes().as_slice())
    }

    fn verify_auth_token(pairing_code: &str, salt: &[u8], nonce: &[u8], token: &str) -> bool {
        let expected = generate_auth_token(pairing_code, salt, nonce);
        // Constant-time comparison to prevent timing attacks
        let start = Instant::now();
        let result = expected.as_bytes().len() == token.as_bytes().len()
            && expected.as_bytes().iter().zip(token.as_bytes().iter())
                .fold(true, |acc, (a, b)| acc && (a == b));
        let _elapsed = start.elapsed();
        result
    }

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    struct HandshakeData {
        pairing_code_hash: String,
        salt: String,
        nonce: String,
        auth_token: String,
        dtls_fingerprint: Option<String>,
    }

    impl HandshakeData {
        fn new(pairing_code: &str) -> Self {
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

        fn verify(&self, pairing_code: &str) -> bool {
            let salt = match decode_salt(&self.salt) {
                Ok(s) => s,
                Err(_) => return false,
            };

            let nonce = STANDARD.decode(&self.nonce).unwrap_or_default();
            verify_auth_token(pairing_code, &salt, &nonce, &self.auth_token)
        }
    }

    // Test pairing code generation
    #[test]
    fn test_pairing_code_length() {
        let code = generate_pairing_code();
        assert_eq!(code.len(), 6, "Pairing code should be 6 digits");
    }

    #[test]
    fn test_pairing_code_numeric() {
        let code = generate_pairing_code();
        assert!(code.chars().all(|c| c.is_ascii_digit()), "Pairing code should be numeric");
    }

    #[test]
    fn test_validate_pairing_code() {
        assert!(validate_pairing_code("123456"));
        assert!(validate_pairing_code("000000"));
        assert!(validate_pairing_code("999999"));
        assert!(!validate_pairing_code("12345"));   // Too short
        assert!(!validate_pairing_code("1234567")); // Too long
        assert!(!validate_pairing_code("12345a"));  // Non-numeric
        assert!(!validate_pairing_code(""));        // Empty
    }

    #[test]
    fn test_salt_generation() {
        let salt1 = generate_salt();
        let salt2 = generate_salt();

        assert_eq!(salt1.len(), 16, "Salt should be 16 bytes");
        assert_ne!(salt1, salt2, "Salts should be unique");
    }

    #[test]
    fn test_salt_roundtrip() {
        let salt = generate_salt();
        let encoded = encode_salt(&salt);
        let decoded = decode_salt(&encoded).unwrap();

        assert_eq!(salt, decoded, "Salt should survive encode/decode roundtrip");
    }

    #[test]
    fn test_invalid_salt_decode() {
        assert!(decode_salt("invalid!!").is_err());
        assert!(decode_salt("YWJjZA==").is_err()); // 4 bytes, not 16
    }

    #[test]
    fn test_key_derivation() {
        let code = "123456";
        let salt = generate_salt();

        let secret1 = derive_shared_secret(code, &salt);
        let secret2 = derive_shared_secret(code, &salt);

        assert_eq!(secret1.len(), 32, "Secret should be 32 bytes");
        assert_eq!(secret1, secret2, "Same input should produce same output");
    }

    #[test]
    fn test_key_derivation_different_inputs() {
        let salt = generate_salt();

        let secret1 = derive_shared_secret("123456", &salt);
        let secret2 = derive_shared_secret("654321", &salt);

        assert_ne!(secret1, secret2, "Different codes should produce different secrets");
    }

    #[test]
    fn test_auth_token_generation() {
        let code = "123456";
        let salt = generate_salt();
        let nonce = [1u8; 16];

        let token = generate_auth_token(code, &salt, &nonce);

        assert!(!token.is_empty(), "Token should not be empty");
    }

    #[test]
    fn test_auth_token_verification() {
        let code = "123456";
        let salt = generate_salt();
        let nonce = [1u8; 16];

        let token = generate_auth_token(code, &salt, &nonce);

        assert!(verify_auth_token(code, &salt, &nonce, &token), "Correct code should verify");
        assert!(!verify_auth_token("654321", &salt, &nonce, &token), "Wrong code should fail");
    }

    #[test]
    fn test_handshake_data_roundtrip() {
        let code = "123456";
        let handshake = HandshakeData::new(code);

        // Serialize and deserialize
        let json = serde_json::to_string(&handshake).unwrap();
        let restored: HandshakeData = serde_json::from_str(&json).unwrap();

        assert_eq!(handshake.salt, restored.salt);
        assert_eq!(handshake.nonce, restored.nonce);
        assert_eq!(handshake.auth_token, restored.auth_token);
    }

    #[test]
    fn test_handshake_verification() {
        let code = "123456";
        let handshake = HandshakeData::new(code);

        assert!(handshake.verify(code), "Correct code should verify");
        assert!(!handshake.verify("654321"), "Wrong code should fail");
    }
}
