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
mod session;
mod slots;
mod store;
mod tpm;
mod vault;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, DragDropEvent, Emitter, Manager, State, WebviewWindow, WindowEvent};
use zeroize::Zeroizing;

use crate::core::{Answer, Clipboard, Handoff, View};
use crate::session::Session;
use crate::vault::Keep;

const MAIN: &str = "main";
const EXPIRY_CHECK: Duration = Duration::from_secs(60);
const LOCK_CHECK: Duration = Duration::from_secs(5);
/// A check this late means the computer was asleep.
const SLEPT: Duration = Duration::from_secs(60);
/// A program asking while locked waits this long for you to unlock.
const UNLOCK_WAIT: Duration = Duration::from_secs(120);

/// Tell the screens to refresh. While Safe Copy is on, the window stays above
/// other apps so you can copy a key from them; it keeps its size, so nothing
/// jumps between screens. A program asking for a key brings the window forward.
fn changed(app: &AppHandle) {
    let Some(handoff) = app.try_state::<Arc<Session>>().and_then(|s| s.current()) else {
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

/// Bring the main window forward, for a program waiting on you.
fn raise(app: &AppHandle) {
    if let Some(main) = app.get_webview_window(MAIN) {
        let _ = main.unminimize();
        let _ = main.show();
        let _ = main.set_focus();
    }
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

type S<'a> = State<'a, Arc<Session>>;

/// The open core, for a command from the window; locked, the command fails.
/// Using it counts as activity, so the idle lock waits.
struct H(Arc<Handoff>);

impl std::ops::Deref for H {
    type Target = Handoff;
    fn deref(&self) -> &Handoff {
        &self.0
    }
}

impl<'de, R: tauri::Runtime> tauri::ipc::CommandArg<'de, R> for H {
    fn from_command(command: tauri::ipc::CommandItem<'de, R>) -> Result<Self, tauri::ipc::InvokeError> {
        let session = command.message.webview().state::<Arc<Session>>().inner().clone();
        session.touch(now_ms()).map(H).map_err(Into::into)
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

#[derive(Serialize)]
struct GateView {
    /// setup (no passphrase yet), locked or open.
    state: &'static str,
    /// An older profile whose keys move behind the new passphrase.
    migrating: bool,
    /// Shown once, after setup, to write down.
    recovery: Option<String>,
    min_passphrase: usize,
    /// A TPM PIN unlocks it, so the lock screen asks for that first.
    pin: bool,
    min_pin: usize,
}

#[tauri::command]
fn gate(s: S) -> GateView {
    let (state, migrating) = s.state();
    GateView {
        state,
        migrating,
        recovery: s.recovery.lock().unwrap_or_else(|e| e.into_inner()).as_ref().map(|c| c.to_string()),
        min_passphrase: slots::MIN_PASSPHRASE,
        pin: s.has_pin(),
        min_pin: slots::MIN_PIN,
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
    opening(app, Box::new(move |locked, session| {
        let (store, settings, code) = locked.set_up(&passphrase)?;
        *session.recovery.lock().unwrap_or_else(|e| e.into_inner()) = Some(code);
        Ok((store, settings))
    }))
    .await
}

#[tauri::command]
async fn unlock(app: AppHandle, passphrase: String) -> Result<(), String> {
    let passphrase = Zeroizing::new(passphrase);
    opening(app, Box::new(move |locked, _| locked.unlock(&passphrase))).await
}

#[tauri::command]
async fn recover(app: AppHandle, code: String, passphrase: String) -> Result<(), String> {
    let (code, passphrase) = (Zeroizing::new(code), Zeroizing::new(passphrase));
    opening(app, Box::new(move |locked, _| locked.recover(&code, &passphrase))).await
}

#[tauri::command]
async fn unlock_pin(app: AppHandle, pin: String) -> Result<(), String> {
    let pin = Zeroizing::new(pin);
    opening(app, Box::new(move |locked, _| locked.unlock_pin(&mut *tpm::open()?, &pin))).await
}

#[tauri::command]
async fn add_pin(app: AppHandle, h: H, pin: String) -> Result<(), String> {
    let pin = Zeroizing::new(pin);
    let handoff = h.0.clone();
    let result = tauri::async_runtime::spawn_blocking(move || handoff.add_pin(&pin)).await.map_err(|e| e.to_string())?;
    done(&app, result)
}

#[tauri::command]
fn remove_pin(app: AppHandle, h: H) -> Result<(), String> {
    done(&app, h.remove_pin())
}

#[tauri::command]
fn recovery_done(app: AppHandle, s: S) {
    s.recovery.lock().unwrap_or_else(|e| e.into_inner()).take();
    changed(&app);
}

#[tauri::command]
fn remember(app: AppHandle, s: S, on: bool) -> Result<(), String> {
    done(&app, s.remember(on))
}

#[tauri::command]
fn lock(app: AppHandle, s: S) -> Result<(), String> {
    done(&app, s.lock())
}

/// Open the locked profile with `how`, then open the files that waited.
async fn opening(app: AppHandle, how: session::How<'static>) -> Result<(), String> {
    let result = tauri::async_runtime::spawn_blocking(move || {
        let session = app.state::<Arc<Session>>().inner().clone();
        session.open(how, now_ms())?;
        let files = std::mem::take(&mut *session.files.lock().unwrap_or_else(|e| e.into_inner()));
        open_files(&app, &files, false);
        changed(&app);
        Ok(())
    })
    .await;
    result.map_err(|e| e.to_string())?
}

/// Not activity: the screens refresh on every change, including the clock.
#[tauri::command]
fn view(s: S) -> Result<View, String> {
    Ok(s.current().ok_or("KeyHandoff is locked.")?.view())
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
fn send_kept(app: AppHandle, h: H, contact: String, key: String) -> Result<(), String> {
    done(&app, h.send_kept(&contact, &key))
}

#[tauri::command]
fn send_kept_to_team(app: AppHandle, h: H, team: String, key: String) -> Result<(), String> {
    done(&app, h.send_kept_to_team(&team, &key))
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
    let Some(session) = app.try_state::<Arc<Session>>() else { return };
    let Some(handoff) = session.current() else {
        // Locked: they open once you unlock.
        session.files.lock().unwrap_or_else(|e| e.into_inner()).extend_from_slice(files);
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
            gate, suggest, set_up, unlock, recover, recovery_done, lock, remember, unlock_pin, add_pin, remove_pin, view, send_kept, send_kept_to_team, copy_card, send_to, save_team, delete_team, send_to_team, welcome, add, retry, paste, seal, keep_own, copy_sealed, copy_request, drag_out, close_safe, set_keep,
            forget, copy_pointer, set_name, answer, dismiss, ready
        ])
        .setup(move |app| {
            let handle = app.handle().clone();
            let (profile, starter) = (args.profile.clone(), handle.clone());
            let start: session::Start = Box::new(move |store, settings| {
                let outbox = os::runtime_dir().join(format!("handoff-{profile}"));
                let notify = starter.clone();
                Handoff::open(store, settings, outbox, Clipboard::system(), Box::new(move || changed(&notify)))
            });
            let session = Arc::new(Session::new(store::profile_dir(&args.profile), &args.profile, start)?);
            app.manage(session.clone());
            if let Err(e) = session.open_if_remembered(now_ms()) {
                eprintln!("keyhandoff: {e}");
            }

            // Programs can ask while locked: the window comes forward and
            // they wait for the unlock.
            let asking = handle.clone();
            let serving = session.clone();
            let serve: os::ServeKeys = Arc::new(move |peer, stream| {
                if serving.current().is_none() {
                    raise(&asking);
                }
                match serving.wait_open(UNLOCK_WAIT) {
                    Some(handoff) => pointer::serve(&handoff, &peer, stream),
                    None => pointer::refuse(stream, "KeyHandoff is locked. Unlock it and try again."),
                }
            });
            if let Err(e) = os::listen_for_keys(&args.profile, serve) {
                eprintln!("keyhandoff: programs can't ask for keys: {e}");
            }

            open_files(&handle, &args.files, false);
            let launches = handle.clone();
            os::listen_for_launches(&args.profile, move |files| open_files(&launches, &files, true))?;

            // Kept-for-a-while keys and old requests are deleted on schedule,
            // and the vault locks when you're away: idle, asleep or the
            // screen locked. A tick that comes late means the computer slept.
            let ticker = handle.clone();
            std::thread::spawn(move || {
                let (mut last, mut expiry) = (now_ms(), now_ms());
                loop {
                    std::thread::sleep(LOCK_CHECK);
                    let now = now_ms();
                    let slept = now.saturating_sub(last) > SLEPT.as_millis() as u64;
                    last = now;
                    let locked = if session.remembered() {
                        false
                    } else if slept || os::screen_locked() {
                        session.current().is_some() && session.lock().is_ok()
                    } else {
                        session.lock_if_idle(now)
                    };
                    if locked || now.saturating_sub(expiry) >= EXPIRY_CHECK.as_millis() as u64 {
                        expiry = now;
                        changed(&ticker);
                    }
                }
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
                if let Some(h) = window.app_handle().try_state::<Arc<Session>>().and_then(|s| s.current()) {
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
