//! Drag a file out of a window with AppKit: one dragging item carrying the
//! file's URL and, for text-only drop targets, the armored text, drawn with
//! the file's own Finder icon. The screens call this on mouse-down, so the
//! session starts from a drag event made at the pointer's current place.

use std::cell::OnceCell;
use std::path::Path;

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, NSObjectProtocol, ProtocolObject};
use objc2::{AnyThread, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSDragOperation, NSDraggingContext, NSDraggingItem, NSDraggingSession, NSDraggingSource, NSEvent, NSEventModifierFlags,
    NSEventType, NSPasteboardItem, NSPasteboardTypeFileURL, NSPasteboardTypeString, NSPasteboardWriting, NSWindow, NSWorkspace,
};
use objc2_foundation::{NSArray, NSPoint, NSProcessInfo, NSRect, NSSize, NSString};

define_class!(
    // SAFETY: NSObject has no subclassing requirements, and this class has
    // no Drop and no ivars.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "KeyHandoffDragSource"]
    struct DragSource;

    unsafe impl NSObjectProtocol for DragSource {}

    unsafe impl NSDraggingSource for DragSource {
        #[unsafe(method(draggingSession:sourceOperationMaskForDraggingContext:))]
        fn operations(&self, _session: &NSDraggingSession, _context: NSDraggingContext) -> NSDragOperation {
            NSDragOperation::Copy
        }
    }
);

thread_local! {
    /// AppKit doesn't keep the source alive, so one lives for the app's life.
    static SOURCE: OnceCell<Retained<DragSource>> = const { OnceCell::new() };
}

pub fn drag_file(window: &tauri::WebviewWindow, file: &Path, text: String) -> Result<(), String> {
    let ns_window = window.ns_window().map_err(|e| e.to_string())? as usize;
    let file = file.to_path_buf();
    window
        .run_on_main_thread(move || {
            let mtm = MainThreadMarker::new().expect("run_on_main_thread runs on the main thread");
            // SAFETY: Tauri's NSWindow for this window, alive while it is.
            let win: &NSWindow = unsafe { &*(ns_window as *const NSWindow) };
            if let Err(e) = start(mtm, win, &file, &text) {
                eprintln!("keyhandoff: dragging the file out failed: {e}");
            }
        })
        .map_err(|e| e.to_string())
}

fn start(mtm: MainThreadMarker, win: &NSWindow, file: &Path, text: &str) -> Result<(), String> {
    let view = win.contentView().ok_or("The window has no view.")?;
    let path = NSString::from_str(&file.to_string_lossy());

    let item = NSPasteboardItem::new();
    // SAFETY: AppKit constants, set before main runs.
    let (url_type, text_type) = unsafe { (NSPasteboardTypeFileURL, NSPasteboardTypeString) };
    item.setString_forType(&NSString::from_str(&super::clipboard::file_uri(file)), url_type);
    item.setString_forType(&NSString::from_str(text), text_type);
    let writer = ProtocolObject::<dyn NSPasteboardWriting>::from_retained(item);
    let dragging = NSDraggingItem::initWithPasteboardWriter(NSDraggingItem::alloc(), &writer);

    let at = win.mouseLocationOutsideOfEventStream();
    let in_view = view.convertPoint_fromView(at, None);
    let icon = NSWorkspace::sharedWorkspace().iconForFile(&path);
    let frame = NSRect::new(NSPoint::new(in_view.x - 24.0, in_view.y - 24.0), NSSize::new(48.0, 48.0));
    // SAFETY: an NSImage is valid dragging contents.
    unsafe { dragging.setDraggingFrame_contents(frame, Some(&**icon as &AnyObject)) };

    let event = NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
        NSEventType::LeftMouseDragged,
        at,
        NSEventModifierFlags::empty(),
        NSProcessInfo::processInfo().systemUptime(),
        win.windowNumber(),
        None,
        0,
        1,
        1.0,
    )
    .ok_or("Could not start dragging.")?;

    let source = SOURCE.with(|cell| cell.get_or_init(|| unsafe { msg_send![DragSource::alloc(mtm), init] }).clone());
    let items = NSArray::from_retained_slice(&[dragging]);
    view.beginDraggingSessionWithItems_event_source(&items, &event, ProtocolObject::from_ref(&*source));
    Ok(())
}
