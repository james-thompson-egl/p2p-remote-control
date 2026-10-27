//! Integration tests for P2P Remote Control
//!
//! Tests that don't require actual WebRTC connections

use std::process::Command;
use std::env;

#[test]
fn test_help_command() {
    let current_dir = env::current_dir().expect("Failed to get current directory");
    let binary_path = current_dir.join("target/debug/p2p-remote-control.exe");

    let output = Command::new(binary_path.to_str().unwrap())
        .arg("--help")
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}\n{}", stdout, stderr);
    assert!(combined.contains("Usage"));
    assert!(combined.contains("--host"));
    assert!(combined.contains("--connect"));
}

#[test]
fn test_invalid_option() {
    let current_dir = env::current_dir().expect("Failed to get current directory");
    let binary_path = current_dir.join("target/debug/p2p-remote-control.exe");

    let output = Command::new(binary_path.to_str().unwrap())
        .arg("--invalid")
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}\n{}", stdout, stderr);

    // Should show usage or error message
    assert!(combined.contains("Unknown option") || combined.contains("Usage"));
}
