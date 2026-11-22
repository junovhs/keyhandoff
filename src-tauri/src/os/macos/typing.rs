//! Typing a key into another app's field on macOS, through the Accessibility
//! API. No clipboard and no shell commands.
//!
//! - The destination is the focused element of the focused app, as
//!   Accessibility reports it (or its focused window, for apps that don't
//!   expose their fields), named by the window's title. The app is named by
//!   its bundle id, which the shared controller checks against chat and mail.
//! - Each character is one key-down and key-up carrying its text, posted to
//!   that process only (`CGEventPostToPid`), with no modifiers. Before every
//!   character, the same app must be in front, the same element focused, and
//!   no modifier or Caps Lock held.
//! - Clicking the panel brings KeyHandoff forward on macOS (an app's window
//!   can refuse key focus, not activation), so while KeyHandoff is in front
//!   the last destination stands as long as its element still has focus in
//!   its app, and that app is brought back before typing.
//!
//! macOS asks you to allow KeyHandoff under Privacy & Security, Accessibility,
//! the first time; until then the panel says so.

use std::ffi::c_void;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use objc2_core_foundation::{CFBoolean, CFDictionary, CFRetained, CFString, CFType};

type Result<T> = std::result::Result<T, String>;

#[allow(non_upper_case_globals)]
#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrusted() -> bool;
    fn AXIsProcessTrustedWithOptions(options: *const c_void) -> bool;
    fn AXUIElementCreateSystemWide() -> *const c_void;
    fn AXUIElementCreateApplication(pid: libc::pid_t) -> *const c_void;
    fn AXUIElementCopyAttributeValue(element: *const c_void, attribute: *const c_void, value: *mut *const c_void) -> i32;
    fn AXUIElementGetPid(element: *const c_void, pid: *mut libc::pid_t) -> i32;
    static kAXTrustedCheckOptionPrompt: *const c_void;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFEqual(a: *const c_void, b: *const c_void) -> u8;
}

/// An owned Core Foundation object (an AXUIElement, or a value read from one).
struct Cf(CFRetained<CFType>);

// SAFETY: Core Foundation retain and release are thread-safe, and
// Accessibility calls may be made from any thread.
unsafe impl Send for Cf {}
unsafe impl Sync for Cf {}

impl Cf {
    /// Take ownership of a +1 reference; `None` for null.
    fn take(ptr: *const c_void) -> Option<Self> {
        let ptr = std::ptr::NonNull::new(ptr as *mut CFType)?;
        // SAFETY: the caller passes a +1 reference from a Create or Copy call.
        Some(Cf(unsafe { CFRetained::from_raw(ptr) }))
    }

    fn ptr(&self) -> *const c_void {
        (&raw const *self.0).cast()
    }

    fn attribute(&self, name: &str) -> Option<Cf> {
        let name = CFString::from_str(name);
        let mut value: *const c_void = std::ptr::null();
        // SAFETY: a valid element and attribute name; on success `value` is +1.
        let status = unsafe { AXUIElementCopyAttributeValue(self.ptr(), (&raw const *name).cast(), &mut value) };
        if status == 0 { Cf::take(value) } else { None }
    }

    fn string(&self) -> Option<String> {
        self.0.downcast_ref::<CFString>().map(|s| s.to_string())
    }

    fn pid(&self) -> Option<libc::pid_t> {
        let mut pid = 0;
        // SAFETY: a valid AXUIElement.
        (unsafe { AXUIElementGetPid(self.ptr(), &mut pid) } == 0 && pid > 0).then_some(pid)
    }
}

impl Clone for Cf {
    fn clone(&self) -> Self {
        Cf(self.0.clone())
    }
}

impl PartialEq for Cf {
    fn eq(&self, other: &Self) -> bool {
        // SAFETY: both are valid CF objects.
        unsafe { CFEqual(self.ptr(), other.ptr()) != 0 }
    }
}

impl std::fmt::Debug for Cf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AXUIElement")
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Target {
    pub title: String,
    /// The app's bundle id and name, which the shared controller checks
    /// against the chat and mail apps it refuses.
    pub class: String,
    pid: libc::pid_t,
    element: Cf,
}

/// The last destination outside KeyHandoff, kept while KeyHandoff is in front.
static LAST: Mutex<Option<Target>> = Mutex::new(None);

const NOT_ALLOWED: &str =
    "Allow KeyHandoff under System Settings, Privacy & Security, Accessibility, then choose Type on the key again.";

pub struct Desktop;

impl Desktop {
    /// Typing works on every macOS KeyHandoff runs on. Accessibility
    /// permission is checked when there's a destination to find, so the
    /// screens don't raise macOS's prompt just by showing the Type button.
    pub fn open() -> Result<Self> {
        Ok(Desktop)
    }

    /// The focused field outside KeyHandoff. `None` while KeyHandoff itself
    /// is in front and nothing was chosen yet.
    pub fn capture(&self) -> Result<Option<Target>> {
        trusted()?;
        let pid = focused_app()?;
        if pid == std::process::id() as libc::pid_t {
            let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
            if last.as_ref().is_some_and(|t| focused_in(t.pid).as_ref() != Some(&t.element)) {
                *last = None;
            }
            return Ok(last.clone());
        }
        let target = describe(pid)?;
        *LAST.lock().unwrap_or_else(|e| e.into_inner()) = Some(target.clone());
        Ok(Some(target))
    }

    pub fn type_text(&self, target: &Target, text: &str, alive: &dyn Fn() -> bool, started: &mut dyn FnMut() -> Result<()>) -> Result<()> {
        crate::typing::validate(text.as_bytes())?;
        trusted()?;
        bring_forward(target)?;
        let mut began = false;
        let mut units = [0u16; 2];
        for c in text.chars() {
            if !alive() {
                return Err("Typing stopped: KeyHandoff locked, timed out or was cancelled. Some characters may already be in the field.".into());
            }
            same(target)?;
            if !began {
                started()?;
                began = true;
            }
            post(target.pid, c.encode_utf16(&mut units))?;
            std::thread::sleep(Duration::from_millis(8));
        }
        Ok(())
    }
}

fn trusted() -> Result<()> {
    // SAFETY: a plain query.
    if unsafe { AXIsProcessTrusted() } {
        return Ok(());
    }
    // Ask macOS to show its prompt, once per run.
    static ASKED: std::sync::Once = std::sync::Once::new();
    ASKED.call_once(|| {
        // SAFETY: an Accessibility constant; the dictionary holds CF objects.
        let key = unsafe { &*kAXTrustedCheckOptionPrompt.cast::<CFString>() };
        let options = CFDictionary::<CFString, CFBoolean>::from_slices(&[key], &[CFBoolean::new(true)]);
        unsafe { AXIsProcessTrustedWithOptions((&raw const *options).cast()) };
    });
    Err(NOT_ALLOWED.into())
}

fn focused_app() -> Result<libc::pid_t> {
    // SAFETY: Create returns +1.
    let system = Cf::take(unsafe { AXUIElementCreateSystemWide() }).ok_or("Accessibility isn't available.")?;
    system.attribute("AXFocusedApplication").and_then(|app| app.pid()).ok_or_else(|| "Click the field you want to type into.".to_string())
}

/// The focused element of the app `pid`, or its focused window when the app
/// doesn't expose its fields (some web views and Electron apps).
fn focused_in(pid: libc::pid_t) -> Option<Cf> {
    // SAFETY: Create returns +1.
    let app = Cf::take(unsafe { AXUIElementCreateApplication(pid) })?;
    app.attribute("AXFocusedUIElement").or_else(|| app.attribute("AXFocusedWindow"))
}

fn describe(pid: libc::pid_t) -> Result<Target> {
    // SAFETY: Create returns +1.
    let app = Cf::take(unsafe { AXUIElementCreateApplication(pid) }).ok_or("Couldn't identify the destination app.")?;
    let element = app
        .attribute("AXFocusedUIElement")
        .or_else(|| app.attribute("AXFocusedWindow"))
        .ok_or("Click the field you want to type into.")?;
    let (bundle, name) = app_names(pid);
    if bundle.is_empty() && name.is_empty() {
        return Err("This window doesn't identify its app. Choose another destination.".into());
    }
    let class = format!("{bundle} {name}").trim().to_string();
    let title = app.attribute("AXFocusedWindow").and_then(|w| w.attribute("AXTitle")).and_then(|t| t.string()).unwrap_or_default();
    let title = if title.is_empty() { name.clone() } else if name.is_empty() { title } else { format!("{title} ({name})") };
    Ok(Target { title, class, pid, element })
}

/// The app's bundle id and its name as the Dock shows it.
fn app_names(pid: libc::pid_t) -> (String, String) {
    use objc2_app_kit::NSRunningApplication;
    objc2::rc::autoreleasepool(|_| match NSRunningApplication::runningApplicationWithProcessIdentifier(pid) {
        Some(app) => (
            app.bundleIdentifier().map(|s| s.to_string()).unwrap_or_default(),
            app.localizedName().map(|s| s.to_string()).unwrap_or_default(),
        ),
        None => (String::new(), String::new()),
    })
}

/// Bring the destination's app back in front (clicking the panel brought
/// KeyHandoff forward) and wait until it is.
fn bring_forward(target: &Target) -> Result<()> {
    use objc2_app_kit::{NSApplicationActivationOptions, NSRunningApplication};
    if focused_app().ok() != Some(target.pid) {
        let activated = objc2::rc::autoreleasepool(|_| {
            NSRunningApplication::runningApplicationWithProcessIdentifier(target.pid)
                // KeyHandoff is the active app here (its panel was clicked),
                // so it can hand activation back without forcing it.
                .is_some_and(|app| app.activateWithOptions(NSApplicationActivationOptions::empty()))
        });
        if !activated {
            return Err("The destination app has closed. Choose Type again.".into());
        }
        let deadline = Instant::now() + Duration::from_secs(1);
        while focused_app().ok() != Some(target.pid) {
            if Instant::now() >= deadline {
                return Err("The destination app didn't come forward. Click the field and choose Type again.".into());
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    same(target)
}

/// The destination still has focus, and no modifier or Caps Lock is held
/// (one could turn a typed key into a command).
fn same(target: &Target) -> Result<()> {
    if focused_app()? != target.pid || focused_in(target.pid).as_ref() != Some(&target.element) {
        return Err("Typing stopped because focus moved or the destination changed. Some characters may already be in the field.".into());
    }
    use objc2_core_graphics::{CGEventFlags, CGEventSource, CGEventSourceStateID};
    let held = CGEventSource::flags_state(CGEventSourceStateID::CombinedSessionState);
    let modifiers = CGEventFlags::MaskShift
        | CGEventFlags::MaskControl
        | CGEventFlags::MaskAlternate
        | CGEventFlags::MaskCommand
        | CGEventFlags::MaskAlphaShift
        | CGEventFlags::MaskSecondaryFn;
    if held.intersects(modifiers) {
        return Err("Release modifier keys and turn off Caps Lock before typing.".into());
    }
    Ok(())
}

/// One character, as a key-down and key-up carrying its text, to `pid` only.
#[allow(deprecated)]
fn post(pid: libc::pid_t, text: &[u16]) -> Result<()> {
    use objc2_core_graphics::{CGEventCreateKeyboardEvent, CGEventFlags, CGEventKeyboardSetUnicodeString, CGEventPostToPid, CGEventSetFlags};
    for down in [true, false] {
        let event = CGEventCreateKeyboardEvent(None, 0, down).ok_or("Couldn't make a keystroke.")?;
        CGEventSetFlags(Some(&event), CGEventFlags::empty());
        // SAFETY: `text` holds `len` UTF-16 units.
        unsafe { CGEventKeyboardSetUnicodeString(Some(&event), text.len() as _, text.as_ptr()) };
        CGEventPostToPid(pid, Some(&event));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Types into a real TextEdit document. Needs Accessibility permission
    /// for the test binary's terminal, and a logged-in, unlocked screen.
    #[test]
    #[ignore = "drives TextEdit; needs Accessibility permission"]
    fn macos_types_into_textedit_exactly() {
        use std::process::Command;
        let file = std::env::temp_dir().join(format!("kh-type-{}.txt", std::process::id()));
        std::fs::write(&file, "").unwrap();
        Command::new("/usr/bin/open").args(["-a", "TextEdit"]).arg(&file).status().unwrap();
        std::thread::sleep(Duration::from_secs(2));
        let desktop = Desktop::open().unwrap();
        let target = desktop.capture().unwrap().expect("TextEdit is in front");
        assert!(target.class.contains("com.apple.TextEdit"), "{target:?}");
        let typed = "sk_synthetic-Ä€+/=~ 😀";
        desktop.type_text(&target, typed, &|| true, &mut || Ok(())).unwrap();
        Command::new("/usr/bin/osascript").args(["-e", "tell application \"TextEdit\" to save front document", "-e", "tell application \"TextEdit\" to close front document"]).status().unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), typed);
        let _ = std::fs::remove_file(&file);
    }
}
