//! The general pasteboard. macOS sends no notice when it changes, so Safe
//! Copy checks its change count every 10 ms while it listens, on a thread
//! that ends when the capture does; nothing watches the clipboard while Safe
//! Copy is off.
//!
//! - Capture: the next change is read once as text, then the pasteboard is
//!   cleared, so the key cannot be pasted again. Clipboard managers on macOS
//!   check the same change count, most every half second or more, so the
//!   clear usually comes first; Safe Copy names the ones running.
//! - Offer: one pasteboard item with the file's URL and the armored text, so
//!   it pastes as an attachment where supported and as text elsewhere.
//!
//! Limits: the pasteboard server keeps the copy until it's cleared, and any
//! program can read it in that moment; with Universal Clipboard on, a copy
//! may reach your other Apple devices before it's cleared. NSString buffers
//! are not zeroized.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use keyhandoff_seal::Secret;
use objc2::rc::{Retained, autoreleasepool};
use objc2::runtime::ProtocolObject;
use objc2_app_kit::{NSPasteboard, NSPasteboardItem, NSPasteboardType, NSPasteboardTypeFileURL, NSPasteboardTypeString, NSPasteboardWriting};
use objc2_foundation::{NSArray, NSString};
use zeroize::{Zeroize, Zeroizing};

use crate::os::{Captured, Clip};

const TICK: Duration = Duration::from_millis(10);
const MAX_CAPTURE_BYTES: usize = keyhandoff_seal::MAX_SECRET_BYTES;

fn general() -> Retained<NSPasteboard> {
    NSPasteboard::generalPasteboard()
}

fn string_type() -> &'static NSPasteboardType {
    // SAFETY: an AppKit constant, set before main runs.
    unsafe { NSPasteboardTypeString }
}

fn file_url_type() -> &'static NSPasteboardType {
    // SAFETY: an AppKit constant, set before main runs.
    unsafe { NSPasteboardTypeFileURL }
}

/// Arm Safe Copy: take the next text copied by another program, then clear
/// the clipboard. `done` runs once, on the capture thread. The general
/// pasteboard can't report reads, so `read` never runs (`CLIPBOARD_BAIT`).
pub fn arm_capture(
    timeout: Duration,
    cancel: Arc<AtomicBool>,
    done: impl FnOnce(Captured) + Send + 'static,
    _read: impl Fn() + Send + 'static,
) -> Result<(), String> {
    let start = autoreleasepool(|_| general().changeCount());
    thread::Builder::new()
        .name("safe-copy".into())
        .spawn(move || done(capture(start, timeout, &cancel)))
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn capture(mut seen: isize, timeout: Duration, cancel: &AtomicBool) -> Captured {
    let deadline = Instant::now() + timeout;
    loop {
        if cancel.load(Ordering::SeqCst) {
            return Captured::Cancelled;
        }
        if Instant::now() >= deadline {
            return Captured::TimedOut;
        }
        let caught = autoreleasepool(|_| {
            let pb = general();
            let now = pb.changeCount();
            if now == seen {
                return None;
            }
            seen = now;
            // Not text (an image, files): keep listening.
            let text = pb.stringForType(string_type())?;
            let secret = to_secret(Zeroizing::new(text.to_string().into_bytes()))?;
            pb.clearContents();
            Some(secret)
        });
        if let Some(secret) = caught {
            return Captured::Caught(secret, None);
        }
        thread::sleep(TICK);
    }
}

/// Trim surrounding whitespace; refuse empty, oversized or non-UTF-8 text.
fn to_secret(bytes: Zeroizing<Vec<u8>>) -> Option<Secret> {
    let trimmed = bytes.trim_ascii();
    if trimmed.is_empty() || trimmed.len() > MAX_CAPTURE_BYTES || std::str::from_utf8(trimmed).is_err() {
        return None;
    }
    Some(Secret::new(trimmed.to_vec()))
}

/// Put a file on the clipboard (pastes as an attachment where supported) with
/// `text` for programs that only take text. Replaces any earlier offer.
pub fn offer_file(file: &Path, text: String) -> Result<(), String> {
    let mut text = text;
    let written = autoreleasepool(|_| {
        let item = NSPasteboardItem::new();
        item.setString_forType(&NSString::from_str(&file_uri(file)), file_url_type());
        item.setString_forType(&NSString::from_str(&text), string_type());
        write(item)
    });
    text.zeroize();
    written
}

/// Put plain text on the clipboard. Only for text that isn't secret.
pub fn offer_text(text: String) -> Result<(), String> {
    autoreleasepool(|_| {
        let item = NSPasteboardItem::new();
        item.setString_forType(&NSString::from_str(&text), string_type());
        write(item)
    })
}

fn write(item: Retained<NSPasteboardItem>) -> Result<(), String> {
    let pb = general();
    pb.clearContents();
    let items = NSArray::from_retained_slice(&[ProtocolObject::<dyn NSPasteboardWriting>::from_retained(item)]);
    if pb.writeObjects(&items) { Ok(()) } else { Err("Could not put it on the clipboard.".into()) }
}

/// Read the clipboard once for a paste: files first, then text.
pub fn read_clipboard() -> Result<Clip, String> {
    autoreleasepool(|_| {
        let pb = general();
        let files: Vec<PathBuf> = pb
            .pasteboardItems()
            .map(|items| items.iter().filter_map(|item| item.stringForType(file_url_type())).filter_map(|u| path_from_uri(&u.to_string())).collect())
            .unwrap_or_default();
        if !files.is_empty() {
            return Ok(Clip::Files(files));
        }
        match pb.stringForType(string_type()) {
            Some(text) => {
                let text = Zeroizing::new(text.to_string());
                if text.trim().is_empty() { Ok(Clip::Empty) } else { Ok(Clip::Text(text)) }
            }
            None => Ok(Clip::Empty),
        }
    })
}

const URI_SAFE: &[u8] = b"-._~/";

pub(crate) fn file_uri(path: &Path) -> String {
    use std::os::unix::ffi::OsStrExt;
    let mut uri = String::from("file://");
    for &b in path.as_os_str().as_bytes() {
        if b.is_ascii_alphanumeric() || URI_SAFE.contains(&b) {
            uri.push(b as char);
        } else {
            uri.push_str(&format!("%{b:02X}"));
        }
    }
    uri
}

fn path_from_uri(line: &str) -> Option<PathBuf> {
    use std::os::unix::ffi::OsStringExt;
    let rest = line.trim().strip_prefix("file://")?;
    let rest = rest.strip_prefix("localhost").unwrap_or(rest);
    // Finder hands out file reference URLs (`/.file/id=…`); ask Foundation
    // for the path they point at.
    if rest.starts_with("/.file/id=") {
        return resolve_reference(line.trim());
    }
    let bytes = rest.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    Some(PathBuf::from(std::ffi::OsString::from_vec(out)))
}

fn resolve_reference(uri: &str) -> Option<PathBuf> {
    let url = objc2_foundation::NSURL::URLWithString(&NSString::from_str(uri))?;
    let path = url.filePathURL()?.path()?;
    Some(PathBuf::from(path.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uris_round_trip_spaces_and_unicode() {
        let path = Path::new("/var/folders/x/T/handoff/Key for Zoë & co.handoff");
        let uri = file_uri(path);
        assert_eq!(uri, "file:///var/folders/x/T/handoff/Key%20for%20Zo%C3%AB%20%26%20co.handoff");
        assert_eq!(path_from_uri(&uri).as_deref(), Some(path));
        assert_eq!(path_from_uri("file://localhost/tmp/a%20b"), Some(PathBuf::from("/tmp/a b")));
        assert_eq!(path_from_uri("https://example.com/x"), None);
    }

    #[test]
    fn capture_text_is_trimmed_and_bounded() {
        let s = to_secret(Zeroizing::new(b"  sk-test-123\n".to_vec())).unwrap();
        assert_eq!(s.expose(), b"sk-test-123");
        assert!(to_secret(Zeroizing::new(b" \n\t".to_vec())).is_none());
        assert!(to_secret(Zeroizing::new(vec![b'a'; MAX_CAPTURE_BYTES + 1])).is_none());
    }

    // Real pasteboard checks: `cargo test clipboard::tests -- --ignored
    // --test-threads=1`. They use `pbcopy` and `pbpaste` as the other program,
    // and replace whatever is on your clipboard.

    fn pbcopy(text: &str) {
        use std::io::Write;
        let mut child = std::process::Command::new("/usr/bin/pbcopy").stdin(std::process::Stdio::piped()).spawn().unwrap();
        child.stdin.take().unwrap().write_all(text.as_bytes()).unwrap();
        child.wait().unwrap();
    }

    fn pbpaste() -> String {
        String::from_utf8(std::process::Command::new("/usr/bin/pbpaste").output().unwrap().stdout).unwrap()
    }

    #[test]
    #[ignore = "uses the real clipboard"]
    fn macos_capture_takes_one_copy_and_clears_it() {
        let (tx, rx) = std::sync::mpsc::channel();
        arm_capture(Duration::from_secs(5), Arc::new(AtomicBool::new(false)), move |r| tx.send(r).unwrap(), || {}).unwrap();
        thread::sleep(Duration::from_millis(100));
        pbcopy("  sk-synthetic-mac-0001\n");
        match rx.recv_timeout(Duration::from_secs(5)).unwrap() {
            Captured::Caught(s, _) => assert_eq!(s.expose(), b"sk-synthetic-mac-0001"),
            _ => panic!("not caught"),
        }
        assert_eq!(pbpaste(), "", "clipboard still pasteable");
        // Copying again afterwards works normally, and is not captured.
        pbcopy("after");
        thread::sleep(Duration::from_millis(100));
        assert_eq!(pbpaste(), "after");
    }

    #[test]
    #[ignore = "uses the real clipboard"]
    fn macos_capture_times_out_and_cancels() {
        let (tx, rx) = std::sync::mpsc::channel();
        arm_capture(Duration::from_millis(300), Arc::new(AtomicBool::new(false)), move |r| tx.send(r).unwrap(), || {}).unwrap();
        assert!(matches!(rx.recv_timeout(Duration::from_secs(3)).unwrap(), Captured::TimedOut));
        let (tx, rx) = std::sync::mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        arm_capture(Duration::from_secs(5), cancel.clone(), move |r| tx.send(r).unwrap(), || {}).unwrap();
        cancel.store(true, Ordering::SeqCst);
        assert!(matches!(rx.recv_timeout(Duration::from_secs(3)).unwrap(), Captured::Cancelled));
    }

    #[test]
    #[ignore = "uses the real clipboard"]
    fn macos_offer_serves_file_and_text_then_reads_back() {
        let path = Path::new("/tmp/Key request from Test.handoff");
        offer_file(path, "BEGIN HANDOFF test".into()).unwrap();
        assert_eq!(pbpaste(), "BEGIN HANDOFF test");
        match read_clipboard().unwrap() {
            Clip::Files(f) => assert_eq!(f, vec![path.to_path_buf()]),
            _ => panic!("expected a file"),
        }
        pbcopy("plain text");
        match read_clipboard().unwrap() {
            Clip::Text(t) => assert_eq!(&*t, "plain text"),
            _ => panic!("expected text"),
        }
        offer_text("handoff:Stripe".into()).unwrap();
        assert_eq!(pbpaste(), "handoff:Stripe");
    }
}
