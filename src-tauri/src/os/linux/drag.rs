//! Drag a file out of a window with GTK, offering the file and, for text-only
//! drop targets, the armored text. Call on the main thread while the mouse
//! button is down.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use gtk::glib::ObjectExt;
use gtk::prelude::*;
use gtk::{TargetList, gdk};

const URI: u32 = 0;
const TEXT: u32 = 1;

pub fn drag_file(window: &tauri::WebviewWindow, file: &Path, text: String) -> Result<(), String> {
    let win = window.gtk_window().map_err(|e| e.to_string())?;
    let uri = super::clipboard::file_uri(file);
    let targets = TargetList::new(&[]);
    targets.add_uri_targets(URI);
    targets.add_text_targets(TEXT);

    let handlers = Rc::new(RefCell::new(Vec::new()));
    handlers.borrow_mut().push(win.connect_drag_data_get(move |_, _, data, info, _| {
        if info == URI {
            data.set_uris(&[uri.as_str()]);
        } else {
            data.set_text(&text);
        }
    }));
    let ended = handlers.clone();
    handlers.borrow_mut().push(win.connect_drag_end(move |w, _| {
        for id in ended.borrow_mut().drain(..) {
            w.disconnect(id);
        }
    }));

    let started = win.drag_begin_with_coordinates(
        &targets,
        gdk::DragAction::COPY,
        gdk::ffi::GDK_BUTTON1_MASK as i32,
        None,
        -1,
        -1,
    );
    match started {
        Some(context) => {
            context.drag_set_icon_name("package-x-generic", 0, 0);
            Ok(())
        }
        None => {
            for id in handlers.borrow_mut().drain(..) {
                win.disconnect(id);
            }
            Err("Could not start dragging.".into())
        }
    }
}
