# Testing Guide

## Quick Tests

### 1. Unit Tests

```bash
# Run all tests
cargo test

# Run pairing tests specifically
cargo test -- pairing

# Run with output
cargo test -- --nocapture
```

### 2. Build Tests

```bash
# Debug build
cargo build

# Release build
cargo build --release

# With DXGI feature
cargo build --release --features dxgi

# Check for errors without building
cargo check
```

### 3. Integration Tests

```bash
# Run test script
bash test.sh

# Or manually:
cargo run -- --help
cargo run -- --invalid  # Should show error
```

## Manual Testing

### Test 1: Help Command

```bash
cargo run -- --help
```

Expected output:
```
P2P Remote Control - Secure, serverless remote desktop

Usage:
  p2p-remote-control.exe --host              Start as host
  p2p-remote-control.exe --connect <code>    Connect to host
```

### Test 2: Host Mode (Terminal)

```bash
# Terminal 1 - Host
cargo run -- --host
```

Expected:
- 6-digit pairing code displayed
- QR code shown (optional)
- SDP offer generated
- Waits for SDP answer

### Test 3: Client Mode (Terminal)

```bash
# Terminal 2 - Client
cargo run -- --connect 123456
```

Expected:
- Prompts for SDP offer
- Generates SDP answer
- Waits for connection

### Test 4: Pairing Code Validation

```rust
// In pairing_test.rs
validate_pairing_code("123456")  // true
validate_pairing_code("12345")   // false (too short)
validate_pairing_code("12345a")  // false (non-numeric)
```

### Test 5: Security Tests

```bash
# Test key derivation
cargo test -- test_key_derivation

# Test HMAC auth
cargo test -- test_auth_token

# Test handshake verification
cargo test -- test_handshake
```

## Test Coverage

| Module | Tests | Coverage |
|--------|-------|----------|
| `pairing.rs` | ✓ | 90%+ |
| `input_injector.rs` | - | Manual only |
| `screen_capture.rs` | - | Manual only |
| `webrtc_conn.rs` | - | Manual only |

## Known Test Limitations

1. **WebRTC tests** require actual network connectivity
2. **Screen capture tests** require a display
3. **Input injection tests** require Windows GUI

## Performance Testing

### Frame Rate Test

```bash
# Run host with logging
RUST_LOG=debug cargo run -- --host 2>&1 | grep "Captured frame"
```

Look for:
- Frame capture times (<50ms for 20fps)
- Consistent frame sizes

### Latency Test

Manual test:
1. Start host
2. Connect client
3. Move mouse on client
4. Measure delay on host screen

Expected: <100ms for local network

## Security Verification

### 1. Check Encryption

```bash
# Verify WebRTC uses DTLS
# (Check webrtc-rs source or Wireshark capture)
```

### 2. Verify No Credentials Stored

```bash
# Check user directories
dir %APPDATA%
dir %LOCALAPPDATA%

# Should not find any stored pairing codes
```

### 3. Test Session Ephemeral

```bash
# Start host, get pairing code
cargo run -- --host

# Kill process

# Start again - should get NEW pairing code
cargo run -- --host
```

## AV Testing

### Windows Defender

```bash
# After building, check status
powershell: Get-MpThreatDetection

# Check if binary was quarantined
powershell: Get-MpThreatCatalog
```

If detected:
1. Add exclusion: `Add-MpPreference -ExclusionPath "C:\path\to\p2p-remote-control"`
2. Or build from source (more trusted)

### Other AV Products

Test on:
- Bitdefender
- Avast
- Kaspersky

## Network Testing

### Local Network

1. Host and client on same WiFi
2. Should connect directly (P2P)

### Different Networks

1. Host on home WiFi
2. Client on cellular/hotspot
3. Should connect via STUN server

### Firewall Testing

```bash
# Block UDP and verify connection fails
# Then allow UDP and verify connection works
```

## Troubleshooting

### "Connection timeout"

1. Check STUN server reachable:
   ```
   ping stun.l.google.com
   ```

2. Check firewall allows UDP

3. Try different STUN server in code

### "Invalid pairing code"

1. Verify exactly 6 digits
2. No spaces or extra characters
3. Case doesn't matter (all numeric)

### "Screen capture failed"

1. Check display connected
2. Try GDI fallback (remove dxgi feature)
3. Check permissions
