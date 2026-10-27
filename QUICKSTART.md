# Quick Start Guide

## Prerequisites

1. Install Rust: https://rustup.rs/
   ```powershell
   winget install Rustlang.Rust
   ```

2. Verify installation:
   ```bash
   rustc --version
   cargo --version
   ```

## Build

```bash
cd p2p-remote-control
cargo build --release
```

Binary location: `target/release/p2p-remote-control.exe`

## Run - Host (Machine to Control)

```bash
# Run host mode
.\target\release\p2p-remote-control.exe --host
```

Output:
```
=== HOST MODE ACTIVE ===
Pairing Code: 123456

To connect:
1. On viewer machine, run: p2p-remote --connect 123456
2. Copy the SDP offer shown below
3. Paste it into the viewer
4. Copy the SDP answer from viewer
5. Paste it here to establish connection

=== SDP OFFER (copy this) ===
v=0
o=- 0 0 IN IP4 127.0.0.1
...
```

## Run - Client (Viewer Machine)

```bash
# Run client mode with pairing code
.\target\release\p2p-remote-control.exe --connect 123456
```

Output:
```
Enter SDP offer from host (paste and press Enter twice when done):
[paste the SDP offer from host here]

=== SDP ANSWER (copy this) ===
v=0
o=- 0 0 IN IP4 127.0.0.1
...
```

Copy the SDP answer and paste it back into the host.

## Connection Established

Once both sides exchange SDP:
- Host starts capturing screen
- Client can send mouse/keyboard input
- Press Ctrl+C on either side to disconnect

## Troubleshooting

### "Invalid pairing code"
- Ensure pairing code is exactly 6 digits
- Check for typos when entering

### "Connection timeout"
- Ensure both machines can reach STUN servers (stun.l.google.com:19302)
- Check firewall settings
- Verify SDP was copied completely (no truncated lines)

### Screen capture slow (~15fps)
- GDI fallback is being used
- DXGI Desktop Duplication requires more permissions
- Consider running with higher privileges if available

### AV detection
- Build from source locally (Option C)
- Add folder exclusion in Windows Security
- Self-sign the binary (see README.md)

## Security Notes

- Pairing code is ephemeral (generated per session)
- No credentials stored on disk
- All traffic encrypted via WebRTC DTLS
- No server involved - pure P2P
