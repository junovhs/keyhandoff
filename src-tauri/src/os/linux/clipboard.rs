//! X11 clipboard. Every operation opens its own connection and a hidden
//! 1x1 window, and its thread ends when the work is done, so nothing watches
//! the clipboard while Safe Copy is off.
//!
//! - Capture: XFixes reports the next CLIPBOARD owner change; the text is
//!   read once, then Handoff takes CLIPBOARD and PRIMARY and refuses every
//!   paste until something else is copied, so the key cannot be pasted again.
//! - Offer: own CLIPBOARD and serve a file (`text/uri-list`,
//!   `x-special/gnome-copied-files`) with the armored text as the text fallback.
//!
//! Limits: a clipboard manager that keeps history (GPaste, CopyQ, Klipper)
//! may already have stored the copy; X11 has no shared call to delete it.
//! x11rb's reply buffers are not zeroized.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use keyhandoff_seal::Secret;
use x11rb::connection::Connection;
use x11rb::protocol::Event;
use x11rb::protocol::xfixes::{ConnectionExt as _, SelectionEventMask};
use x11rb::protocol::xproto::{
    AtomEnum, ConnectionExt as _, CreateWindowAux, EventMask, PropMode, SELECTION_NOTIFY_EVENT, SelectionNotifyEvent,
    SelectionRequestEvent, WindowClass,
};
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;
use x11rb::{CURRENT_TIME, NONE};
use zeroize::{Zeroize, Zeroizing};

use crate::os::{Captured, Clip};

x11rb::atom_manager! {
    Atoms: AtomsCookie {
        CLIPBOARD,
        UTF8_STRING,
        TARGETS,
        TEXT,
        INCR,
        HANDOFF_CLIP,
        TEXT_PLAIN: b"text/plain",
        TEXT_PLAIN_UTF8: b"text/plain;charset=utf-8",
        URI_LIST: b"text/uri-list",
        GNOME_FILES: b"x-special/gnome-copied-files",
    }
}

const TICK: Duration = Duration::from_millis(20);
const READ_WAIT: Duration = Duration::from_secs(1);
const MAX_CAPTURE_BYTES: usize = keyhandoff_seal::MAX_SECRET_BYTES;

/// Bumped whenever Handoff takes the clipboard, so older owner threads stop.
static OWNER_GENERATION: AtomicU64 = AtomicU64::new(0);

struct X {
    conn: RustConnection,
    win: u32,
    atoms: Atoms,
}

impl X {
    fn open() -> Result<Self, String> {
        let (conn, screen) = x11rb::connect(None).map_err(|e| format!("No X11 display: {e}"))?;
        let root = conn.setup().roots[screen].root;
        let win = conn.generate_id().map_err(err)?;
        conn.create_window(
            x11rb::COPY_DEPTH_FROM_PARENT,
            win,
            root,
            0,
            0,
            1,
            1,
            0,
            WindowClass::INPUT_ONLY,
            x11rb::COPY_FROM_PARENT,
            &CreateWindowAux::new().event_mask(EventMask::PROPERTY_CHANGE),
        )
        .map_err(err)?;
        let atoms = Atoms::new(&conn).map_err(err)?.reply().map_err(err)?;
        conn.flush().map_err(err)?;
        Ok(Self { conn, win, atoms })
    }

    fn convert(&self, target: u32, time: u32) -> Result<(), String> {
        self.conn.convert_selection(self.win, self.atoms.CLIPBOARD, target, self.atoms.HANDOFF_CLIP, time).map_err(err)?;
        self.conn.flush().map_err(err)
    }

    /// Read and delete the transfer property. `None` for INCR (too large).
    fn take_property(&self) -> Result<Option<Zeroizing<Vec<u8>>>, String> {
        let reply = self
            .conn
            .get_property(true, self.win, self.atoms.HANDOFF_CLIP, AtomEnum::ANY, 0, u32::MAX / 4)
            .map_err(err)?
            .reply()
            .map_err(err)?;
        if reply.type_ == self.atoms.INCR {
            return Ok(None);
        }
        Ok(Some(Zeroizing::new(reply.value)))
    }

    /// Become the owner of CLIPBOARD (and PRIMARY if asked). Returns the new
    /// owner generation, or an error if another client won the race.
    fn own(&self, primary: bool) -> Result<u64, String> {
        let generation = OWNER_GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
        self.conn.set_selection_owner(self.win, self.atoms.CLIPBOARD, CURRENT_TIME).map_err(err)?;
        if primary {
            self.conn.set_selection_owner(self.win, AtomEnum::PRIMARY.into(), CURRENT_TIME).map_err(err)?;
        }
        let owner = self.conn.get_selection_owner(self.atoms.CLIPBOARD).map_err(err)?.reply().map_err(err)?.owner;
        if owner != self.win {
            return Err("Could not take the clipboard.".into());
        }
        Ok(generation)
    }

    /// Answer paste requests until something else takes CLIPBOARD or Handoff
    /// takes it again. `offer` lists (target, bytes); `None` refuses everything.
    fn serve(&self, offer: Option<&[(u32, Vec<u8>)]>, generation: u64) {
        loop {
            if OWNER_GENERATION.load(Ordering::SeqCst) != generation {
                return;
            }
            loop {
                match self.conn.poll_for_event() {
                    Ok(Some(Event::SelectionRequest(r))) => self.answer(&r, offer),
                    Ok(Some(Event::SelectionClear(c))) if c.selection == self.atoms.CLIPBOARD => return,
                    Ok(Some(_)) => {}
                    Ok(None) => break,
                    Err(_) => return,
                }
            }
            thread::sleep(TICK);
        }
    }

    fn answer(&self, r: &SelectionRequestEvent, offer: Option<&[(u32, Vec<u8>)]>) {
        let property = if r.property == NONE { r.target } else { r.property };
        let entries = offer.unwrap_or(&[]);
        let served = if r.target == self.atoms.TARGETS {
            let mut targets = vec![self.atoms.TARGETS];
            targets.extend(entries.iter().map(|(t, _)| *t));
            self.conn.change_property32(PropMode::REPLACE, r.requestor, property, AtomEnum::ATOM, &targets).is_ok()
        } else if let Some((target, bytes)) = entries.iter().find(|(t, _)| *t == r.target) {
            self.conn.change_property8(PropMode::REPLACE, r.requestor, property, *target, bytes).is_ok()
        } else {
            false
        };
        let event = SelectionNotifyEvent {
            response_type: SELECTION_NOTIFY_EVENT,
            sequence: 0,
            time: r.time,
            requestor: r.requestor,
            selection: r.selection,
            target: r.target,
            property: if served { property } else { NONE },
        };
        let _ = self.conn.send_event(false, r.requestor, EventMask::NO_EVENT, event);
        let _ = self.conn.flush();
    }
}

fn err(e: impl std::fmt::Display) -> String {
    format!("Clipboard error: {e}")
}

/// Arm Safe Copy: take the next text copied by another program, then clear the
/// clipboard. `done` runs once, on the capture thread.
pub fn arm_capture(timeout: Duration, cancel: Arc<AtomicBool>, done: impl FnOnce(Captured) + Send + 'static) -> Result<(), String> {
    let x = X::open()?;
    x.conn.xfixes_query_version(5, 0).map_err(err)?.reply().map_err(err)?;
    x.conn
        .xfixes_select_selection_input(x.win, x.atoms.CLIPBOARD, SelectionEventMask::SET_SELECTION_OWNER)
        .map_err(err)?;
    x.conn.flush().map_err(err)?;
    thread::Builder::new()
        .name("safe-copy".into())
        .spawn(move || {
            let result = capture(&x, timeout, &cancel);
            let caught = matches!(result, Captured::Caught(..));
            let generation = if caught { x.own(true).ok() } else { None };
            done(result);
            if let Some(generation) = generation {
                x.serve(None, generation);
            }
        })
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn capture(x: &X, timeout: Duration, cancel: &AtomicBool) -> Captured {
    let deadline = Instant::now() + timeout;
    loop {
        if cancel.load(Ordering::SeqCst) {
            return Captured::Cancelled;
        }
        if Instant::now() >= deadline {
            return Captured::TimedOut;
        }
        loop {
            match x.conn.poll_for_event() {
                Ok(Some(Event::XfixesSelectionNotify(e))) if e.owner != x.win && e.owner != NONE => {
                    if let Err(e) = x.convert(x.atoms.UTF8_STRING, e.selection_timestamp) {
                        return Captured::Failed(e);
                    }
                }
                Ok(Some(Event::SelectionNotify(e))) if e.requestor == x.win && e.property != NONE => {
                    match x.take_property() {
                        Ok(Some(bytes)) => {
                            if let Some(secret) = to_secret(bytes) {
                                return Captured::Caught(secret, None);
                            }
                        }
                        Ok(None) => {}
                        Err(e) => return Captured::Failed(e),
                    }
                }
                // Not text (an image, files): keep listening.
                Ok(Some(_)) => {}
                Ok(None) => break,
                Err(e) => return Captured::Failed(err(e)),
            }
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
    let x = X::open()?;
    let uri = file_uri(file);
    let mut text = text.into_bytes();
    let offer = vec![
        (x.atoms.URI_LIST, format!("{uri}\r\n").into_bytes()),
        (x.atoms.GNOME_FILES, format!("copy\n{uri}").into_bytes()),
        (x.atoms.UTF8_STRING, text.clone()),
        (x.atoms.TEXT_PLAIN_UTF8, text.clone()),
        (x.atoms.TEXT_PLAIN, text.clone()),
        (x.atoms.TEXT, text.clone()),
        (AtomEnum::STRING.into(), std::mem::take(&mut text)),
    ];
    let generation = x.own(false)?;
    thread::Builder::new()
        .name("clipboard-offer".into())
        .spawn(move || x.serve(Some(&offer), generation))
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Put plain text on the clipboard. Only for text that isn't secret.
pub fn offer_text(text: String) -> Result<(), String> {
    let x = X::open()?;
    let text = text.into_bytes();
    let offer = vec![
        (x.atoms.UTF8_STRING, text.clone()),
        (x.atoms.TEXT_PLAIN_UTF8, text.clone()),
        (x.atoms.TEXT_PLAIN, text.clone()),
        (x.atoms.TEXT, text.clone()),
        (AtomEnum::STRING.into(), text),
    ];
    let generation = x.own(false)?;
    thread::Builder::new()
        .name("clipboard-offer".into())
        .spawn(move || x.serve(Some(&offer), generation))
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Read the clipboard once for a paste: files first, then text.
pub fn read_clipboard() -> Result<Clip, String> {
    let x = X::open()?;
    for target in [x.atoms.URI_LIST, x.atoms.UTF8_STRING, AtomEnum::STRING.into()] {
        x.convert(target, CURRENT_TIME)?;
        let Some(bytes) = wait_for_property(&x)? else { continue };
        let Ok(text) = String::from_utf8(bytes.to_vec()) else { continue };
        let mut text = Zeroizing::new(text);
        if target == x.atoms.URI_LIST {
            let files: Vec<PathBuf> = text.lines().filter_map(path_from_uri).collect();
            text.zeroize();
            if !files.is_empty() {
                return Ok(Clip::Files(files));
            }
        } else if !text.trim().is_empty() {
            return Ok(Clip::Text(text));
        }
    }
    Ok(Clip::Empty)
}

fn wait_for_property(x: &X) -> Result<Option<Zeroizing<Vec<u8>>>, String> {
    let deadline = Instant::now() + READ_WAIT;
    while Instant::now() < deadline {
        while let Some(event) = x.conn.poll_for_event().map_err(err)? {
            if let Event::SelectionNotify(e) = event {
                if e.requestor == x.win {
                    return if e.property == NONE { Ok(None) } else { x.take_property() };
                }
            }
        }
        thread::sleep(TICK);
    }
    Ok(None)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uris_round_trip_spaces_and_unicode() {
        let path = Path::new("/run/user/1000/handoff/Key for Zoë & co.handoff");
        let uri = file_uri(path);
        assert_eq!(uri, "file:///run/user/1000/handoff/Key%20for%20Zo%C3%AB%20%26%20co.handoff");
        assert_eq!(path_from_uri(&uri).as_deref(), Some(path));
        assert_eq!(path_from_uri("file://localhost/tmp/a%20b"), Some(PathBuf::from("/tmp/a b")));
        assert_eq!(path_from_uri("https://example.com/x"), None);
    }

    // Real X11 checks: `cargo test -- --ignored` with DISPLAY set and xclip installed.

    fn xclip_copy(text: &str) {
        use std::io::Write;
        let mut child = std::process::Command::new("xclip")
            .args(["-selection", "clipboard", "-i"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("xclip");
        child.stdin.take().unwrap().write_all(text.as_bytes()).unwrap();
        child.wait().unwrap();
    }

    fn xclip_paste(target: &str) -> Option<String> {
        let out = std::process::Command::new("timeout")
            .args(["3", "xclip", "-selection", "clipboard", "-o", "-t", target])
            .output()
            .ok()?;
        out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
    }

    #[test]
    #[ignore = "needs an X11 display and xclip"]
    fn x11_capture_takes_one_copy_and_clears_it() {
        let (tx, rx) = std::sync::mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        arm_capture(Duration::from_secs(5), cancel, move |r| tx.send(r).unwrap()).unwrap();
        thread::sleep(Duration::from_millis(100));
        xclip_copy("  sk-synthetic-x11-0001\n");
        match rx.recv_timeout(Duration::from_secs(5)).unwrap() {
            Captured::Caught(s, _) => assert_eq!(s.expose(), b"sk-synthetic-x11-0001"),
            _ => panic!("not caught"),
        }
        thread::sleep(Duration::from_millis(200));
        assert_eq!(xclip_paste("UTF8_STRING").unwrap_or_default(), "", "clipboard still pasteable");
        let primary = std::process::Command::new("timeout").args(["3", "xclip", "-selection", "primary", "-o"]).output().unwrap();
        assert!(primary.stdout.is_empty());
        // Copying again afterwards works normally, and is not captured.
        xclip_copy("after");
        thread::sleep(Duration::from_millis(100));
        assert_eq!(xclip_paste("UTF8_STRING").as_deref(), Some("after"));
    }

    #[test]
    #[ignore = "needs an X11 display and xclip"]
    fn x11_capture_times_out_and_cancels() {
        let (tx, rx) = std::sync::mpsc::channel();
        arm_capture(Duration::from_millis(300), Arc::new(AtomicBool::new(false)), move |r| tx.send(r).unwrap()).unwrap();
        assert!(matches!(rx.recv_timeout(Duration::from_secs(3)).unwrap(), Captured::TimedOut));
        let (tx, rx) = std::sync::mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        arm_capture(Duration::from_secs(5), cancel.clone(), move |r| tx.send(r).unwrap()).unwrap();
        cancel.store(true, Ordering::SeqCst);
        assert!(matches!(rx.recv_timeout(Duration::from_secs(3)).unwrap(), Captured::Cancelled));
    }

    #[test]
    #[ignore = "needs an X11 display and xclip"]
    fn x11_offer_serves_file_and_text_then_reads_back() {
        let path = Path::new("/tmp/Key request from Test.handoff");
        offer_file(path, "BEGIN HANDOFF test".into()).unwrap();
        thread::sleep(Duration::from_millis(100));
        assert_eq!(xclip_paste("text/uri-list").as_deref(), Some("file:///tmp/Key%20request%20from%20Test.handoff\r\n"));
        assert_eq!(xclip_paste("x-special/gnome-copied-files").as_deref(), Some("copy\nfile:///tmp/Key%20request%20from%20Test.handoff"));
        assert_eq!(xclip_paste("UTF8_STRING").as_deref(), Some("BEGIN HANDOFF test"));
        match read_clipboard().unwrap() {
            Clip::Files(f) => assert_eq!(f, vec![path.to_path_buf()]),
            _ => panic!("expected a file"),
        }
        xclip_copy("plain text");
        thread::sleep(Duration::from_millis(100));
        match read_clipboard().unwrap() {
            Clip::Text(t) => assert_eq!(&*t, "plain text"),
            _ => panic!("expected text"),
        }
    }

    /// How often an independent program reading the clipboard as fast as it
    /// can sees a fake key before Safe Copy takes it. A measurement, not a
    /// pass/fail check: results go in docs/capture-tests.md.
    /// `CLIPBOARD_RACE_COPIES` sets the count (default 1000).
    #[test]
    #[ignore = "measurement: needs an X11 display and xclip"]
    fn x11_fast_reader_race() {
        use std::collections::HashSet;
        use std::io::{BufRead, BufReader};
        use std::sync::Mutex;

        let copies: usize = std::env::var("CLIPBOARD_RACE_COPIES").ok().and_then(|n| n.parse().ok()).unwrap_or(1000);
        // cargo test builds examples next to the test binary's deps folder.
        let exe = std::env::current_exe().unwrap();
        let reader_path = exe.parent().unwrap().parent().unwrap().join("examples/clipboard_reader");
        let mut reader = std::process::Command::new(&reader_path)
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap_or_else(|e| panic!("{}: {e} (cargo build --example clipboard_reader)", reader_path.display()));
        let mut lines = BufReader::new(reader.stdout.take().unwrap()).lines();
        assert_eq!(lines.next().unwrap().unwrap(), "ready");
        let seen = Arc::new(Mutex::new(HashSet::new()));
        let collect = seen.clone();
        thread::spawn(move || lines.map_while(Result::ok).for_each(|l| {
            collect.lock().unwrap().insert(l);
        }));

        let mut caught = 0;
        let mut wipe_ms = Vec::with_capacity(copies);
        for i in 0..copies {
            let key = format!("sk-synthetic-race-{i:04}");
            let (tx, rx) = std::sync::mpsc::channel();
            arm_capture(Duration::from_secs(5), Arc::new(AtomicBool::new(false)), move |r| tx.send((r, Instant::now())).unwrap()).unwrap();
            let copied = Instant::now();
            xclip_copy(&key);
            if let Ok((Captured::Caught(s, _), at)) = rx.recv_timeout(Duration::from_secs(5)) {
                caught += usize::from(s.expose() == key.as_bytes());
                wipe_ms.push(at.duration_since(copied).as_secs_f64() * 1000.0);
            }
        }
        thread::sleep(Duration::from_secs(1));
        let _ = reader.kill();
        let seen = seen.lock().unwrap().len();
        wipe_ms.sort_by(f64::total_cmp);
        let pct = |p: f64| wipe_ms.get(((wipe_ms.len() as f64 - 1.0) * p) as usize).copied().unwrap_or(f64::NAN);
        eprintln!(
            "race: {copies} copies, Safe Copy caught {caught}, fast reader saw {seen}; copy-to-catch ms p50 {:.1} p90 {:.1} max {:.1}",
            pct(0.5),
            pct(0.9),
            pct(1.0)
        );
        assert_eq!(caught, copies, "Safe Copy missed copies; the race numbers would mean nothing");
    }

    #[test]
    fn capture_text_is_trimmed_and_bounded() {
        let s = to_secret(Zeroizing::new(b"  sk-test-123\n".to_vec())).unwrap();
        assert_eq!(s.expose(), b"sk-test-123");
        assert!(to_secret(Zeroizing::new(b" \n\t".to_vec())).is_none());
        assert!(to_secret(Zeroizing::new(vec![b'a'; MAX_CAPTURE_BYTES + 1])).is_none());
        assert!(to_secret(Zeroizing::new(vec![0xff, 0xfe])).is_none());
    }
}
