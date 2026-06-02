# Contributing

Thanks for your interest in contributing to `password-copy`!

## Prerequisites

- Rust (stable, edition 2024)
- A YubiKey Bio (USB-A) with an enrolled fingerprint for manual testing
- Linux with Wayland (`wl-clipboard` installed) or macOS

### Build dependencies (Linux/Fedora)

```sh
sudo dnf install gcc systemd-devel
```

### Build dependencies (macOS)

Xcode Command Line Tools (`xcode-select --install`).

## Building

```sh
cargo build --release
```

## Project structure

This is intentionally a single-file project. All logic lives in `src/main.rs`.
Please keep it that way unless there is a compelling reason to split.

## Code style

- US English for all code, comments, and documentation
- Explicit imports (no globs, no `use crate::*`)
- Minimal dependencies — do not add crates for things achievable in a few lines
- `anyhow` for error handling (no custom error types needed)
- No `unsafe` beyond the existing `mlock` and `zeroize` usage unless strictly
  necessary and well-justified

## Security rules

This is a security-sensitive project. Any contribution must:

- **Zeroize** all sensitive data (keys, plaintext passwords) immediately after
  use
- **Never** log, print, or expose secrets to stdout/stderr
- **Never** introduce network access, IPC, or any form of remote communication
- Keep the data file permissions at 0600
- Prefer `mlock` for buffers holding secrets
- Do not weaken the existing threat model

## Testing

There are no automated tests (the tool requires physical YubiKey interaction).
Manual testing procedure:

```sh
# Clean slate
rm -f ~/.local/share/com.christiangrete.password

# Setup flow
cargo run --release
# → Enter a test password, touch YubiKey twice

# Decrypt flow
cargo run --release
# → Touch YubiKey once, verify clipboard contains the password

# Verify clipboard clears after 5 seconds
```

## Commit messages

- Use imperative mood ("Add feature" not "Added feature")
- Keep the subject line under 72 characters
- US English

## Pull requests

- One logical change per PR
- Describe *what* and *why*, not *how*
- Ensure `cargo clippy` and `cargo check` pass without warnings
