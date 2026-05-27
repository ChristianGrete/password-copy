# AGENTS.md

## Project overview

`password-copy` is a single-purpose Rust CLI tool for Linux and macOS that
decrypts a stored password via YubiKey Bio (FIDO2 hmac-secret extension) and
copies it to the system clipboard. Auto-clears after 10 seconds.

- Author: Christian Grete <webmaster@christiangrete.com>
- Platform: Linux (Wayland) and macOS
- Binary location: `~/.local/bin/password-copy`
- Data file: platform data dir + `com.christiangrete.password`
  - Linux: `~/.local/share/com.christiangrete.password`
  - macOS: `~/Library/Application Support/com.christiangrete.password`
- YubiKey: YubiKey Bio (USB-A), FIDO2 with hmac-secret extension

## Architecture

Single binary, single file (`src/main.rs`), no subcommands, no flags, no config.

**Flow:**
1. If data file missing → setup (prompt password, create FIDO2 credential, encrypt, store)
2. If data file exists → decrypt (FIDO2 hmac-secret, AES-256-GCM, wl-copy, auto-clear 10s)
3. Reset → user deletes `~/.local/share/com.christiangrete.password` manually

**Crypto:**
- FIDO2 hmac-secret via `ctap-hid-fido2` crate (pure Rust, USB HID)
- Encryption: AES-256-GCM, key = 32-byte hmac-secret output
- Serialization: CBOR via `ciborium`

**Hardening:**
- `zeroize`: key and plaintext password are zeroed in RAM immediately after use
- `mlock`: sensitive buffers are locked to prevent swap-out
- Data file permissions: 0600 (owner read/write only)

## Build commands

```sh
cargo build --release
cp target/release/password-copy ~/.local/bin/
```

## System dependencies

Required for building on Linux (Fedora):

```sh
sudo dnf install gcc systemd-devel
```

On macOS, Xcode Command Line Tools are sufficient (no extra packages).

Required at runtime on Linux:

```sh
sudo dnf install wl-clipboard
```

On macOS, `pbcopy` is pre-installed.

If USB HID access fails as non-root on Linux, add a udev rule:

```sh
# /etc/udev/rules.d/70-yubikey.rules
KERNEL=="hidraw*", SUBSYSTEM=="hidraw", ATTRS{idVendor}=="1050", MODE="0660", TAG+="uaccess"
```

## Code style

- Rust edition 2024
- All code in `src/main.rs` (intentionally single-file)
- Comments, code, docs: US English
- Minimal, no over-engineering
- `anyhow` for error handling
- Explicit imports (no glob imports, no `use crate::*`)

## Testing

No automated tests. Manual testing only:

```sh
# Setup (first run, no data file)
rm -f ~/.local/share/com.christiangrete.password
password-copy

# Decrypt + clipboard (subsequent runs)
password-copy
# or alias:
pc
```

## Security considerations

- Password encrypted at rest, key derived from YubiKey hmac-secret
- Key only exists in RAM during decrypt, zeroized immediately after
- YubiKey biometric (fingerprint) required for every access
- Sensitive memory pages locked with `mlock` (prevents swap)
- Data file restricted to mode 0600
- No network, no daemon, no state beyond the single encrypted file
- Clipboard auto-clears after 10 seconds
