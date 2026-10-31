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
mod slots;
mod store;
mod vault;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, DragDropEvent, Emitter, Manager, State, WebviewWindow, WindowEvent};
use zeroize::Zeroizing;

use crate::core::{Answer, Clipboard, Handoff, View};
use crate::store::{Locked, Settings, Store};
use crate::vault::Keep;

const MAIN: &str = "main";
const EXPIRY_CHECK: Duration = Duration::from_secs(60);

type H<'a> = State<'a, Arc<Handoff>>;

/// Tell the screens to refresh. While Safe Copy is on, the window stays above
/// other apps so you can copy a key from them; it keeps its size, so nothing
/// jumps between screens. A program asking for a key brings the window forward.
fn changed(app: &AppHandle) {
    let Some(handoff) = app.try_state::<Arc<Handoff>>() else {
        let _ = app.emit("changed", ());
        return;
    };
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

/// Whether Safe Copy last put the window on top, so it's only changed on the switch.
static ON_TOP: Mutex<bool> = Mutex::new(false);

fn set_safe_copy_mode(win: &WebviewWindow, on: bool) {
    let mut on_top = ON_TOP.lock().unwrap_or_else(|e| e.into_inner());
    if *on_top != on {
        *on_top = on;
        let _ = win.set_always_on_top(on);
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

/// The profile until the passphrase opens it. Everything else waits: the
/// `Handoff` core, and the socket programs ask for keys over, start only then.
struct Gate {
    profile: String,
    locked: Mutex<Option<Locked>>,
    /// Files opened before unlock, opened right after it.
    files: Mutex<Vec<PathBuf>>,
    /// The recovery code from setup, shown until you say it's written down.
    recovery: Mutex<Option<Zeroizing<String>>>,
}

type G<'a> = State<'a, Arc<Gate>>;

#[derive(Serialize)]
struct GateView {
    /// setup (no passphrase yet), locked or open.
    state: &'static str,
    /// An older profile whose keys move behind the new passphrase.
    migrating: bool,
    /// Shown once, after setup, to write down.
    recovery: Option<String>,
    min_passphrase: usize,
}

#[tauri::command]
fn gate(g: G) -> GateView {
    let locked = g.locked.lock().unwrap_or_else(|e| e.into_inner());
    let state = match locked.as_ref() {
        Some(l) if l.needs_setup() => "setup",
        Some(_) => "locked",
        None => "open",
    };
    GateView {
        state,
        migrating: locked.as_ref().is_some_and(Locked::migrating),
        recovery: g.recovery.lock().unwrap_or_else(|e| e.into_inner()).as_ref().map(|c| c.to_string()),
        min_passphrase: slots::MIN_PASSPHRASE,
    }
}

/// Five random words to use as the passphrase.
#[tauri::command]
fn suggest() -> String {
    slots::suggest().to_string()
}

/// Argon2id takes about a second, so these run off the main thread.
#[tauri::command]
async fn set_up(app: AppHandle, passphrase: String) -> Result<(), String> {
    let passphrase = Zeroizing::new(passphrase);
    opening(app, move |locked, gate| {
        let (store, settings, code) = locked.set_up(&passphrase)?;
        *gate.recovery.lock().unwrap_or_else(|e| e.into_inner()) = Some(code);
        Ok((store, settings))
    })
    .await
}

#[tauri::command]
async fn unlock(app: AppHandle, passphrase: String) -> Result<(), String> {
    let passphrase = Zeroizing::new(passphrase);
    opening(app, move |locked, _| locked.unlock(&passphrase)).await
}

#[tauri::command]
async fn recover(app: AppHandle, code: String, passphrase: String) -> Result<(), String> {
    let (code, passphrase) = (Zeroizing::new(code), Zeroizing::new(passphrase));
    opening(app, move |locked, _| locked.recover(&code, &passphrase)).await
}

#[tauri::command]
fn recovery_done(app: AppHandle, g: G) {
    g.recovery.lock().unwrap_or_else(|e| e.into_inner()).take();
    changed(&app);
}

/// Open the locked profile with `how`, then start everything that needs it.
async fn opening(
    app: AppHandle,
    how: impl FnOnce(&mut Locked, &Gate) -> Result<(Store, Settings), String> + Send + 'static,
) -> Result<(), String> {
    let result = tauri::async_runtime::spawn_blocking(move || {
        let gate = app.state::<Arc<Gate>>().inner().clone();
        let mut locked = gate.locked.lock().unwrap_or_else(|e| e.into_inner());
        let profile = locked.as_mut().ok_or("KeyHandoff is already unlocked.")?;
        let (store, settings) = how(profile, &gate)?;
        start(&app, &gate, store, settings)?;
        *locked = None;
        drop(locked);
        let files = std::mem::take(&mut *gate.files.lock().unwrap_or_else(|e| e.into_inner()));
        open_files(&app, &files, false);
        changed(&app);
        Ok(())
    })
    .await;
    result.map_err(|e| e.to_string())?
}

fn start(app: &AppHandle, gate: &Gate, store: Store, settings: Settings) -> Result<(), String> {
    let outbox = os::runtime_dir().join(format!("handoff-{}", gate.profile));
    let notify_handle = app.clone();
    let handoff = Handoff::open(store, settings, outbox, Clipboard::system(), Box::new(move || changed(&notify_handle)))?;
    app.manage(handoff.clone());
    if let Err(e) = os::listen_for_keys(&gate.profile, Arc::new(move |peer, stream| pointer::serve(&handoff, &peer, stream))) {
        eprintln!("keyhandoff: programs can't ask for keys: {e}");
    }
    Ok(())
}

#[tauri::command]
fn view(h: H) -> View {
    h.view()
}

#[tauri::command]
fn copy_card(app: AppHandle, h: H) -> Result<(), String> {
    done(&app, h.copy_card())
}

#[tauri::command]
fn save_team(app: AppHandle, h: H, id: Option<String>, name: String, members: Vec<String>) -> Result<String, String> {
    let result = h.save_team(id.as_deref(), &name, &members);
    changed(&app);
    result
}

#[tauri::command]
fn delete_team(app: AppHandle, h: H, id: String) -> Result<(), String> {
    done(&app, h.delete_team(&id))
}

#[tauri::command]
fn send_to_team(app: AppHandle, h: H, id: String) -> Result<(), String> {
    done(&app, h.send_to_team(&id))
}

#[tauri::command]
fn welcome(app: AppHandle, h: H, team: String, member: String) -> Result<(), String> {
    done(&app, h.welcome(&team, &member))
}

#[tauri::command]
fn send_to(app: AppHandle, h: H, id: String) -> Result<(), String> {
    done(&app, h.send_to(&id))
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
    let Some(handoff) = app.try_state::<Arc<Handoff>>() else {
        // Locked: they open once you unlock.
        if let Some(gate) = app.try_state::<Arc<Gate>>() {
            gate.files.lock().unwrap_or_else(|e| e.into_inner()).extend_from_slice(files);
        }
        if let (true, Some(main)) = (raise, app.get_webview_window(MAIN)) {
            let _ = main.show();
            let _ = main.unminimize();
            let _ = main.set_focus();
        }
        return;
    };
    for file in files {
        if let Err(e) = handoff.open_path(file) {
            handoff.problem(e);
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
            gate, suggest, set_up, unlock, recover, recovery_done, view, copy_card, send_to, save_team, delete_team, send_to_team, welcome, add, retry, paste, seal, keep_own, copy_sealed, copy_request, drag_out, close_safe, set_keep,
            forget, copy_pointer, set_name, answer, dismiss, ready
        ])
        .setup(move |app| {
            let locked = Locked::open(&args.profile)?;
            app.manage(Arc::new(Gate {
                profile: args.profile.clone(),
                locked: Mutex::new(Some(locked)),
                files: Mutex::new(Vec::new()),
                recovery: Mutex::new(None),
            }));

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
