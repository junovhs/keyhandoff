//! Windows clipboard: Copy writes `CF_HDROP` plus `CF_UNICODETEXT` in one
//! open, so pasting into Teams or a folder attaches the file and text-only
//! apps get the armored text. Paste reads a file list or text back natively,
//! so nothing pasted passes through the screens.

use std::os::windows::ffi::OsStringExt;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use ::windows::core::w;
use ::windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL};
use ::windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard, RegisterClipboardFormatW,
    SetClipboardData,
};
use ::windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GMEM_MOVEABLE};
use ::windows::Win32::System::Ole::{CF_HDROP, CF_UNICODETEXT};
use ::windows::Win32::UI::Shell::{DragQueryFileW, HDROP};
use zeroize::Zeroizing;

use crate::os::Clip;

/// Longest pasted text we will look at. An armored object is a few KiB.
const MAX_TEXT_UNITS: usize = 64 * 1024;
const DROPEFFECT_COPY: u32 = 1;
/// Size of the `DROPFILES` header that starts a `CF_HDROP` block.
const DROPFILES_LEN: u32 = 20;

/// Hold the clipboard open for the life of the guard.
struct Open;

impl Open {
    fn new() -> Result<Self, String> {
        // Another app may hold it for a moment.
        let mut last = String::new();
        for _ in 0..25 {
            // SAFETY: plain call; closed by Drop.
            match unsafe { OpenClipboard(None) } {
                Ok(()) => return Ok(Open),
                Err(e) => last = e.to_string(),
            }
            thread::sleep(Duration::from_millis(20));
        }
        Err(format!("The clipboard is busy or unavailable ({last})."))
    }
}

impl Drop for Open {
    fn drop(&mut self) {
        // SAFETY: we opened it.
        let _ = unsafe { CloseClipboard() };
    }
}

/// Put `file` on the clipboard as a file, with `text` as the text fallback.
/// "Preferred DropEffect" = copy tells Explorer to copy, not move, the file.
pub fn offer_file(file: &Path, text: String) -> Result<(), String> {
    let hdrop = drop_files(&[file]);
    let wide: Vec<u8> = text.encode_utf16().chain([0]).flat_map(u16::to_le_bytes).collect();
    let _open = Open::new()?;
    // SAFETY: the clipboard is open; each block is handed to the clipboard
    // on success and freed by us on failure.
    unsafe {
        EmptyClipboard().map_err(|e| e.to_string())?;
        set(CF_HDROP.0 as u32, &hdrop)?;
        set(CF_UNICODETEXT.0 as u32, &wide)?;
        let effect = RegisterClipboardFormatW(w!("Preferred DropEffect"));
        if effect != 0 {
            let _ = set(effect, &DROPEFFECT_COPY.to_le_bytes());
        }
    }
    Ok(())
}

/// Read the clipboard once for a paste: files first, then text.
pub fn read_clipboard() -> Result<Clip, String> {
    let _open = Open::new()?;
    // SAFETY: the clipboard is open; we only read the handles it gives us
    // and never free or change them.
    unsafe {
        if IsClipboardFormatAvailable(CF_HDROP.0 as u32).is_ok() {
            let handle = GetClipboardData(CF_HDROP.0 as u32).map_err(|e| e.to_string())?;
            let files = dropped_paths(HDROP(handle.0));
            if !files.is_empty() {
                return Ok(Clip::Files(files));
            }
        }
        if IsClipboardFormatAvailable(CF_UNICODETEXT.0 as u32).is_ok() {
            let handle = GetClipboardData(CF_UNICODETEXT.0 as u32).map_err(|e| e.to_string())?;
            return Ok(read_text(HGLOBAL(handle.0)).map_or(Clip::Empty, Clip::Text));
        }
    }
    Ok(Clip::Empty)
}

/// A `CF_HDROP` block: a `DROPFILES` header (offset to the list, drop point
/// 0,0, not in a non-client area, wide characters) followed by each path as
/// UTF-16 with a terminating zero, and one more zero to end the list.
fn drop_files(paths: &[&Path]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&DROPFILES_LEN.to_le_bytes()); // pFiles
    out.extend_from_slice(&0i32.to_le_bytes()); // pt.x
    out.extend_from_slice(&0i32.to_le_bytes()); // pt.y
    out.extend_from_slice(&0i32.to_le_bytes()); // fNC
    out.extend_from_slice(&1i32.to_le_bytes()); // fWide
    for path in paths {
        for unit in path.as_os_str().to_string_lossy().encode_utf16().chain([0]) {
            out.extend_from_slice(&unit.to_le_bytes());
        }
    }
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

/// Copy `bytes` into a movable global block, which is how the clipboard
/// takes data.
fn global(bytes: &[u8]) -> Result<HGLOBAL, String> {
    // SAFETY: a fresh allocation of `bytes.len()` bytes, written then unlocked.
    unsafe {
        let mem = GlobalAlloc(GMEM_MOVEABLE, bytes.len()).map_err(|e| e.to_string())?;
        let ptr = GlobalLock(mem) as *mut u8;
        if ptr.is_null() {
            let _ = GlobalFree(Some(mem));
            return Err("Could not lock clipboard memory.".into());
        }
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, bytes.len());
        let _ = GlobalUnlock(mem);
        Ok(mem)
    }
}

unsafe fn set(format: u32, bytes: &[u8]) -> Result<(), String> {
    let mem = global(bytes)?;
    if let Err(e) = SetClipboardData(format, Some(HANDLE(mem.0))) {
        let _ = GlobalFree(Some(mem));
        return Err(e.to_string());
    }
    Ok(())
}

/// The paths in a file-drop block.
unsafe fn dropped_paths(hdrop: HDROP) -> Vec<PathBuf> {
    let count = DragQueryFileW(hdrop, u32::MAX, None);
    (0..count)
        .filter_map(|i| {
            let len = DragQueryFileW(hdrop, i, None) as usize;
            let mut buf = vec![0u16; len + 1];
            let got = DragQueryFileW(hdrop, i, Some(&mut buf)) as usize;
            (got > 0).then(|| PathBuf::from(std::ffi::OsString::from_wide(&buf[..got])))
        })
        .collect()
}

/// Copy the clipboard's text out. Not zeroed in place: it belongs to the
/// clipboard, not to us.
unsafe fn read_text(global: HGLOBAL) -> Option<Zeroizing<String>> {
    let ptr = GlobalLock(global) as *const u16;
    if ptr.is_null() {
        return None;
    }
    let cap = (GlobalSize(global) / 2).min(MAX_TEXT_UNITS);
    let units = std::slice::from_raw_parts(ptr, cap);
    let len = units.iter().position(|&c| c == 0).unwrap_or(cap);
    let text = Zeroizing::new(String::from_utf16_lossy(&units[..len]));
    let _ = GlobalUnlock(global);
    (!text.trim().is_empty()).then_some(text)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Copy like another app would.
    pub fn set_clipboard_text(text: &str) -> Result<(), String> {
        let wide: Vec<u8> = text.encode_utf16().chain([0]).flat_map(u16::to_le_bytes).collect();
        let _open = Open::new()?;
        unsafe {
            EmptyClipboard().map_err(|e| e.to_string())?;
            set(CF_UNICODETEXT.0 as u32, &wide)
        }
    }

    /// The block we build is one the shell itself reads back as a file list,
    /// checked through `DragQueryFileW` on a plain global block, no clipboard.
    #[test]
    fn drop_block_reads_back_through_the_shell() {
        let paths = [Path::new(r"C:\Users\Zoë\AppData\Local\Temp\handoff-default\Key request from Zoë.handoff"), Path::new(r"D:\b.handoff")];
        let mem = global(&drop_files(&paths)).unwrap();
        let read = unsafe { dropped_paths(HDROP(mem.0)) };
        unsafe {
            let _ = GlobalFree(Some(mem));
        }
        assert_eq!(read, paths.iter().map(|p| p.to_path_buf()).collect::<Vec<_>>());
    }

    /// The file and the text fallback both land, and paste sees the file first.
    #[test]
    #[ignore = "uses the real clipboard"]
    fn offer_file_then_paste_finds_the_file() {
        let path = std::env::temp_dir().join("handoff-clipboard-test.handoff");
        offer_file(&path, "-----BEGIN HANDOFF-----".into()).unwrap();
        match read_clipboard().unwrap() {
            Clip::Files(files) => assert_eq!(files, vec![path]),
            _ => panic!("paste did not find the file"),
        }
        let _open = Open::new().unwrap();
        assert!(unsafe { IsClipboardFormatAvailable(CF_UNICODETEXT.0 as u32).is_ok() }, "no text fallback");
    }
}
