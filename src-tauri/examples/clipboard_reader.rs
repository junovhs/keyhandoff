//! Test-only: an independent program that reads every clipboard change as
//! fast as it can, to measure how often it sees a key before Safe Copy wipes
//! it (docs/capture-tests.md). It prints `ready`, then each text it reads
//! that starts with `sk-synthetic-race-`, one per line. Fake keys only.
//!
//! `cargo run --example clipboard_reader`; the race tests start it themselves.

const MARKER: &str = "sk-synthetic-race-";

fn main() {
    imp::run();
}

fn seen(text: &str) {
    use std::io::Write;
    if text.starts_with(MARKER) {
        let mut out = std::io::stdout().lock();
        let _ = writeln!(out, "{text}");
        let _ = out.flush();
    }
}

fn ready() {
    use std::io::Write;
    println!("ready");
    let _ = std::io::stdout().flush();
}

/// X11: XFixes reports each new CLIPBOARD owner; ask that owner for the text
/// at once, blocking on events rather than polling.
#[cfg(target_os = "linux")]
mod imp {
    use x11rb::connection::Connection;
    use x11rb::protocol::xfixes::{ConnectionExt as _, SelectionEventMask};
    use x11rb::protocol::xproto::{AtomEnum, ConnectionExt as _, CreateWindowAux, WindowClass};
    use x11rb::protocol::Event;
    use x11rb::NONE;

    pub fn run() {
        let (conn, screen) = x11rb::connect(None).expect("an X11 display");
        let root = conn.setup().roots[screen].root;
        let win = conn.generate_id().unwrap();
        conn.create_window(0, win, root, 0, 0, 1, 1, 0, WindowClass::INPUT_ONLY, 0, &CreateWindowAux::new()).unwrap();
        let atom = |name: &[u8]| conn.intern_atom(false, name).unwrap().reply().unwrap().atom;
        let (clipboard, utf8, property) = (atom(b"CLIPBOARD"), atom(b"UTF8_STRING"), atom(b"RACE_READER"));
        conn.xfixes_query_version(5, 0).unwrap().reply().unwrap();
        conn.xfixes_select_selection_input(win, clipboard, SelectionEventMask::SET_SELECTION_OWNER).unwrap();
        conn.flush().unwrap();
        super::ready();
        loop {
            match conn.wait_for_event() {
                Ok(Event::XfixesSelectionNotify(e)) if e.owner != NONE => {
                    let _ = conn.convert_selection(win, clipboard, utf8, property, e.selection_timestamp);
                    let _ = conn.flush();
                }
                Ok(Event::SelectionNotify(e)) if e.property != NONE => {
                    if let Ok(Ok(reply)) = conn.get_property(true, win, property, AtomEnum::ANY, 0, 1 << 16).map(|c| c.reply()) {
                        super::seen(&String::from_utf8_lossy(&reply.value));
                    }
                }
                Ok(_) => {}
                Err(e) => panic!("X11: {e}"),
            }
        }
    }
}

/// Windows: a clipboard format listener that opens the clipboard and reads
/// `CF_UNICODETEXT` the moment `WM_CLIPBOARDUPDATE` arrives.
#[cfg(windows)]
mod imp {
    use windows::core::w;
    use windows::Win32::Foundation::{HGLOBAL, HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::DataExchange::{AddClipboardFormatListener, CloseClipboard, GetClipboardData, OpenClipboard};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};
    use windows::Win32::System::Ole::CF_UNICODETEXT;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, RegisterClassW, HWND_MESSAGE, MSG, WINDOW_EX_STYLE,
        WINDOW_STYLE, WM_CLIPBOARDUPDATE, WNDCLASSW,
    };

    pub fn run() {
        // SAFETY: plain window setup on this thread; the window lives until exit.
        unsafe {
            let instance = GetModuleHandleW(None).unwrap();
            let class = WNDCLASSW { lpfnWndProc: Some(proc), hInstance: instance.into(), lpszClassName: w!("RaceReader"), ..Default::default() };
            RegisterClassW(&class);
            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("RaceReader"),
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
            .unwrap();
            AddClipboardFormatListener(hwnd).unwrap();
            super::ready();
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                DispatchMessageW(&msg);
            }
        }
    }

    extern "system" fn proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if msg == WM_CLIPBOARDUPDATE {
            if let Some(text) = read(hwnd) {
                super::seen(&text);
            }
            return LRESULT(0);
        }
        // SAFETY: default handling for everything else.
        unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
    }

    /// Retry briefly: the copying app may still hold the clipboard open.
    fn read(hwnd: HWND) -> Option<String> {
        for _ in 0..50 {
            // SAFETY: open, read and close on this thread; the lock is released before closing.
            unsafe {
                if OpenClipboard(Some(hwnd)).is_err() {
                    std::thread::sleep(std::time::Duration::from_micros(200));
                    continue;
                }
                let text = GetClipboardData(CF_UNICODETEXT.0 as u32).ok().and_then(|h| {
                    let global = HGLOBAL(h.0);
                    let ptr = GlobalLock(global) as *const u16;
                    if ptr.is_null() {
                        return None;
                    }
                    let units = std::slice::from_raw_parts(ptr, GlobalSize(global) / 2);
                    let end = units.iter().position(|&u| u == 0).unwrap_or(units.len());
                    let text = String::from_utf16_lossy(&units[..end]);
                    let _ = GlobalUnlock(global);
                    Some(text)
                });
                let _ = CloseClipboard();
                return text;
            }
        }
        None
    }
}

#[cfg(not(any(target_os = "linux", windows)))]
mod imp {
    pub fn run() {
        eprintln!("clipboard_reader: Linux (X11) and Windows only");
        std::process::exit(2);
    }
}
