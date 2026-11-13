//! The signed-in session lives in the system keychain (Keychain on macOS, Credential Manager on
//! Windows, the Secret Service on Linux), never in a plain file. `TURBODOC_INSECURE_SESSION=1`
//! keeps it in the app's data folder instead, for unsigned development builds whose every
//! rebuild the keychain would otherwise ask about.
use std::path::PathBuf;

const SERVICE: &str = "ai.turbodoc.desktop";
const ACCOUNT: &str = "session";

fn insecure() -> Option<PathBuf> {
    if std::env::var_os("TURBODOC_INSECURE_SESSION").is_some_and(|v| v != "0") || rune::is_headless() {
        let dirs = rune::dirs::AppDirs::new(crate::APP_ID)?;
        return Some(dirs.data().join("session.json"));
    }
    None
}

pub fn load() -> Option<String> {
    if let Some(path) = insecure() {
        return std::fs::read_to_string(path).ok();
    }
    keyring::Entry::new(SERVICE, ACCOUNT).ok()?.get_password().ok()
}

pub fn save(secret: &str) -> Result<(), String> {
    if let Some(path) = insecure() {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        return write_private(&path, secret);
    }
    keyring::Entry::new(SERVICE, ACCOUNT).and_then(|e| e.set_password(secret)).map_err(|e| e.to_string())
}

pub fn clear() {
    if let Some(path) = insecure() {
        let _ = std::fs::remove_file(path);
        return;
    }
    if let Ok(entry) = keyring::Entry::new(SERVICE, ACCOUNT) {
        let _ = entry.delete_credential();
    }
}

#[cfg(unix)]
fn write_private(path: &std::path::Path, data: &str) -> Result<(), String> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut f = std::fs::OpenOptions::new().create(true).write(true).truncate(true).mode(0o600).open(path).map_err(|e| e.to_string())?;
    f.write_all(data.as_bytes()).map_err(|e| e.to_string())
}

#[cfg(not(unix))]
fn write_private(path: &std::path::Path, data: &str) -> Result<(), String> {
    std::fs::write(path, data).map_err(|e| e.to_string())
}
