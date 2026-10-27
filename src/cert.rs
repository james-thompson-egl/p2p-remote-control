//! Certificate-based authentication module
//!
//! Implements:
//! - Self-signed certificate generation
//! - Certificate storage in %APPDATA%
//! - Certificate validation
//! - Configurable validity periods

use rcgen::{CertificateParams, DistinguishedName, DnType, KeyPair};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{info, error, warn};

/// Certificate storage directory
fn cert_dir() -> PathBuf {
    let app_data = std::env::var("APPDATA")
        .unwrap_or_else(|_| "C:\\Users\\Public\\AppData".to_string());
    PathBuf::from(app_data).join("p2p-remote-control")
}

/// Get path to client certificate
pub fn cert_path() -> PathBuf {
    cert_dir().join("client.crt")
}

/// Get path to client private key
pub fn key_path() -> PathBuf {
    cert_dir().join("client.key")
}

/// Get path to trusted host certificate
pub fn trusted_cert_path() -> PathBuf {
    cert_dir().join("trusted.crt")
}

/// Generate a self-signed certificate for client authentication
pub fn generate_client_cert(validity_days: u64) -> Result<(), Box<dyn std::error::Error>> {
    let dir = cert_dir();
    fs::create_dir_all(&dir)?;

    let mut params = CertificateParams::default();

    // Generate distinguished name
    let mut dn = DistinguishedName::new();
    dn.push(DnType::CommonName, format!("p2p-remote-client-{}",
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs()));
    dn.push(DnType::OrganizationName, "P2P Remote Control");
    params.distinguished_name = dn;

    // Set validity period
    let now = time::OffsetDateTime::now_utc();
    params.not_before = now;
    params.not_after = now + time::Duration::days(validity_days as i64);

    // Generate key pair and certificate
    let key_pair = KeyPair::generate()?;
    let cert = params.self_signed(&key_pair)?;
    let cert_pem = cert.pem();
    let key_pem = key_pair.serialize_pem();

    // Save certificate and key
    let cert_file = cert_path();
    let key_file = key_path();

    fs::write(&cert_file, &cert_pem)?;
    fs::write(&key_file, &key_pem)?;

    info!("Generated client certificate valid for {} days", validity_days);
    info!("Certificate: {:?}", cert_file);
    info!("Private key: {:?}", key_file);

    Ok(())
}

/// Generate a host certificate (for servers that need to identify themselves)
pub fn generate_host_cert(hostname: &str, validity_days: u64) -> Result<(), Box<dyn std::error::Error>> {
    let dir = cert_dir();
    fs::create_dir_all(&dir)?;

    let mut params = CertificateParams::default();

    let mut dn = DistinguishedName::new();
    dn.push(DnType::CommonName, format!("p2p-remote-host-{}", hostname));
    dn.push(DnType::OrganizationName, "P2P Remote Control");
    params.distinguished_name = dn;

    // Add subject alternative name for hostname
    params.subject_alt_names = vec![
        rcgen::SanType::DnsName(hostname.to_string().try_into()?)
    ];

    let now = time::OffsetDateTime::now_utc();
    params.not_before = now;
    params.not_after = now + time::Duration::days(validity_days as i64);

    // Generate key pair and certificate
    let key_pair = KeyPair::generate()?;
    let cert = params.self_signed(&key_pair)?;
    let cert_pem = cert.pem();
    let key_pem = key_pair.serialize_pem();

    // Save to host-specific files
    let host_cert_dir = dir.join("hosts").join(hostname);
    fs::create_dir_all(&host_cert_dir)?;

    fs::write(host_cert_dir.join("cert.pem"), &cert_pem)?;
    fs::write(host_cert_dir.join("key.pem"), &key_pem)?;

    info!("Generated host certificate for {}: {:?}", hostname, host_cert_dir);

    Ok(())
}

/// Load client certificate and key
pub fn load_client_cert() -> Result<(String, String), Box<dyn std::error::Error>> {
    let cert = fs::read_to_string(cert_path())?;
    let key = fs::read_to_string(key_path())?;
    Ok((cert, key))
}

/// Load host certificate and key
pub fn load_host_cert(hostname: &str) -> Result<(String, String), Box<dyn std::error::Error>> {
    let host_cert_dir = cert_dir().join("hosts").join(hostname);
    let cert = fs::read_to_string(host_cert_dir.join("cert.pem"))?;
    let key = fs::read_to_string(host_cert_dir.join("key.pem"))?;
    Ok((cert, key))
}

/// Verify a certificate is not expired
pub fn verify_cert_not_expired(cert_pem: &str) -> Result<bool, Box<dyn std::error::Error>> {
    // Parse PEM to get DER bytes
    let pem = pem::parse(cert_pem.as_bytes())?;
    let (_, cert) = x509_parser::parse_x509_certificate(pem.contents())?;
    let now = time::OffsetDateTime::now_utc();

    // Convert ASN1Time to OffsetDateTime for comparison
    let not_before = cert.validity().not_before.to_datetime();
    let not_after = cert.validity().not_after.to_datetime();

    if now < not_before || now > not_after {
        warn!("Certificate has expired or is not yet valid");
        return Ok(false);
    }

    Ok(true)
}

/// Get certificate info (subject, expiry)
pub fn get_cert_info(cert_pem: &str) -> Result<String, Box<dyn std::error::Error>> {
    // Parse PEM to get DER bytes
    let pem = pem::parse(cert_pem.as_bytes())?;
    let (_, cert) = x509_parser::parse_x509_certificate(pem.contents())?;

    let subject = cert.subject();
    let not_before = cert.validity().not_before.to_datetime();
    let not_after = cert.validity().not_after.to_datetime();

    Ok(format!(
        "Subject: {:?}\nValid from: {}\nValid until: {}",
        subject, not_before, not_after
    ))
}

/// Trust a certificate (copy to trusted list)
pub fn trust_cert(cert_pem: &str) -> Result<(), Box<dyn std::error::Error>> {
    let dir = cert_dir();
    fs::create_dir_all(&dir)?;

    fs::write(trusted_cert_path(), cert_pem)?;
    info!("Trusted certificate saved");

    Ok(())
}

/// Check if a certificate is trusted
pub fn is_cert_trusted(cert_pem: &str) -> Result<bool, Box<dyn std::error::Error>> {
    let trusted_path = trusted_cert_path();
    if !trusted_path.exists() {
        return Ok(false);
    }

    let trusted = fs::read_to_string(&trusted_path)?;
    // Normalize by parsing and re-serializing
    let trusted_pem = pem::parse(trusted.as_bytes())?;
    let cert_pem_parsed = pem::parse(cert_pem.as_bytes())?;
    Ok(trusted_pem.contents() == cert_pem_parsed.contents())
}

/// List all certificates in the store
pub fn list_certs() -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let dir = cert_dir();
    if !dir.exists() {
        return Ok(vec![]);
    }

    let mut certs = vec![];

    // Check for client cert
    if cert_path().exists() {
        certs.push("client.crt".to_string());
    }

    // Check for host certs
    let hosts_dir = dir.join("hosts");
    if hosts_dir.exists() {
        for entry in fs::read_dir(&hosts_dir)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();
            certs.push(format!("host:{}", name));
        }
    }

    Ok(certs)
}

/// Revoke certificate by deleting it
pub fn revoke_cert(cert_type: &str) -> Result<(), Box<dyn std::error::Error>> {
    match cert_type {
        "client" => {
            if cert_path().exists() {
                fs::remove_file(cert_path())?;
            }
            if key_path().exists() {
                fs::remove_file(key_path())?;
            }
            info!("Revoked client certificate");
        }
        "trusted" => {
            if trusted_cert_path().exists() {
                fs::remove_file(trusted_cert_path())?;
            }
            info!("Revoked trusted certificate");
        }
        host if host.starts_with("host:") => {
            let hostname = &host[5..];
            let host_dir = cert_dir().join("hosts").join(hostname);
            if host_dir.exists() {
                fs::remove_dir_all(&host_dir)?;
            }
            info!("Revoked host certificate for {}", hostname);
        }
        _ => {
            error!("Unknown certificate type: {}", cert_type);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_and_verify_cert() {
        // Generate a temporary cert
        let _ = generate_client_cert(30);

        // Verify it exists
        assert!(cert_path().exists());
        assert!(key_path().exists());

        // Load and verify
        let (cert, key) = load_client_cert().unwrap();
        assert!(!cert.is_empty());
        assert!(!key.is_empty());

        // Check expiry
        let expired = verify_cert_not_expired(&cert).unwrap();
        assert!(expired); // Should not be expired

        // Get info
        let info = get_cert_info(&cert).unwrap();
        assert!(info.contains("Subject"));
    }
}
