//! Updates, so a security fix reaches everyone who installed KeyHandoff.
//!
//! An installed copy asks GitHub for the latest release's `latest.json` at
//! launch and every few hours; that request, and the download it may lead to,
//! is the app's only network traffic. A package is applied only if it carries
//! a valid signature from the release key, whose public half is built into the
//! app (`tauri.conf.json`); the updater refuses anything else.
//!
//! A release waits 48 hours after it was signed before installed copies take
//! it, so a release the owner didn't make can be pulled first (ADR 0005). The
//! signing time is in the signature itself, so it can't be changed either.
//!
//! A locked vault has nothing open to lose, so while it's locked a Windows or
//! AppImage update installs and restarts at once. Otherwise, and always for a
//! `.deb` (installing one asks for an admin password), the window offers
//! Restart to update. Copies run from source (no bundle type) never update.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;
use tauri::utils::config::BundleType;
use tauri::utils::platform::bundle_type;
use tauri::AppHandle;
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::session::Session;

const CHECK_EVERY: Duration = Duration::from_secs(6 * 60 * 60);
/// While an update waits, how often to see whether the vault has locked.
const INSTALL_CHECK: Duration = Duration::from_secs(60);
/// How long after signing a release installed copies wait before taking it.
const HOLD_SECS: u64 = 48 * 60 * 60;

/// When the release key signed an update: the `timestamp:` in its signature's
/// trusted comment (Unix seconds). Minisign's global signature covers that
/// comment, and `Update::download` checks it before anything installs, so an
/// earlier time to skip the wait fails the download.
fn signed_at(signature: &str) -> Option<u64> {
    use base64::Engine;
    let text = base64::engine::general_purpose::STANDARD.decode(signature.trim()).ok()?;
    let text = String::from_utf8(text).ok()?;
    let comment = text.lines().find_map(|line| line.strip_prefix("trusted comment: "))?;
    comment.split('\t').find_map(|field| field.strip_prefix("timestamp:"))?.parse().ok()
}

/// Whether an update signed at `signed` may install at `now`. A signature
/// without a signing time never does.
fn due(signed: Option<u64>, now: u64) -> bool {
    signed.is_some_and(|t| now >= t.saturating_add(HOLD_SECS))
}

/// An update downloaded and verified, waiting to be installed.
struct Ready {
    update: Update,
    bytes: Vec<u8>,
}

#[derive(Default)]
pub struct Updates {
    ready: Mutex<Option<Ready>>,
}

/// What the window shows: the version waiting, and whether installing it asks
/// for an admin password.
#[derive(Serialize, Clone, Debug)]
pub struct UpdateView {
    pub version: String,
    pub asks_password: bool,
}

/// Installing a `.deb` goes through `pkexec`, so never unasked.
fn asks_password() -> bool {
    matches!(bundle_type(), Some(BundleType::Deb))
}

impl Updates {
    pub fn view(&self) -> Option<UpdateView> {
        let ready = self.ready.lock().unwrap_or_else(|e| e.into_inner());
        ready.as_ref().map(|r| UpdateView { version: r.update.version.clone(), asks_password: asks_password() })
    }

    /// Install the waiting update and restart into it. On Windows the
    /// installer closes the app and starts the new version itself.
    pub fn install(&self, app: &AppHandle) -> Result<(), String> {
        let ready = self.ready.lock().unwrap_or_else(|e| e.into_inner()).take().ok_or("No update is waiting.")?;
        ready.update.install(&ready.bytes).map_err(|e| format!("The update didn't install: {e}"))?;
        app.restart()
    }
}

/// Check now and every few hours; install a waiting update once it's safe.
/// `changed` tells the screens an update is ready.
pub fn watch(app: AppHandle, updates: Arc<Updates>, session: Arc<Session>, changed: fn(&AppHandle)) {
    if bundle_type().is_none() {
        return;
    }
    tauri::async_runtime::spawn(async move {
        loop {
            if updates.view().is_none() {
                match fetch(&app).await {
                    Ok(Some(ready)) => {
                        *updates.ready.lock().unwrap_or_else(|e| e.into_inner()) = Some(ready);
                        changed(&app);
                    }
                    Ok(None) => {}
                    Err(e) => eprintln!("keyhandoff: update check: {e}"),
                }
            }
            let mut waited = Duration::ZERO;
            while waited < CHECK_EVERY {
                if updates.view().is_some() && !asks_password() && session.state().0 == "locked" {
                    if let Err(e) = updates.install(&app) {
                        eprintln!("keyhandoff: {e}");
                    }
                }
                tokio_sleep(INSTALL_CHECK).await;
                waited += INSTALL_CHECK;
            }
        }
    });
}

/// The latest release if it's newer, downloaded and checked against its signature.
async fn fetch(app: &AppHandle) -> Result<Option<Ready>, String> {
    let updater = app.updater().map_err(|e| e.to_string())?;
    let Some(update) = updater.check().await.map_err(|e| e.to_string())? else { return Ok(None) };
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    if !due(signed_at(&update.signature), now) {
        // Checked again at the next look, every few hours.
        return Ok(None);
    }
    let bytes = update.download(|_, _| {}, || {}).await.map_err(|e| e.to_string())?;
    Ok(Some(Ready { update, bytes }))
}

async fn tokio_sleep(d: Duration) {
    let _ = tauri::async_runtime::spawn_blocking(move || std::thread::sleep(d)).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The v0.1.0 .deb's real signature, signed at 1793487511.
    const SIGNATURE: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZSBmcm9tIHRhdXJpIHNlY3JldCBrZXkKUlVSYUtlVEVrN3pVM0pOa2ZNcURNbDZweEhUcmN0NExDanBJNzIySjFucCsrMTlBWWlBWmh1c2ptaVAzc05ZNlZMYnQ2Q1VaM3oxRjd1dUFzK1lndnlIelptNTQzMTM1blFNPQp0cnVzdGVkIGNvbW1lbnQ6IHRpbWVzdGFtcDoxNzkzNDg3NTExCWZpbGU6S2V5SGFuZG9mZl8wLjEuMF9hbWQ2NC5kZWIJdmVyc2lvbjowLjEuMApIVVN5Ty9ZeEtzaWFRS1JsYzFNZ3FpTGdTSzBYQmVzaE9BRGVlVXJZU21KTHd6MzFTdjVzTDBIRXplUHZ4clhOd3BNMWNGL1lBNGw5Y0V1OXYxTkhEUT09Cg==";

    #[test]
    fn a_release_installs_only_48_hours_after_it_was_signed() {
        let signed = signed_at(SIGNATURE);
        assert_eq!(signed, Some(1_793_487_511), "the time comes from the signed trusted comment");
        assert!(!due(signed, 1_793_487_511), "not at release");
        assert!(!due(signed, 1_793_487_511 + 47 * 3600), "not after 47 hours");
        assert!(due(signed, 1_793_487_511 + 48 * 3600), "after 48 hours");
        assert!(!due(signed_at("not a signature"), u64::MAX), "no signing time, no install");
    }
}
