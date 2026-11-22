//! Where updates come from and how one is put in place. `app::update` decides when to check and
//! when to install; this does the checking, downloading and installing, with
//! cargo-packager-updater (the updater that goes with the cargo-packager that makes the
//! installers).
//!
//! The release workflow attaches a small signed manifest per platform to every release,
//! `update-<os>-<arch>.json`, and the app reads it from `releases/latest`. Drafts are never
//! "latest", so a release reaches running copies when it is published. Every download is
//! checked against `PUBKEY` before it is installed.
use std::path::Path;

/// The updater's public key (minisign, base64): `scripts/set-updater-secrets.sh` makes the pair
/// and prints this half. Empty: this build never updates itself.
pub const PUBKEY: &str = "";

/// `{{target}}` is `macos`, `windows` or `linux`; `{{arch}}` is `x86_64` or `aarch64`.
const ENDPOINT: &str = "https://github.com/turbodoc-org/turbodoc-desktop/releases/latest/download/update-{{target}}-{{arch}}.json";

/// Where a copy that can't update itself sends the user.
pub const RELEASES: &str = "https://github.com/turbodoc-org/turbodoc-desktop/releases/latest";

/// How this copy can be updated.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channel {
    /// The updater replaces this copy: a macOS app, the per-user Windows install (the setup
    /// .exe), or an AppImage.
    InPlace,
    /// Something else owns this copy (the .deb, the .msi, an app run from its disk image): say a
    /// new version is out, and link to it.
    Download,
    /// No updates: headless, a development build, or a build without a key.
    Off,
}

pub fn channel() -> Channel {
    if rune::is_headless() || PUBKEY.is_empty() {
        return Channel::Off;
    }
    // `cargo run` never updates itself, unless pointed at a test server.
    if cfg!(debug_assertions) && override_endpoint().is_none() {
        return Channel::Off;
    }
    match std::env::current_exe() {
        Ok(exe) => install_kind(&exe),
        Err(_) => Channel::Off,
    }
}

#[cfg(target_os = "macos")]
fn install_kind(exe: &Path) -> Channel {
    let in_bundle = exe.parent().and_then(Path::parent).is_some_and(|c| c.ends_with("Contents"));
    if !in_bundle {
        Channel::Off
    } else if exe.starts_with("/Volumes") {
        // Opened from the disk image, which is read-only: it has to be copied out first.
        Channel::Download
    } else {
        Channel::InPlace
    }
}

#[cfg(target_os = "windows")]
fn install_kind(exe: &Path) -> Channel {
    // The setup .exe installs per user, under %LOCALAPPDATA%; the .msi installs elsewhere, and
    // the setup .exe would put a second copy beside it.
    match std::env::var_os("LOCALAPPDATA") {
        Some(local) if exe.starts_with(local) => Channel::InPlace,
        _ => Channel::Download,
    }
}

#[cfg(target_os = "linux")]
fn install_kind(exe: &Path) -> Channel {
    if std::env::var_os("APPIMAGE").is_some() {
        Channel::InPlace
    } else if exe.starts_with("/usr") {
        Channel::Download // the .deb: apt owns it
    } else {
        Channel::Off
    }
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn install_kind(_: &Path) -> Channel { Channel::Off }

/// `TURBODOC_UPDATE_URL`: another manifest, with the same placeholders, to try an update
/// against a local server.
fn override_endpoint() -> Option<String> { std::env::var("TURBODOC_UPDATE_URL").ok().filter(|u| !u.is_empty()) }

fn endpoint() -> String { override_endpoint().unwrap_or_else(|| ENDPOINT.to_owned()) }

/// A version newer than this one.
pub struct Found {
    pub version: String,
    pub notes: String,
    /// The updater's handle on it; `None` on the `Download` channel.
    update: Option<cargo_packager_updater::Update>,
}

/// Asks for a newer version. Blocking: call it on a worker.
pub fn check(channel: Channel) -> Result<Option<Found>, String> {
    match channel {
        Channel::Off => Ok(None),
        Channel::InPlace => {
            let config = cargo_packager_updater::Config {
                endpoints: vec![endpoint().parse().map_err(|e| format!("bad update address: {e}"))?],
                pubkey: PUBKEY.into(),
                ..Default::default()
            };
            let current = env!("CARGO_PKG_VERSION").parse().map_err(|e| format!("{e}"))?;
            let update = cargo_packager_updater::check_update(current, config).map_err(|e| e.to_string())?;
            Ok(update.map(|u| Found { version: u.version.to_string(), notes: u.body.clone().unwrap_or_default(), update: Some(u) }))
        }
        Channel::Download => check_manifest(),
    }
}

/// The `Download` channel only reads the manifest's version: nothing is downloaded or installed.
fn check_manifest() -> Result<Option<Found>, String> {
    let url = endpoint().replace("{{target}}", std::env::consts::OS).replace("{{arch}}", std::env::consts::ARCH).replace("{{current_version}}", env!("CARGO_PKG_VERSION"));
    let (status, body) = crate::net::http::Request::new(crate::net::http::Method::Get, &url).send_raw().map_err(|e| e.to_string())?;
    if status == 204 || status == 404 {
        return Ok(None); // nothing published for this platform yet
    }
    crate::net::http::check(status, &body).map_err(|e| e.to_string())?;
    let manifest: serde_json::Value = serde_json::from_str(&body).map_err(|e| e.to_string())?;
    let version = manifest.get("version").and_then(|v| v.as_str()).unwrap_or_default().trim_start_matches('v').to_owned();
    if !newer(&version, env!("CARGO_PKG_VERSION")) {
        return Ok(None);
    }
    let notes = manifest.get("notes").and_then(|v| v.as_str()).unwrap_or_default().to_owned();
    Ok(Some(Found { version, notes, update: None }))
}

/// Whether `candidate` is a later version than `current` (semver; anything unparsable isn't).
pub fn newer(candidate: &str, current: &str) -> bool {
    use cargo_packager_updater::semver::Version;
    match (Version::parse(candidate), Version::parse(current)) {
        (Ok(a), Ok(b)) => a > b,
        _ => false,
    }
}

/// Downloads the update and checks its signature against `PUBKEY`. Blocking.
pub fn download(found: &Found) -> Result<Vec<u8>, String> {
    let update = found.update.as_ref().ok_or("this copy of Turbodoc can't update itself")?;
    update.download().map_err(|e| e.to_string())
}

/// Puts the downloaded update in place: replaces the .app or the AppImage, or runs the Windows
/// installer, which closes this process. Blocking. Pending deletes and saves must be sent first.
pub fn install(found: Found, bytes: Vec<u8>) -> Result<(), String> {
    let update = found.update.ok_or("this copy of Turbodoc can't update itself")?;
    update.install(bytes).map_err(|e| e.to_string())
}

/// Starts the new AppImage, which waits for this process to exit: Rune's relaunch would start
/// `current_exe`, which is inside the old image's mount and goes away with this process. True
/// when it started; the caller then quits. False off Linux or outside an AppImage.
pub fn relaunch_appimage() -> bool {
    #[cfg(target_os = "linux")]
    if let Some(image) = std::env::var_os("APPIMAGE") {
        // Rune's relaunch protocol: the new process waits for this pid before it claims the
        // single instance (rune's quit/relaunch.rs).
        return std::process::Command::new(image)
            .args(std::env::args_os().skip(1))
            .env("RUNE_RELAUNCH_AFTER", std::process::id().to_string())
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .is_ok();
    }
    false
}
