# password-copy

A single-purpose CLI tool that decrypts a stored password via YubiKey Bio
(FIDO2 hmac-secret) and copies it to the system clipboard. Auto-clears after
5 seconds.

## Background

After 22 years on Apple hardware, I switched from a MacBook Pro M1 Pro to a
[Star Labs](https://starlabs.systems) StarFighter Ultra running
[Fedora](https://fedoraproject.org) Linux. No Touch ID — and that's fine. I
picked up a YubiKey Bio to optionally bring back biometric convenience for a
few select use cases.

Turns out KeePassXC doesn't support YubiKey Bio for database unlock. This tool
is a workaround: it securely persists your master password locally, encrypted
by the YubiKey's FIDO2 hmac-secret, so you can unlock your database via
copy & paste instead of typing it every single time.

Use at your own risk depending on your threat model.

## Requirements

- Linux with a Wayland compositor (GNOME, KDE Plasma, Sway, etc.) **or** macOS
- YubiKey Bio (USB-A or USB-C) with enrolled fingerprint
- `wl-clipboard` on Linux (provides `wl-copy`) — pre-installed `pbcopy` is used on macOS

Install runtime dependencies (Linux only):

```sh
sudo dnf install wl-clipboard
```

## Installation

Download the binary and place it in your PATH:

```sh
cp password-copy ~/.local/bin/
```

### USB access (Linux only)

If you get a "Permission denied" error when accessing the YubiKey, create a udev
rule:

```sh
sudo tee /etc/udev/rules.d/70-yubikey.rules <<'EOF'
KERNEL=="hidraw*", SUBSYSTEM=="hidraw", ATTRS{idVendor}=="1050", MODE="0660", TAG+="uaccess"
EOF
sudo udevadm control --reload-rules
sudo udevadm trigger
```

Then re-plug your YubiKey.

## Usage

### First run (setup)

```sh
password-copy
```

1. You will be prompted to enter the password you want to store.
2. Touch your YubiKey (fingerprint) twice — once to create the credential, once
   to verify it.
3. The encrypted password is saved to
   `~/.local/share/com.christiangrete.password` (Linux) or
   `~/Library/Application Support/com.christiangrete.password` (macOS).

### Subsequent runs (decrypt + clipboard)

```sh
password-copy
```

1. Touch your YubiKey (fingerprint) once.
2. The password is copied to your clipboard.
3. The clipboard is automatically cleared after 5 seconds.

### Reset

Delete the stored data file to start over:

```sh
# Linux
rm ~/.local/share/com.christiangrete.password

# macOS
rm ~/Library/Application\ Support/com.christiangrete.password
```

## How it works

- Your password is encrypted with AES-256-GCM.
- The encryption key is derived from the YubiKey's FIDO2 hmac-secret extension
  (32 bytes of full entropy — not a password-derived key).
- Every access requires a biometric (fingerprint) touch on the YubiKey.
- The key and plaintext password are zeroized in RAM immediately after use.
- Sensitive memory is locked (`mlock`) to prevent swapping to disk.
- The data file is restricted to owner-only access (mode 0600).
- No network, no daemon, no config files.

## License

MIT
