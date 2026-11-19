//! Click Type on a saved key, choose a field, then confirm in a floating panel.
//! The panel never takes keyboard focus. Only ids/names/state cross IPC.
use crate::{core::KeyView, session::Session};
use serde::Serialize;
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter, Manager};

#[cfg(target_os = "linux")]
mod x11;
#[cfg(target_os = "linux")]
use x11::{Desktop, Target};
#[cfg(not(target_os = "linux"))]
mod unsupported {
    #[derive(Clone, PartialEq)]
    pub struct Target {
        pub title: String,
    }
    pub struct Desktop;
    impl Desktop {
        pub fn open() -> Result<Self, String> {
            Err("Typing is available on Linux X11. Windows support is still to come.".into())
        }
        pub fn capture(&self) -> Result<Option<Target>, String> {
            unreachable!()
        }
        pub fn type_text(
            &self,
            _: &Target,
            _: &str,
            _: &dyn Fn() -> bool,
            _: &mut dyn FnMut() -> Result<(), String>,
        ) -> Result<(), String> {
            unreachable!()
        }
    }
}
#[cfg(not(target_os = "linux"))]
use unsupported::{Desktop, Target};

const PANEL: &str = "typing";
const ARMED: Duration = Duration::from_secs(30);

#[derive(Default)]
pub struct Typing {
    inner: Mutex<Inner>,
}
#[derive(Default)]
struct Inner {
    serial: u64,
    pending: Option<Pending>,
    message: Option<String>,
    busy: bool,
}
struct Pending {
    token: u64,
    revision: u64,
    key: KeyView,
    target: Option<Target>,
    deadline: Instant,
    ends_at: u64,
    session: std::sync::Weak<crate::core::Handoff>,
}
#[derive(Serialize)]
pub struct TypingView {
    token: u64,
    revision: u64,
    key: Option<KeyView>,
    target: Option<String>,
    ends_at: u64,
    message: Option<String>,
    busy: bool,
}
impl Typing {
    pub fn availability() -> Result<(), String> {
        Desktop::open().map(|_| ())
    }
    pub fn view(&self) -> TypingView {
        let i = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let p = i.pending.as_ref().filter(|p| Instant::now() < p.deadline);
        TypingView {
            token: p.map_or(i.serial, |p| p.token),
            revision: p.map_or(0, |p| p.revision),
            key: p.map(|p| p.key.clone()),
            target: p.and_then(|p| p.target.as_ref().map(|t| t.title.clone())),
            ends_at: p.map_or(0, |p| p.ends_at),
            message: i.message.clone(),
            busy: i.busy,
        }
    }
    pub fn start(self: &Arc<Self>, app: &AppHandle, id: &str) -> Result<(), String> {
        Self::availability()?;
        let session = app.state::<Arc<Session>>();
        let h = session.touch(crate::now_ms())?;
        if h.busy() {
            return Err("Finish Safe Copy or the program request before typing.".into());
        }
        let key = h
            .view()
            .keys
            .into_iter()
            .find(|k| k.id == id)
            .ok_or("That key is no longer here.")?;
        let token = {
            let mut i = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            if i.busy {
                return Err("A key is already being typed.".into());
            }
            i.serial += 1;
            let token = i.serial;
            i.message = None;
            i.pending = Some(Pending {
                token,
                revision: 0,
                key,
                target: None,
                deadline: Instant::now() + ARMED,
                ends_at: crate::now_ms() + ARMED.as_millis() as u64,
                session: Arc::downgrade(&h),
            });
            token
        };
        let panel = app
            .get_webview_window(PANEL)
            .ok_or("The typing panel is unavailable.")?;
        // A short panel at the top right leaves the destination's fields clear.
        // It deliberately cannot take input focus, even when its buttons are clicked.
        if let Ok(Some(m)) = panel.current_monitor() {
            let x = m.position().x + m.size().width as i32 - (380.0 * m.scale_factor()) as i32 - 24;
            let _ = panel.set_position(tauri::PhysicalPosition::new(x, m.position().y + 48));
        }
        panel.show().map_err(|e| e.to_string())?;
        let _ = app.emit("typing-changed", ());
        let (this, app, original) = (self.clone(), app.clone(), Arc::downgrade(&h));
        std::thread::spawn(move || {
            let desktop = Desktop::open();
            loop {
                std::thread::sleep(Duration::from_millis(40));
                let current = app.state::<Arc<Session>>().current();
                let mut i = this.inner.lock().unwrap_or_else(|e| e.into_inner());
                let Some(p) = i.pending.as_mut().filter(|p| p.token == token) else {
                    break;
                };
                if Instant::now() >= p.deadline
                    || !current
                        .as_ref()
                        .zip(original.upgrade())
                        .is_some_and(|(a, b)| Arc::ptr_eq(a, &b))
                {
                    i.pending = None;
                    i.message = Some("Disarmed. Choose Type on the key to try again.".into());
                    let _ = app.emit("typing-changed", ());
                    break;
                }
                let captured = desktop
                    .as_ref()
                    .map_err(Clone::clone)
                    .and_then(|d| d.capture());
                let old = (p.target.clone(), i.message.clone());
                match captured {
                    Ok(Some(t)) => {
                        i.pending.as_mut().unwrap().target = Some(t);
                        i.message = None;
                    }
                    Ok(None) => {
                        i.pending.as_mut().unwrap().target = None;
                        i.message = None;
                    }
                    Err(e) => {
                        i.pending.as_mut().unwrap().target = None;
                        i.message = Some(e);
                    }
                }
                if old.0 != i.pending.as_ref().unwrap().target {
                    i.pending.as_mut().unwrap().revision += 1;
                }
                if old
                    != (
                        i.pending.as_ref().unwrap().target.clone(),
                        i.message.clone(),
                    )
                {
                    let _ = app.emit("typing-changed", ());
                }
            }
        });
        Ok(())
    }
    pub fn cancel(&self, app: &AppHandle) {
        let mut i = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        i.serial += 1; // also stops an in-flight send on its next character
        i.pending = None;
        i.message = None;
        if let Some(w) = app.get_webview_window(PANEL) {
            let _ = w.hide();
        }
        let _ = app.emit("typing-changed", ());
    }
    pub fn send(&self, app: &AppHandle, token: u64, revision: u64) -> Result<(), String> {
        let (target, key, deadline, original) = {
            let mut i = self.inner.lock().unwrap_or_else(|e| e.into_inner());
            let p = i
                .pending
                .as_ref()
                .filter(|p| p.token == token && Instant::now() < p.deadline)
                .ok_or("Disarmed. Choose Type on the key to try again.")?;
            if p.revision != revision {
                return Err(
                    "The destination changed. Check the panel and click Type again.".into(),
                );
            }
            let target = p
                .target
                .clone()
                .ok_or("Click the destination field first.")?;
            let values = (target, p.key.clone(), p.deadline, p.session.clone());
            i.pending = None;
            i.busy = true;
            values
        };
        let result = (|| {
            let desktop = Desktop::open()?;
            // Because the panel never takes focus, compare the live field to
            // the one named by the panel. Never guess or force focus elsewhere.
            if desktop.capture()?.as_ref() != Some(&target) {
                return Err("The destination changed. Choose Type again.".into());
            }
            let session = app.state::<Arc<Session>>();
            let h = session.touch(crate::now_ms())?;
            if !original.upgrade().is_some_and(|old| Arc::ptr_eq(&old, &h)) {
                return Err("KeyHandoff was locked. Choose Type again.".into());
            }
            let alive = || {
                Instant::now() < deadline
                    && self.inner.lock().unwrap_or_else(|e| e.into_inner()).serial == token
                    && session
                        .current()
                        .is_some_and(|current| Arc::ptr_eq(&current, &h))
            };
            h.type_key(&key.id, |bytes, started| {
                let text = validate(bytes)?;
                desktop.type_text(&target, text, &alive, started)
            })
        })();
        let mut i = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        i.busy = false;
        i.message = Some(match &result {
            Ok(()) => format!("Typed {} into {}.", key.label, target.title),
            Err(e) => e.clone(),
        });
        let _ = app.emit("typing-changed", ());
        crate::changed(app);
        result
    }
}

pub(super) fn blocked_class(class: &str) -> bool {
    let class = class.to_lowercase();
    [
        "slack",
        "teams",
        "discord",
        "messages",
        "thunderbird",
        "evolution",
        "geary",
        "outlook",
        "mail",
        "signal",
        "telegram",
        "whatsapp",
    ]
    .iter()
    .any(|app| class.contains(app))
}
/// Reject anything that could navigate/submit a field or act as a terminal command.
fn validate(bytes: &[u8]) -> Result<&str, String> {
    let text = std::str::from_utf8(bytes).map_err(|_| "This key isn't text and can't be typed.")?;
    if text.is_empty() || text.len() > 16_384 || text.chars().any(char::is_control) {
        return Err(
            "Typing needs a single line of text without control characters (up to 16 KiB).".into(),
        );
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn refuse_controls_and_binary_without_echoing_them() {
        for bytes in [
            b"secret\n".as_slice(),
            b"secret\t",
            b"secret\x1b",
            b"\xff",
            b"",
        ] {
            assert!(validate(bytes).is_err());
        }
        assert_eq!(
            validate("sk_synthetic-Ä€+/=~".as_bytes()).unwrap(),
            "sk_synthetic-Ä€+/=~"
        );
        for class in [
            "slack Slack",
            "Microsoft Teams",
            "discord",
            "Thunderbird",
            "org.gnome.Geary",
        ] {
            assert!(blocked_class(class), "{class}");
        }
        assert!(!blocked_class("chromium Chromium"));
    }
}
