//! Windows Safe Copy: a message-only window registered with
//! `AddClipboardFormatListener` gets `WM_CLIPBOARDUPDATE` for every clipboard
//! change while armed. The first change carrying `CF_UNICODETEXT` is read,
//! the clipboard is emptied, and then the matching Win+V history entry is
//! deleted through WinRT `Clipboard::DeleteItemFromHistory`.
//!
//! Windows has no way to keep a copy made by another app out of clipboard
//! history up front (`ExcludeClipboardContentFromMonitorProcessing` and
//! `CanIncludeInClipboardHistory` only work when the copying app sets them),
//! so the entry is deleted after the fact.

use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::{Duration, Instant};

use ::windows::core::w;
use ::windows::ApplicationModel::DataTransfer::{Clipboard, ClipboardHistoryItemsResultStatus, StandardDataFormats};
use ::windows::Win32::Foundation::{HGLOBAL, HWND, LPARAM, LRESULT, WPARAM};
use ::windows::Win32::System::DataExchange::{
    AddClipboardFormatListener, CloseClipboard, EmptyClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard,
    RemoveClipboardFormatListener,
};
use ::windows::Win32::System::LibraryLoader::GetModuleHandleW;
use ::windows::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};
use ::windows::Win32::System::Ole::CF_UNICODETEXT;
use ::windows::Win32::System::WinRT::{RoInitialize, RO_INIT_SINGLETHREADED};
use ::windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW, KillTimer, PostQuitMessage, RegisterClassW,
    SetTimer, HWND_MESSAGE, MSG, WINDOW_EX_STYLE, WINDOW_STYLE, WM_CLIPBOARDUPDATE, WM_TIMER, WNDCLASSW,
};
use keyhandoff_seal::Secret;
use zeroize::{Zeroize, Zeroizing};

use crate::os::Captured;

const TIMEOUT_TIMER: usize = 1;
/// Checks the cancel flag this often.
const CANCEL_TIMER: usize = 2;
const CANCEL_POLL_MS: u32 = 100;
/// How long to keep looking for the copy to show up in clipboard history.
const HISTORY_WAIT: Duration = Duration::from_secs(3);
/// Give up on the history thread after this, so a stuck WinRT call can't hold up the catch.
const HISTORY_LIMIT: Duration = Duration::from_secs(5);

/// What the window procedure saw, for the message loop to act on.
enum Seen {
    Text(Zeroizing<Vec<u16>>),
    TimedOut,
    Cancelled,
}

thread_local! {
    static SEEN: RefCell<Option<Seen>> = const { RefCell::new(None) };
    static CANCEL: RefCell<Option<Arc<AtomicBool>>> = const { RefCell::new(None) };
}

/// What happened to the Win+V history entry after a catch.
#[derive(Debug, PartialEq, Eq)]
enum History {
    Removed,
    Disabled,
    NotFound,
    Cleared,
    Failed,
}

impl History {
    /// Only outcomes the user should act on or know about become a note.
    fn note(&self) -> Option<String> {
        match self {
            History::Removed | History::Disabled | History::NotFound => None,
            History::Cleared => Some("Clipboard history was cleared to remove it.".into()),
            History::Failed => Some("Check Win+V: it could not be removed from clipboard history.".into()),
        }
    }
}

/// `read` would report something reading bait before the copy; Windows has no
/// bait yet (it needs delayed rendering, and Win+V reads every copy), so it
/// is never called here.
pub fn arm_capture(
    timeout: Duration,
    cancel: Arc<AtomicBool>,
    done: impl FnOnce(Captured) + Send + 'static,
    _read: impl Fn() + Send + 'static,
) -> Result<(), String> {
    let (ready_tx, ready_rx) = mpsc::channel::<Result<(), String>>();
    thread::Builder::new()
        .name("safe-copy".into())
        .spawn(move || {
            CANCEL.with(|c| *c.borrow_mut() = Some(cancel));
            // SAFETY: the window lives on this thread and is destroyed by `pump`.
            let hwnd = match unsafe { listen_window(timeout) } {
                Ok(h) => h,
                Err(e) => {
                    let _ = ready_tx.send(Err(e));
                    return;
                }
            };
            let _ = ready_tx.send(Ok(()));
            match unsafe { pump(hwnd) } {
                Seen::Text(wide) => {
                    // Scrub first so the note can say how it went; the clipboard is already empty.
                    let note = scrub_history(&wide).note();
                    let secret = Secret::new(String::from_utf16_lossy(&wide).into_bytes());
                    done(Captured::Caught(secret, note));
                }
                Seen::TimedOut => done(Captured::TimedOut),
                Seen::Cancelled => done(Captured::Cancelled),
            }
        })
        .map_err(|e| e.to_string())?;
    ready_rx.recv().map_err(|_| "Safe Copy thread ended early".to_string())?
}

/// Create the message-only listener window and its timers.
unsafe fn listen_window(timeout: Duration) -> Result<HWND, String> {
    let instance = GetModuleHandleW(None).map_err(|e| e.to_string())?;
    let class = w!("HandoffSafeCopyListener");
    let wc = WNDCLASSW { lpfnWndProc: Some(wndproc), hInstance: instance.into(), lpszClassName: class, ..Default::default() };
    // Fails harmlessly with "class already exists" after the first arm.
    RegisterClassW(&wc);
    let hwnd = CreateWindowExW(
        WINDOW_EX_STYLE(0),
        class,
        w!(""),
        WINDOW_STYLE(0),
        0,
        0,
        0,
        0,
        Some(HWND_MESSAGE),
        None,
        Some(instance.into()),
        None,
    )
    .map_err(|e| e.to_string())?;
    if let Err(e) = AddClipboardFormatListener(hwnd) {
        let _ = DestroyWindow(hwnd);
        return Err(e.to_string());
    }
    let ms = timeout.as_millis().min(u32::MAX as u128) as u32;
    SetTimer(Some(hwnd), TIMEOUT_TIMER, ms, None);
    SetTimer(Some(hwnd), CANCEL_TIMER, CANCEL_POLL_MS, None);
    Ok(hwnd)
}

/// Run the message loop until the window procedure records an outcome.
unsafe fn pump(hwnd: HWND) -> Seen {
    let mut msg = MSG::default();
    while GetMessageW(&mut msg, None, 0, 0).as_bool() {
        DispatchMessageW(&msg);
    }
    let _ = RemoveClipboardFormatListener(hwnd);
    let _ = KillTimer(Some(hwnd), TIMEOUT_TIMER);
    let _ = KillTimer(Some(hwnd), CANCEL_TIMER);
    let _ = DestroyWindow(hwnd);
    SEEN.with(|s| s.borrow_mut().take()).unwrap_or(Seen::Cancelled)
}

fn cancelled() -> bool {
    CANCEL.with(|c| c.borrow().as_ref().is_some_and(|c| c.load(Ordering::SeqCst)))
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    let seen = match msg {
        WM_CLIPBOARDUPDATE if cancelled() => Some(Seen::Cancelled),
        WM_CLIPBOARDUPDATE => take_text(hwnd).map(Seen::Text),
        WM_TIMER if wp.0 == TIMEOUT_TIMER => Some(Seen::TimedOut),
        WM_TIMER if wp.0 == CANCEL_TIMER => cancelled().then_some(Seen::Cancelled),
        _ => return DefWindowProcW(hwnd, msg, wp, lp),
    };
    if let Some(seen) = seen {
        SEEN.with(|s| {
            let mut s = s.borrow_mut();
            if s.is_none() {
                *s = Some(seen);
                PostQuitMessage(0);
            }
        });
    }
    LRESULT(0)
}

/// Read the clipboard text, zero the copy we were handed, and empty the
/// clipboard. `None` (keep listening) if the change carried no text.
unsafe fn take_text(hwnd: HWND) -> Option<Zeroizing<Vec<u16>>> {
    let format = CF_UNICODETEXT.0 as u32;
    IsClipboardFormatAvailable(format).ok()?;
    // The copying app may still hold the clipboard open for a moment.
    let mut opened = false;
    for _ in 0..25 {
        if OpenClipboard(Some(hwnd)).is_ok() {
            opened = true;
            break;
        }
        thread::sleep(Duration::from_millis(20));
    }
    if !opened {
        return None;
    }
    let text = read_open_clipboard(format);
    if text.is_some() {
        let _ = EmptyClipboard();
    }
    let _ = CloseClipboard();
    text
}

unsafe fn read_open_clipboard(format: u32) -> Option<Zeroizing<Vec<u16>>> {
    let handle = GetClipboardData(format).ok()?;
    let global = HGLOBAL(handle.0);
    let ptr = GlobalLock(global) as *mut u16;
    if ptr.is_null() {
        return None;
    }
    let cap = GlobalSize(global) / 2;
    let buf = std::slice::from_raw_parts_mut(ptr, cap);
    let len = buf.iter().position(|&c| c == 0).unwrap_or(cap);
    let text = Zeroizing::new(buf[..len].to_vec());
    // Our process's copy of the clipboard data is freed by EmptyClipboard;
    // zero it first so the freed memory doesn't keep the key.
    buf.zeroize();
    let _ = GlobalUnlock(global);
    (!text.is_empty()).then_some(text)
}

/// Delete the copy from Win+V history. The WinRT `Clipboard` class can only be
/// used from a single-threaded apartment, so this gets a thread of its own.
fn scrub_history(wanted: &[u16]) -> History {
    let wanted = Zeroizing::new(wanted.to_vec());
    let (tx, rx) = mpsc::channel();
    let spawned = thread::Builder::new().name("safe-copy-history".into()).spawn(move || {
        // SAFETY: plain apartment init for this new thread.
        let _ = unsafe { RoInitialize(RO_INIT_SINGLETHREADED) };
        let _ = tx.send(scrub_on_sta(&wanted));
    });
    match spawned {
        Ok(_) => rx.recv_timeout(HISTORY_LIMIT).unwrap_or(History::Failed),
        Err(_) => History::Failed,
    }
}

/// The history service adds the entry on its own schedule, so look for it for
/// a few seconds.
fn scrub_on_sta(wanted: &[u16]) -> History {
    match Clipboard::IsHistoryEnabled() {
        Ok(true) => {}
        Ok(false) => return History::Disabled,
        Err(_) => return History::Failed,
    }
    let deadline = Instant::now() + HISTORY_WAIT;
    loop {
        match remove_from_history(wanted) {
            Ok(Some(true)) => return History::Removed,
            // Found but not deletable, or history refused us: clear it all.
            Ok(Some(false)) | Err(Denied) => return clear_all(),
            Ok(None) => {}
        }
        if Instant::now() >= deadline {
            return History::NotFound;
        }
        thread::sleep(Duration::from_millis(100));
    }
}

struct Denied;

/// `Some(deleted)` once a matching entry is found, `None` if it isn't there
/// (yet, or the read failed and is worth retrying).
fn remove_from_history(wanted: &[u16]) -> Result<Option<bool>, Denied> {
    let Ok(result) = Clipboard::GetHistoryItemsAsync().and_then(|op| op.join()) else {
        return Ok(None);
    };
    match result.Status() {
        Ok(ClipboardHistoryItemsResultStatus::Success) => {}
        Ok(ClipboardHistoryItemsResultStatus::AccessDenied) => return Err(Denied),
        _ => return Ok(None),
    }
    let (Ok(items), Ok(text_format)) = (result.Items(), StandardDataFormats::Text()) else {
        return Ok(None);
    };
    for item in items {
        let Ok(content) = item.Content() else { continue };
        if !content.Contains(&text_format).unwrap_or(false) {
            continue;
        }
        // The comparison needs WinRT's own copy of the text; it is freed (not
        // zeroed) when this HSTRING drops.
        let Ok(text) = content.GetTextAsync().and_then(|op| op.join()) else { continue };
        if text[..] == *wanted {
            return Ok(Some(Clipboard::DeleteItemFromHistory(&item).unwrap_or(false)));
        }
    }
    Ok(None)
}

/// Fallback when the single entry can't be deleted: clear all of history.
fn clear_all() -> History {
    match Clipboard::ClearHistory() {
        Ok(true) => History::Cleared,
        _ => History::Failed,
    }
}

#[cfg(test)]
mod tests {
    //! Real clipboard; run with `cargo test -- --ignored --test-threads=1`.

    use super::*;
    use ::windows::core::HSTRING;

    use crate::os::windows::clipboard::tests::set_clipboard_text;

    fn clipboard_has_text() -> bool {
        unsafe { IsClipboardFormatAvailable(CF_UNICODETEXT.0 as u32).is_ok() }
    }

    /// `None` only when history is really off; API errors panic so they can't pass as a skip.
    fn history_contains(text: &str) -> Option<bool> {
        let wanted = HSTRING::from(text);
        thread::spawn(move || {
            let _ = unsafe { RoInitialize(RO_INIT_SINGLETHREADED) };
            if !Clipboard::IsHistoryEnabled().unwrap() {
                eprintln!("clipboard history is off: history deletion not exercised");
                return None;
            }
            let result = Clipboard::GetHistoryItemsAsync().unwrap().join().unwrap();
            assert_eq!(result.Status().unwrap(), ClipboardHistoryItemsResultStatus::Success);
            let fmt = StandardDataFormats::Text().unwrap();
            Some(result.Items().unwrap().into_iter().any(|item| {
                item.Content()
                    .and_then(|c| if c.Contains(&fmt)? { c.GetTextAsync()?.join() } else { Ok(HSTRING::new()) })
                    .map(|t| t == wanted)
                    .unwrap_or(false)
            }))
        })
        .join()
        .unwrap()
    }

    #[test]
    #[ignore = "uses the real clipboard"]
    fn catches_one_copy_and_wipes_clipboard_and_history() {
        let key = format!("sk_test_handoff_{}", std::process::id());
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        arm_capture(Duration::from_secs(10), cancel.clone(), move |c| {
            let _ = tx.send(c);
        }, || {})
        .unwrap();
        set_clipboard_text(&key).unwrap();
        match rx.recv_timeout(Duration::from_secs(10)).expect("no result") {
            Captured::Caught(secret, note) => {
                assert_eq!(secret.expose(), key.as_bytes());
                eprintln!("history note: {note:?}");
                assert_ne!(note, History::Failed.note(), "history scrub failed");
            }
            _ => panic!("expected a catch"),
        }
        assert!(!clipboard_has_text(), "clipboard still holds text after the catch");
        if let Some(found) = history_contains(&key) {
            assert!(!found, "synthetic key still in clipboard history");
        }
        // Disarmed after one catch: a second copy stays on the clipboard.
        set_clipboard_text("not a key").unwrap();
        thread::sleep(Duration::from_millis(200));
        assert!(clipboard_has_text(), "still listening after the first catch");
    }

    /// How often an independent program reading the clipboard as fast as it
    /// can sees a fake key before Safe Copy empties it. A measurement, not a
    /// pass/fail check: results go in docs/capture-tests.md.
    /// `CLIPBOARD_RACE_COPIES` sets the count (default 1000).
    #[test]
    #[ignore = "measurement: uses the real clipboard"]
    fn fast_reader_race() {
        use std::collections::HashSet;
        use std::io::{BufRead, BufReader};
        use std::sync::Mutex;

        let copies: usize = std::env::var("CLIPBOARD_RACE_COPIES").ok().and_then(|n| n.parse().ok()).unwrap_or(1000);
        // cargo test builds examples next to the test binary's deps folder.
        let exe = std::env::current_exe().unwrap();
        let reader_path = exe.parent().unwrap().parent().unwrap().join("examples/clipboard_reader.exe");
        let mut reader = std::process::Command::new(&reader_path)
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap_or_else(|e| panic!("{}: {e} (cargo build --example clipboard_reader)", reader_path.display()));
        let mut lines = BufReader::new(reader.stdout.take().unwrap()).lines();
        assert_eq!(lines.next().unwrap().unwrap(), "ready");
        let seen = Arc::new(Mutex::new(HashSet::new()));
        let collect = seen.clone();
        thread::spawn(move || {
            lines.map_while(Result::ok).for_each(|l| {
                collect.lock().unwrap().insert(l);
            })
        });

        let mut caught = 0;
        for i in 0..copies {
            let key = format!("sk-synthetic-race-{i:04}");
            let (tx, rx) = mpsc::channel();
            arm_capture(Duration::from_secs(10), Arc::new(AtomicBool::new(false)), move |c| {
                let _ = tx.send(c);
            }, || {})
            .unwrap();
            set_clipboard_text(&key).unwrap();
            if let Ok(Captured::Caught(secret, _)) = rx.recv_timeout(Duration::from_secs(10)) {
                caught += usize::from(secret.expose() == key.as_bytes());
            }
        }
        thread::sleep(Duration::from_secs(1));
        let _ = reader.kill();
        let seen = seen.lock().unwrap().len();
        eprintln!("race: {copies} copies, Safe Copy caught {caught}, fast reader saw {seen}");
        assert_eq!(caught, copies, "Safe Copy missed copies; the race numbers would mean nothing");
    }

    #[test]
    #[ignore = "uses the real clipboard"]
    fn times_out_and_cancels() {
        let (tx, rx) = mpsc::channel();
        arm_capture(Duration::from_millis(300), Arc::new(AtomicBool::new(false)), move |c| {
            let _ = tx.send(c);
        }, || {})
        .unwrap();
        assert!(matches!(rx.recv_timeout(Duration::from_secs(5)), Ok(Captured::TimedOut)));

        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        arm_capture(Duration::from_secs(30), cancel.clone(), move |c| {
            let _ = tx.send(c);
        }, || {})
        .unwrap();
        cancel.store(true, Ordering::SeqCst);
        assert!(matches!(rx.recv_timeout(Duration::from_secs(5)), Ok(Captured::Cancelled)));
    }
}
