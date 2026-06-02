use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;
use std::thread;
use std::time::Duration;

use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{Aes256Gcm, Nonce};
use anyhow::{bail, Context, Result};
use ctap_hid_fido2::fidokey::get_assertion::get_assertion_params::Extension as AssertionExt;
use ctap_hid_fido2::fidokey::get_assertion::GetAssertionArgsBuilder;
use ctap_hid_fido2::fidokey::make_credential::make_credential_params::Extension as CredentialExt;
use ctap_hid_fido2::fidokey::make_credential::MakeCredentialArgsBuilder;
use ctap_hid_fido2::{Cfg, FidoKeyHidFactory};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

const RP_ID: &str = "com.christiangrete.password-copy";
const DATA_FILE: &str = "com.christiangrete.password";
const CLIPBOARD_CLEAR_SECONDS: u64 = 5;

#[derive(Serialize, Deserialize)]
struct StoredBlob {
    credential_id: Vec<u8>,
    salt: [u8; 32],
    nonce: [u8; 12],
    ciphertext: Vec<u8>,
}

fn data_file_path() -> Result<PathBuf> {
    let data_dir = dirs::data_dir().context("Could not determine data directory")?;
    Ok(data_dir.join(DATA_FILE))
}

fn get_hmac_secret(credential_id: &[u8], salt: &[u8; 32]) -> Result<Vec<u8>> {
    let device = FidoKeyHidFactory::create(&Cfg::init())
        .context("Could not find a FIDO2 device. Is your YubiKey plugged in?")?;

    let assertion = GetAssertionArgsBuilder::new(RP_ID, &[0u8; 32])
        .credential_id(credential_id)
        .extensions(&[AssertionExt::HmacSecret(Some(*salt))])
        .build();

    let assertions = device
        .get_assertion_with_args(&assertion)
        .context("FIDO2 assertion failed. Touch your YubiKey.")?;

    let first = assertions
        .first()
        .context("No assertion returned from device")?;

    for ext in &first.extensions {
        if let AssertionExt::HmacSecret(Some(secret)) = ext {
            return Ok(secret.to_vec());
        }
    }

    bail!("Device did not return hmac-secret")
}

fn setup() -> Result<()> {
    println!("password-copy: no stored password found. setting up.");
    print!("password-copy: enter password to store: ");
    std::io::stdout().flush()?;
    let mut password =
        rpassword::read_password().context("Failed to read password")?;

    // Lock password memory to prevent swapping
    mlock(password.as_bytes());

    if password.is_empty() {
        bail!("Password must not be empty");
    }

    // Generate random salt
    let mut salt = [0u8; 32];
    OsRng.fill_bytes(&mut salt);

    // Create FIDO2 credential with hmac-secret
    let device = FidoKeyHidFactory::create(&Cfg::init())
        .context("Could not find a FIDO2 device. Is your YubiKey plugged in?")?;

    let make_cred = MakeCredentialArgsBuilder::new(RP_ID, &[0u8; 32])
        .extensions(&[CredentialExt::HmacSecret(Some(true))])
        .build();

    let credential = device
        .make_credential_with_args(&make_cred)
        .context("Failed to create FIDO2 credential")?;

    let credential_id = credential.credential_descriptor.id.clone();

    // Verify we can get the hmac-secret back
    let mut secret = get_hmac_secret(&credential_id, &salt)?;
    mlock(&secret);

    if secret.len() != 32 {
        secret.zeroize();
        bail!("Expected 32-byte hmac-secret, got {} bytes", secret.len());
    }

    // Encrypt password
    let key = aes_gcm::Key::<Aes256Gcm>::from_slice(&secret);
    let cipher = Aes256Gcm::new(key);

    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, password.as_bytes())
        .map_err(|e| anyhow::anyhow!("Encryption failed: {e}"))?;

    // Zeroize sensitive data immediately after use
    password.zeroize();
    secret.zeroize();

    // Store blob
    let blob = StoredBlob {
        credential_id,
        salt,
        nonce: nonce_bytes,
        ciphertext,
    };

    let path = data_file_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("Failed to create data directory")?;
    }

    let file = fs::File::create(&path).context("Failed to create data file")?;
    ciborium::into_writer(&blob, &file).context("Failed to write data file")?;

    // Restrict file permissions to owner only (0600)
    file.set_permissions(fs::Permissions::from_mode(0o600))
        .context("Failed to set data file permissions")?;

    println!("password-copy: credential stored.");
    println!("  path: {}", path.display());
    Ok(())
}

fn decrypt_and_copy() -> Result<()> {
    let path = data_file_path()?;
    let file = fs::File::open(&path).context("Failed to open data file")?;
    let blob: StoredBlob = ciborium::from_reader(file).context("Failed to parse data file")?;

    let mut secret = get_hmac_secret(&blob.credential_id, &blob.salt)?;
    mlock(&secret);

    // Decrypt password
    let key = aes_gcm::Key::<Aes256Gcm>::from_slice(&secret);
    let cipher = Aes256Gcm::new(key);
    let nonce = Nonce::from_slice(&blob.nonce);

    let mut plaintext = cipher
        .decrypt(nonce, blob.ciphertext.as_ref())
        .map_err(|_| anyhow::anyhow!("Decryption failed. Wrong key or corrupted data."))?;

    // Zeroize key immediately after decryption
    secret.zeroize();

    let mut password = String::from_utf8(plaintext.clone())
        .context("Decrypted data is not valid UTF-8")?;
    plaintext.zeroize();
    mlock(unsafe { password.as_bytes_mut() });

    // Copy to clipboard
    copy_to_clipboard(&password)?;

    // Zeroize password after clipboard copy
    password.zeroize();

    // Spawn background process to clear clipboard after timeout (fallback if user hits Ctrl+C)
    clear_clipboard_after(CLIPBOARD_CLEAR_SECONDS)?;

    // Visual countdown in foreground
    countdown(CLIPBOARD_CLEAR_SECONDS);

    // Clear clipboard now (foreground path completed)
    clear_clipboard()?;

    print!("\rpassword-copy: clipboard cleared.{}", " ".repeat(20));
    println!();
    Ok(())
}

fn copy_to_clipboard(text: &str) -> Result<()> {
    let cmd = if cfg!(target_os = "macos") {
        "pbcopy"
    } else {
        "wl-copy"
    };

    let mut child = Command::new(cmd)
        .stdin(std::process::Stdio::piped())
        .spawn()
        .with_context(|| format!("Failed to start {cmd}. Is it installed?"))?;

    child
        .stdin
        .take()
        .unwrap()
        .write_all(text.as_bytes())
        .with_context(|| format!("Failed to write to {cmd} stdin"))?;

    child.wait().with_context(|| format!("{cmd} exited with error"))?;
    Ok(())
}

fn clear_clipboard_after(seconds: u64) -> Result<()> {
    let clear_cmd = if cfg!(target_os = "macos") {
        format!("sleep {seconds} && echo -n | pbcopy")
    } else {
        format!("sleep {seconds} && wl-copy --clear")
    };

    Command::new("sh")
        .arg("-c")
        .arg(&clear_cmd)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .context("Failed to spawn clipboard clear process")?;

    Ok(())
}

fn clear_clipboard() -> Result<()> {
    if cfg!(target_os = "macos") {
        Command::new("sh")
            .arg("-c")
            .arg("echo -n | pbcopy")
            .status()
            .context("Failed to clear clipboard")?;
    } else {
        Command::new("wl-copy")
            .arg("--clear")
            .status()
            .context("Failed to clear clipboard")?;
    }
    Ok(())
}

fn countdown(seconds: u64) {
    let bar_width = 20;
    for i in 0..seconds {
        let remaining = seconds - i;
        let filled = bar_width - (i as usize * bar_width / seconds as usize);
        let empty = bar_width - filled;
        print!(
            "\rpassword-copy: clearing in {:>2}s [{}{}]",
            remaining,
            "\u{2588}".repeat(filled),
            "\u{2591}".repeat(empty),
        );
        let _ = std::io::stdout().flush();
        thread::sleep(Duration::from_secs(1));
    }
}

/// Lock a memory region to prevent it from being swapped to disk.
/// Best-effort: failure is silently ignored (may require elevated RLIMIT_MEMLOCK).
fn mlock(buf: &[u8]) {
    unsafe {
        libc::mlock(buf.as_ptr().cast(), buf.len());
    }
}

fn main() {
    if let Err(e) = run() {
        eprintln!("password-copy: {e:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let path = data_file_path()?;

    if path.exists() {
        decrypt_and_copy()
    } else {
        setup()
    }
}
