//! Drag a file out of a window with the shell's own data object, which
//! carries `CF_HDROP` and the shell formats; the default drop source draws the
//! drag image. `SHDoDragDrop` blocks until the drop, so it runs as its own
//! task on the UI thread (which has OLE set up) rather than inside the
//! command that asked for it.

use std::path::Path;

use ::windows::core::HSTRING;
use ::windows::Win32::Foundation::HWND;
use ::windows::Win32::System::Com::IDataObject;
use ::windows::Win32::System::Ole::{IDropSource, DROPEFFECT_COPY};
use ::windows::Win32::UI::Shell::{BHID_DataObject, IShellItem, SHCreateItemFromParsingName, SHDoDragDrop};

pub fn drag_file(window: &tauri::WebviewWindow, file: &Path, _text: String) -> Result<(), String> {
    let hwnd = window.hwnd().map_err(|e| e.to_string())?.0 as isize;
    let file = file.to_path_buf();
    window
        .run_on_main_thread(move || {
            if let Err(e) = start(hwnd, &file) {
                eprintln!("keyhandoff: dragging the file out failed: {e}");
            }
        })
        .map_err(|e| e.to_string())
}

fn start(hwnd: isize, file: &Path) -> Result<(), String> {
    // SAFETY: plain shell calls on an absolute path and our own window.
    unsafe {
        let item: IShellItem = SHCreateItemFromParsingName(&HSTRING::from(file.as_os_str()), None).map_err(|e| e.to_string())?;
        let data: IDataObject = item.BindToHandler(None, &BHID_DataObject).map_err(|e| e.to_string())?;
        SHDoDragDrop(Some(HWND(hwnd as *mut _)), &data, None::<&IDropSource>, DROPEFFECT_COPY).map_err(|e| e.to_string())?;
    }
    Ok(())
}
