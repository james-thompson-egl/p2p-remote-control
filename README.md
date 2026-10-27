# P2P Remote Control

Secure, serverless, peer-to-peer remote desktop control for Windows. Built with Rust and WebRTC.

## Features

| Feature | Status |
|---------|--------|
| **Serverless P2P** | Manual SDP exchange via copy-paste, no infrastructure |
| **End-to-end Encryption** | WebRTC DTLS-SRTP + Argon2 key derivation |
| **No Admin Required** | User-mode screen capture + input injection |
| **Portable** | Single binary, no installation |
| **AV-Friendly** | Clean code, no packers, no obfuscation |
| **H.264/VP8 Video** | Hardware-accelerated encoding |
| **DXGI Capture** | GPU-accelerated screen capture (30-60fps) |
| **Tauri GUI** | Modern viewer interface |

## Quick Start

### Prerequisites

- Windows 10/11
- Rust toolchain (see below for installation)
- No admin rights required to run

### Install Rust

```bash
# Download and run rustup installer (no admin required)
winget install Rustlang.Rustup
# or download from https://rustup.rs/
```

### Build from Source

```bash
# Clone the repository
git clone https://github.com/jthompson-p2p/p2p-remote-control.git
cd p2p-remote-control

# Build release binary
cargo build --release

# The binary will be at:
# target\release\p2p-remote-control.exe
```

### Download Pre-built Binary

Pre-built binaries are available in the [Releases](https://github.com/jthompson-p2p/p2p-remote-control/releases) section. Simply download and run - no installation required.

### Usage

**On the machine to be controlled (Host):**

```bash
# Generate a 6-digit pairing code and display SDP offer
.\target\release\p2p-remote-control.exe --host
```

The host will display:
1. A 6-digit pairing code
2. An SDP offer (copy this)

**On the controlling machine (Client/Viewer):**

```bash
# Enter the pairing code and paste the SDP offer from host
.\target\release\p2p-remote-control.exe --connect 123456
```

The client will:
1. Generate an SDP answer (copy this)
2. Display it for you to paste back into the host

**Back on the Host:**

Paste the SDP answer from the client to establish the connection.

### Connection Flow

```
1. Host runs: p2p-remote-control --host
   → Shows pairing code: 123456
   → Shows SDP offer (copy this)

2. Client runs: p2p-remote-control --connect 123456
   → Paste the SDP offer from host
   → Shows SDP answer (copy this)

3. Host pastes the SDP answer from client
   → Connection established!
   → Screen sharing begins
```

See [QUICKSTART.md](QUICKSTART.md) for detailed instructions.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      Host Machine                           │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐     │
│  │   DXGI/     │───▶│   WebRTC    │◀──▶│   Input     │     │
│  │   GDI       │    │  Connection │    │  Injector   │     │
│  │  Capture    │    │  (P2P)      │    │  (SendInput)│     │
│  └─────────────┘    └──────┬──────┘    └─────────────┘     │
│                            │                                │
└────────────────────────────┼────────────────────────────────┘
                             │
                    ┌────────▼────────┐
                    │  WebRTC P2P     │
                    │  (Direct, No    │
                    │   Server)       │
                    └────────┬────────┘
                             │
┌────────────────────────────┼────────────────────────────────┐
│                    Viewer Machine                           │
│  ┌─────────────┐    ┌──────▼──────┐    ┌─────────────┐     │
│  │   Tauri     │◀───│   WebRTC    │───▶│  Input      │     │
│  │   GUI       │    │  Client     │    │  Capture    │     │
│  └─────────────┘    └─────────────┘    └─────────────┘     │
└─────────────────────────────────────────────────────────────┘
```

## Security

### Encryption Stack

```
┌─────────────────────────────────────┐
│  Application Layer                  │
│  - Argon2id key derivation          │
│  - HMAC authentication              │
├─────────────────────────────────────┤
│  WebRTC Layer                       │
│  - DTLS 1.2 (key exchange)          │
│  - SRTP (media encryption)          │
├─────────────────────────────────────┤
│  Transport Layer                    │
│  - UDP (with ICE/NAT traversal)     │
└─────────────────────────────────────┘
```

### Security Features

| Threat | Mitigation |
|--------|------------|
| Eavesdropping | WebRTC DTLS-SRTP encryption (end-to-end) |
| MITM | DTLS fingerprint verification |
| Unauthorized Access | 6-digit pairing code + HMAC auth |
| Replay Attacks | Nonce-based handshake |
| Persistence | Ephemeral keys, no stored credentials |

### Pairing Flow

```
1. Host generates 6-digit code + ephemeral keypair
2. Viewer enters code → Argon2id derives shared secret
3. SDP exchange includes:
   - Salt (for key derivation)
   - Nonce (anti-replay)
   - HMAC token (authentication)
4. DTLS handshake verifies fingerprint
5. Connection established → keys discarded
```

## Components

### Core Modules

| Module | Description |
|--------|-------------|
| `screen_capture.rs` | GDI screen capture (fallback, ~15fps) |
| `dxgi_capture.rs` | DXGI Desktop Duplication (GPU, ~60fps) |
| `input_injector.rs` | Win32 SendInput for remote control |
| `pairing.rs` | Secure pairing (Argon2, HMAC, QR) |
| `video_encoder.rs` | H.264/VP8/JPEG encoding |
| `webrtc_conn.rs` | WebRTC P2P connection handling |
| `client_input.rs` | Viewer-side input capture |

### Viewer

| Component | Description |
|-----------|-------------|
| `viewer/src/main.rs` | Tauri application entry |
| `viewer/src/webrtc_client.rs` | WebRTC client logic |
| `frontend/index.html` | GUI interface |

## Build Options

```bash
# Standard build (GDI capture)
cargo build --release

# With DXGI capture (faster)
cargo build --release --features dxgi

# Run with logging
RUST_LOG=debug cargo run --release -- --host
```

## AV Detection Mitigation

### Option A: Self-Sign (Recommended)

```powershell
# Create certificate
$cert = New-SelfSignedCertificate -CertStoreLocation Cert:\CurrentUser\My `
    -DisplayName "P2P Remote Control" -Type CodeSigning

# Export and sign
Export-Certificate -Cert $cert -FilePath p2p-remote.cer
signtool sign /fd SHA256 /f p2p-remote.cer target\release\p2p-remote-control.exe
```

### Option B: Add Exclusion (One-time Admin)

```powershell
Add-MpPreference -ExclusionPath "C:\Users\YourName\Tools\P2PRemote\"
```

### Option C: Build from Source

Building locally is more trusted than downloaded binaries. No packer or obfuscation is used.

## Performance

| Mode | FPS | CPU Usage |
|------|-----|-----------|
| GDI Capture | ~15-20fps | Moderate |
| DXGI Capture | ~30-60fps | Low (GPU-accelerated) |
| H.264 Encoding | ~60fps | Low (hardware) |

## Limitations

### No Admin Mode
- Cannot capture secure desktop (login screen)
- Cannot auto-start (manual launch required)
- Some system hotkeys may not work (Win, Ctrl+Alt+Del)

### Network
- Requires UDP for WebRTC
- STUN server needed for NAT traversal (uses Google's public STUN)

## Roadmap

- [ ] H.264 hardware encoding via MediaFoundation
- [ ] Clipboard synchronization
- [ ] File transfer
- [ ] Multi-monitor support
- [ ] Audio forwarding
- [ ] Mobile viewer (iOS/Android)

## License

MIT
