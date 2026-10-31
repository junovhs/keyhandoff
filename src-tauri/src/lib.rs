//! KeyHandoff backend. This Rust side alone holds secret values;
//! commands give the screens names, states and ids only.
//!
//! `core` holds every flow, `os` the per-OS clipboard, key protection,
//! drag-out and key socket, `store` and `vault` the protected files, and
//! `pointer` what programs asking for keys send. This file is the Tauri glue:
//! commands, the one window and opening files.

mod core;
mod os;
mod pointer;
mod store;
mod vault;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tauri::{AppHandle, DragDropEvent, Emitter, LogicalSize, Manager, PhysicalSize, State, WebviewWindow, WindowEvent};

use crate::core::{Answer, Clipboard, Handoff, View};
use crate::store::Store;
use crate::vault::Keep;

const MAIN: &str = "main";
/// The window's compact size while Safe Copy is on. The rest of the time the
/// window is whatever size the person made it.
const SAFE_SIZE: (f64, f64) = (400.0, 540.0);
const EXPIRY_CHECK: Duration = Duration::from_secs(60);

type H<'a> = State<'a, Arc<Handoff>>;

/// Tell the screens to refresh. While Safe Copy is on, the window turns
/// compact and stays above other apps so you can copy a key from them. A
/// program asking for a key brings the window forward.
fn changed(app: &AppHandle) {
    let Some(handoff) = app.try_state::<Arc<Handoff>>() else { return };
    let view = handoff.view();
    let state = view.safe.state;
    if let Some(win) = app.get_webview_window(MAIN) {
        set_safe_copy_mode(&win, state != "off");
        if view.program.is_some() {
            let _ = win.unminimize();
            let _ = win.show();
        }
        // Naming the caught key, or answering a program, needs the keyboard.
        if state == "caught" || view.program.is_some() {
            let _ = win.set_focus();
        }
    }
    let _ = app.emit("changed", ());
}

/// The person's window size from before Safe Copy turned on, put back after.
static SIZE_BEFORE_SAFE_COPY: Mutex<Option<PhysicalSize<u32>>> = Mutex::new(None);

fn set_safe_copy_mode(win: &WebviewWindow, on: bool) {
    let mut before = SIZE_BEFORE_SAFE_COPY.lock().unwrap_or_else(|e| e.into_inner());
    if on && before.is_none() {
        *before = win.inner_size().ok();
        let _ = win.set_always_on_top(true);
        let _ = win.set_size(LogicalSize::new(SAFE_SIZE.0, SAFE_SIZE.1));
    } else if !on {
        if let Some(size) = before.take() {
            let _ = win.set_always_on_top(false);
            let _ = win.set_size(size);
        }
    }
    if on {
        let _ = win.unminimize();
        let _ = win.show();
    }
}

/// Refresh after a command and pass its result on.
fn done(app: &AppHandle, result: Result<(), String>) -> Result<(), String> {
    changed(app);
    result
}

#[tauri::command]
fn view(h: H) -> View {
    h.view()
}

#[tauri::command]
fn ask(app: AppHandle, h: H) -> Result<(), String> {
    done(&app, h.ask())
}

#[tauri::command]
fn add(app: AppHandle, h: H) -> Result<(), String> {
    done(&app, h.add())
}

#[tauri::command]
fn retry(app: AppHandle, h: H) -> Result<(), String> {
    done(&app, h.retry())
}

#[tauri::command]
fn paste(app: AppHandle, h: H) -> Result<(), String> {
    done(&app, h.paste())
}

#[tauri::command]
fn seal(app: AppHandle, h: H, label: String) -> Result<(), String> {
    done(&app, h.seal(&label))
}

#[tauri::command]
fn keep_own(app: AppHandle, h: H, label: String, keep: Keep) -> Result<(), String> {
    done(&app, h.keep_own(&label, keep))
}

#[tauri::command]
fn copy_sealed(app: AppHandle, h: H) -> Result<(), String> {
    done(&app, h.copy_sealed())
}

#[tauri::command]
fn copy_request(app: AppHandle, h: H) -> Result<(), String> {
    done(&app, h.copy_request())
}

/// Start dragging the sealed or request file out of the calling window.
/// Sync commands run on the main thread, which GTK needs.
#[tauri::command]
fn drag_out(window: WebviewWindow, h: H, what: String) -> Result<(), String> {
    let (file, text) = h.drag_source(&what).ok_or("Nothing to drag.")?;
    os::drag_file(&window, &file, text)
}

#[tauri::command]
fn close_safe(app: AppHandle, h: H) {
    h.close_safe();
    changed(&app);
}

#[tauri::command]
fn set_keep(app: AppHandle, h: H, id: String, keep: Keep) -> Result<(), String> {
    done(&app, h.set_keep(&id, keep))
}

#[tauri::command]
fn forget(app: AppHandle, h: H, id: String) -> Result<(), String> {
    done(&app, h.forget(&id))
}

#[tauri::command]
fn copy_pointer(app: AppHandle, h: H, id: String) -> Result<(), String> {
    done(&app, h.copy_pointer(&id))
}

#[tauri::command]
fn set_name(app: AppHandle, h: H, name: String) -> Result<(), String> {
    done(&app, h.set_name(&name))
}

#[tauri::command]
fn answer(app: AppHandle, h: H, answer: Answer) -> Result<(), String> {
    done(&app, h.answer(answer))
}

#[tauri::command]
fn dismiss(app: AppHandle, h: H) {
    h.dismiss();
    changed(&app);
}

/// Called by the main window after its first paint: show it and log cold start.
#[tauri::command]
fn ready(app: AppHandle, started: State<Started>) -> Result<(), String> {
    let main = app.get_webview_window(MAIN).ok_or("no main window")?;
    if !main.is_visible().unwrap_or(true) {
        main.show().map_err(|e| e.to_string())?;
        main.set_focus().map_err(|e| e.to_string())?;
        eprintln!("keyhandoff: cold start to visible main window: {} ms", started.0.elapsed().as_millis());
    }
    Ok(())
}

struct Started(Instant);

/// Open files given on the command line, by a later launch or by a drop.
/// Errors become the main window's notice. `raise` brings the main window
/// forward; at startup the screens show it themselves once painted.
fn open_files(app: &AppHandle, files: &[PathBuf], raise: bool) {
    let Some(handoff) = app.try_state::<Arc<Handoff>>() else { return };
    for file in files {
        if let Err(e) = handoff.open_path(file) {
            handoff.notice(e);
        }
    }
    if !raise {
        changed(app);
        return;
    }
    if let Some(main) = app.get_webview_window(MAIN) {
        let _ = main.show();
        let _ = main.unminimize();
        if handoff.view().safe.state == "off" {
            let _ = main.set_focus();
        }
    }
    changed(app);
}

struct Args {
    profile: String,
    files: Vec<PathBuf>,
}

/// `keyhandoff [--profile NAME] [FILE...]`; `KEYHANDOFF_PROFILE` also picks the profile.
fn parse_args() -> Result<Args, String> {
    let mut profile = std::env::var("KEYHANDOFF_PROFILE").ok().filter(|p| !p.is_empty());
    let mut files = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--profile" {
            profile = Some(args.next().ok_or("--profile needs a name")?);
        } else if let Some(p) = arg.strip_prefix("--profile=") {
            profile = Some(p.to_string());
        } else if !arg.starts_with('-') {
            let path = PathBuf::from(&arg);
            files.push(path.canonicalize().unwrap_or(path));
        }
    }
    let profile = profile.unwrap_or_else(|| "default".into());
    if !store::valid_profile(&profile) {
        return Err(format!("profile names use a-z, 0-9, - and _: {profile}"));
    }
    Ok(Args { profile, files })
}

pub fn run() {
    let started = Instant::now();
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("keyhandoff: {e}");
            std::process::exit(2);
        }
    };
    if os::forward_to_running(&args.profile, &args.files) {
        return;
    }
    tauri::Builder::default()
        .manage(Started(started))
        .invoke_handler(tauri::generate_handler![
            view, ask, add, retry, paste, seal, keep_own, copy_sealed, copy_request, drag_out, close_safe, set_keep,
            forget, copy_pointer, set_name, answer, dismiss, ready
        ])
        .setup(move |app| {
            let (store, settings) = Store::open(&args.profile)?;
            let outbox = os::runtime_dir().join(format!("handoff-{}", args.profile));
            let notify_handle = app.handle().clone();
            let handoff = Handoff::open(store, settings, outbox, Clipboard::system(), Box::new(move || changed(&notify_handle)))?;
            app.manage(handoff.clone());
            if let Err(e) = os::listen_for_keys(&args.profile, Arc::new(move |peer, stream| pointer::serve(&handoff, &peer, stream))) {
                eprintln!("keyhandoff: programs can't ask for keys: {e}");
            }

            let handle = app.handle().clone();
            open_files(&handle, &args.files, false);
            let launches = handle.clone();
            os::listen_for_launches(&args.profile, move |files| open_files(&launches, &files, true))?;

            // Kept-for-a-while keys and old requests are deleted on schedule.
            let ticker = handle.clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(EXPIRY_CHECK);
                changed(&ticker);
            });

            // Fallback: show the main window even if the screens never call `ready`.
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(1500));
                if let Some(main) = handle.get_webview_window(MAIN) {
                    if !main.is_visible().unwrap_or(true) {
                        eprintln!("keyhandoff: screens did not report ready; showing main window anyway");
                        let _ = main.show();
                    }
                }
            });
            Ok(())
        })
        .on_window_event(|window, event| match event {
            // Closing the window stops any Safe Copy and quits.
            WindowEvent::CloseRequested { .. } if window.label() == MAIN => {
                if let Some(h) = window.app_handle().try_state::<Arc<Handoff>>() {
                    h.close_safe();
                }
                window.app_handle().exit(0);
            }
            WindowEvent::DragDrop(DragDropEvent::Drop { paths, .. }) => open_files(window.app_handle(), paths, false),
            _ => {}
        })
        .run(tauri::generate_context!())
        .expect("error while running KeyHandoff");
}
