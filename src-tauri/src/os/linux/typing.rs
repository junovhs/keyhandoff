//! X11 only. No clipboard or shell commands. The spare keycode is restored on
//! every exit; the server is grabbed only around a focus check + one key pair.
use std::{thread, time::Duration};
use x11rb::{
    connection::Connection,
    protocol::{xproto::*, xtest::ConnectionExt as _},
    rust_connection::RustConnection,
    CURRENT_TIME, NONE,
};

type Result<T> = std::result::Result<T, String>;
fn err(e: impl std::fmt::Display) -> String {
    format!("X11 typing: {e}")
}

#[derive(Clone, Debug, PartialEq)]
pub struct Target {
    pub title: String,
    window: Window,
    focus: Window,
    /// `WM_CLASS`, which the shared controller checks against the chat and
    /// mail apps it refuses.
    pub class: String,
    pid: u32,
}

pub struct Desktop {
    conn: RustConnection,
    root: Window,
    num_lock: u16,
}
impl Desktop {
    pub fn open() -> Result<Self> {
        if std::env::var("XDG_SESSION_TYPE").as_deref() == Ok("wayland")
            || std::env::var_os("WAYLAND_DISPLAY").is_some()
        {
            return Err(
                "Typing is available on Linux X11. Wayland support is still to come.".into(),
            );
        }
        Self::connect(None)
    }
    fn connect(display: Option<&str>) -> Result<Self> {
        let (conn, screen) = x11rb::connect(display).map_err(err)?;
        let root = conn.setup().roots[screen].root;
        conn.xtest_get_version(2, 2)
            .map_err(err)?
            .reply()
            .map_err(err)?;
        let modifier = conn
            .get_modifier_mapping()
            .map_err(err)?
            .reply()
            .map_err(err)?;
        let setup = conn.setup();
        let mapping = conn
            .get_keyboard_mapping(setup.min_keycode, setup.max_keycode - setup.min_keycode + 1)
            .map_err(err)?
            .reply()
            .map_err(err)?;
        let width = mapping.keysyms_per_keycode as usize;
        let mut num_lock = 0;
        for (group, codes) in modifier
            .keycodes
            .chunks(modifier.keycodes.len() / 8)
            .enumerate()
        {
            if codes.iter().any(|&code| {
                code >= setup.min_keycode
                    && mapping
                        .keysyms
                        .get(
                            (code - setup.min_keycode) as usize * width
                                ..(code - setup.min_keycode) as usize * width + width,
                        )
                        .is_some_and(|s| s.contains(&0xff7f))
            }) {
                num_lock |= 1 << group;
            }
        }
        Ok(Self {
            conn,
            root,
            num_lock,
        })
    }
    fn atom(&self, name: &str) -> Result<Atom> {
        Ok(self
            .conn
            .intern_atom(false, name.as_bytes())
            .map_err(err)?
            .reply()
            .map_err(err)?
            .atom)
    }
    fn property(&self, win: Window, name: &str) -> Result<GetPropertyReply> {
        self.conn
            .get_property(false, win, self.atom(name)?, AtomEnum::ANY, 0, 1024)
            .map_err(err)?
            .reply()
            .map_err(err)
    }
    fn number(&self, win: Window, name: &str) -> Result<u32> {
        Ok(self
            .property(win, name)?
            .value32()
            .and_then(|mut v| v.next())
            .unwrap_or(0))
    }
    fn text(&self, win: Window, name: &str) -> Result<String> {
        Ok(String::from_utf8_lossy(&self.property(win, name)?.value)
            .replace('\0', " ")
            .trim()
            .to_string())
    }
    fn top(&self, mut win: Window) -> Result<Window> {
        for _ in 0..64 {
            if self.property(win, "WM_STATE")?.type_ != NONE {
                return Ok(win);
            }
            let tree = self
                .conn
                .query_tree(win)
                .map_err(err)?
                .reply()
                .map_err(err)?;
            if tree.parent == self.root || tree.parent == NONE {
                return Ok(win);
            }
            win = tree.parent;
        }
        Err("Couldn't identify the destination window.".into())
    }
    /// None is one of our own windows. Refused/unknown destinations clear any
    /// earlier selection rather than silently keeping a stale destination.
    pub fn capture(&self) -> Result<Option<Target>> {
        let focus = self
            .conn
            .get_input_focus()
            .map_err(err)?
            .reply()
            .map_err(err)?
            .focus;
        if focus <= 1 || focus == self.root {
            return Err("Click the field you want to type into.".into());
        }
        let window = self.top(focus)?;
        let class = self.text(window, "WM_CLASS")?;
        let pid = self.number(window, "_NET_WM_PID")?;
        if pid == std::process::id() || class.to_lowercase().contains("keyhandoff") {
            return Ok(None);
        }
        if class.is_empty() {
            return Err("This window doesn't identify its app. Choose another destination.".into());
        }
        let title = self.text(window, "_NET_WM_NAME")?;
        let title = if title.is_empty() {
            self.text(window, "WM_NAME")?
        } else {
            title
        };
        Ok(Some(Target {
            title: if title.is_empty() {
                class.clone()
            } else {
                title
            },
            window,
            focus,
            class,
            pid,
        }))
    }
    fn same(&self, target: &Target) -> Result<()> {
        let active = self.number(self.root, "_NET_ACTIVE_WINDOW")?;
        if active != NONE && active != target.window {
            return Err("Typing stopped because the destination changed. Some characters may already be in the field.".into());
        }
        let actual = self.capture()?;
        if !actual.as_ref().is_some_and(|a| {
            a.window == target.window
                && a.focus == target.focus
                && a.class == target.class
                && a.pid == target.pid
        }) {
            return Err("Typing stopped because focus moved or the destination changed. Some characters may already be in the field.".into());
        }
        // Do not neutralise modifiers: a held Ctrl/Alt could turn a key into a command.
        let mask = u16::from(
            self.conn
                .query_pointer(self.root)
                .map_err(err)?
                .reply()
                .map_err(err)?
                .mask,
        );
        if mask & (0xff & !self.num_lock) != 0 {
            return Err("Release modifier keys and turn off Caps Lock before typing.".into());
        }
        Ok(())
    }
    pub fn type_text(
        &self,
        target: &Target,
        text: &str,
        alive: &dyn Fn() -> bool,
        started: &mut dyn FnMut() -> Result<()>,
    ) -> Result<()> {
        crate::typing::validate(text.as_bytes())?;
        self.same(target)?;
        let setup = self.conn.setup();
        let count = setup.max_keycode - setup.min_keycode + 1;
        let map = self
            .conn
            .get_keyboard_mapping(setup.min_keycode, count)
            .map_err(err)?
            .reply()
            .map_err(err)?;
        let width = map.keysyms_per_keycode as usize;
        // Only an actually unused code, never a real or modifier key.
        let index = map
            .keysyms
            .chunks(width)
            .rposition(|syms| syms.iter().all(|s| *s == 0))
            .ok_or("No spare X11 keycode is available.")?;
        let code = setup.min_keycode + index as u8;
        let original = &map.keysyms[index * width..(index + 1) * width];
        let _mapping = Mapping {
            desktop: self,
            code,
            original,
        };
        let mut began = false;
        for c in text.chars() {
            if !alive() {
                return Err("Typing stopped: KeyHandoff locked, timed out or was cancelled. Some characters may already be in the field.".into());
            }
            let sym = if (c as u32) <= 0xff {
                c as u32
            } else {
                0x01000000 | c as u32
            };
            // The mapping is public X11 state, so wipe it as soon as the
            // receiving toolkit has processed the pair (also via Drop on errors).
            self.conn
                .change_keyboard_mapping(1, code, map.keysyms_per_keycode, &vec![sym; width])
                .map_err(err)?
                .check()
                .map_err(err)?;
            thread::sleep(Duration::from_millis(12));
            self.same(target)?;
            // Persist Once consumption without holding the X server frozen.
            if !began {
                started()?;
                began = true;
            }
            self.conn.grab_server().map_err(err)?.check().map_err(err)?;
            let guard = ServerGrab(self);
            self.same(target)?;
            if !alive() {
                return Err("Typing stopped: KeyHandoff locked or timed out.".into());
            }
            let pressed = self
                .conn
                .xtest_fake_input(KEY_PRESS_EVENT, code, CURRENT_TIME, self.root, 0, 0, 0)
                .map_err(err)
                .and_then(|cookie| cookie.check().map_err(err));
            // Try release even when the press request reports an error.
            let released = self
                .conn
                .xtest_fake_input(KEY_RELEASE_EVENT, code, CURRENT_TIME, self.root, 0, 0, 0)
                .map_err(err)
                .and_then(|cookie| cookie.check().map_err(err));
            pressed?;
            released?;
            drop(guard);
            thread::sleep(Duration::from_millis(12));
            self.conn
                .change_keyboard_mapping(1, code, map.keysyms_per_keycode, original)
                .map_err(err)?
                .check()
                .map_err(err)?;
        }
        Ok(())
    }
}
struct ServerGrab<'a>(&'a Desktop);
impl Drop for ServerGrab<'_> {
    fn drop(&mut self) {
        let _ = self.0.conn.ungrab_server();
        let _ = self.0.conn.flush();
    }
}
struct Mapping<'a> {
    desktop: &'a Desktop,
    code: u8,
    original: &'a [u32],
}
impl Drop for Mapping<'_> {
    fn drop(&mut self) {
        if let Ok(cookie) = self.desktop.conn.change_keyboard_mapping(
            1,
            self.code,
            self.original.len() as u8,
            self.original,
        ) {
            let _ = cookie.check();
        }
        let _ = self.desktop.conn.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{BufRead, BufReader, Write},
        process::{Command, Stdio},
    };
    /// Runs against a separate GTK process on an isolated X server. The real
    /// toolkit resolves MappingNotify and Unicode, rather than a mock decoder.
    #[test]
    #[ignore = "requires KEYHANDOFF_TEST_DISPLAY (isolated Xephyr) and Python GTK3"]
    fn x11_types_unicode_and_stops_at_focus_changes() {
        let display = std::env::var("KEYHANDOFF_TEST_DISPLAY")
            .expect("use an isolated X server, never the real desktop");
        let desktop = Desktop::connect(Some(&display)).unwrap();
        let source = r#"
import gi, sys, json
gi.require_version('Gtk', '3.0')
from gi.repository import Gtk, GLib
window = Gtk.Window(title='Stripe Dashboard')
window.set_wmclass('typing-test', 'TypingTest')
entry = Gtk.Entry()
entry.connect('changed', lambda _: window.set_title('Stripe Dashboard - edited'))
window.add(entry)
window.show_all()
window.present()
entry.grab_focus()
def ready():
    print('READY', flush=True)
    return False
GLib.timeout_add(200, ready)
def command(source, condition):
    cmd = sys.stdin.readline().strip()
    if cmd == 'get': print(json.dumps(entry.get_text()), flush=True)
    elif cmd == 'clear': entry.set_text(''); print('OK', flush=True)
    elif cmd == 'quit': Gtk.main_quit(); return False
    return True
GLib.io_add_watch(sys.stdin, GLib.IO_IN, command)
Gtk.main()
"#;
        let mut child = Command::new("/usr/bin/python3")
            .args(["-u", "-c", source])
            .env("DISPLAY", &display)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        struct ChildGuard(std::process::Child);
        impl Drop for ChildGuard {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
        let mut input = child.stdin.take().unwrap();
        let mut output = BufReader::new(child.stdout.take().unwrap());
        let _guard = ChildGuard(child);
        let mut line = String::new();
        output.read_line(&mut line).unwrap();
        assert_eq!(line.trim(), "READY");
        // No WM is needed in the isolated server.
        let tree = desktop
            .conn
            .query_tree(desktop.root)
            .unwrap()
            .reply()
            .unwrap();
        let window = tree
            .children
            .into_iter()
            .find(|&w| {
                desktop
                    .text(w, "WM_CLASS")
                    .unwrap_or_default()
                    .contains("TypingTest")
            })
            .unwrap();
        desktop
            .conn
            .set_input_focus(InputFocus::PARENT, window, CURRENT_TIME)
            .unwrap()
            .check()
            .unwrap();
        let target = desktop.capture().unwrap().unwrap();
        assert!(target.title.contains("Stripe"));
        let setup = desktop.conn.setup();
        let before = desktop
            .conn
            .get_keyboard_mapping(setup.min_keycode, setup.max_keycode - setup.min_keycode + 1)
            .unwrap()
            .reply()
            .unwrap()
            .keysyms;
        let text = "sk_synthetic_Az09-Ä€+/=~😀";
        let mut began = 0;
        desktop
            .type_text(&target, text, &|| true, &mut || {
                began += 1;
                Ok(())
            })
            .unwrap();
        assert_eq!(began, 1);
        writeln!(input, "get").unwrap();
        input.flush().unwrap();
        line.clear();
        output.read_line(&mut line).unwrap();
        assert_eq!(serde_json::from_str::<String>(&line).unwrap(), text);
        let after = desktop
            .conn
            .get_keyboard_mapping(setup.min_keycode, setup.max_keycode - setup.min_keycode + 1)
            .unwrap()
            .reply()
            .unwrap()
            .keysyms;
        assert_eq!(before, after, "the spare keycode is restored");
        writeln!(input, "clear").unwrap();
        input.flush().unwrap();
        line.clear();
        output.read_line(&mut line).unwrap();
        // Move focus after the first pair. No later character can go to root.
        let checks = std::cell::Cell::new(0);
        let alive = || {
            let n = checks.get() + 1;
            checks.set(n);
            if n == 3 {
                desktop
                    .conn
                    .set_input_focus(InputFocus::POINTER_ROOT, desktop.root, CURRENT_TIME)
                    .unwrap()
                    .check()
                    .unwrap();
            }
            true
        };
        assert!(desktop
            .type_text(&target, "abcdef", &alive, &mut || Ok(()))
            .is_err());
        writeln!(input, "get").unwrap();
        input.flush().unwrap();
        line.clear();
        output.read_line(&mut line).unwrap();
        assert_eq!(serde_json::from_str::<String>(&line).unwrap(), "a");
        let after = desktop
            .conn
            .get_keyboard_mapping(setup.min_keycode, setup.max_keycode - setup.min_keycode + 1)
            .unwrap()
            .reply()
            .unwrap()
            .keysyms;
        assert_eq!(
            before, after,
            "mapping is restored after an interrupted send"
        );
        desktop
            .conn
            .set_input_focus(InputFocus::PARENT, window, CURRENT_TIME)
            .unwrap()
            .check()
            .unwrap();
        let class = desktop.atom("WM_CLASS").unwrap();
        use x11rb::wrapper::ConnectionExt as _;
        desktop
            .conn
            .change_property8(
                PropMode::REPLACE,
                window,
                class,
                AtomEnum::STRING,
                b"slack\0Slack\0",
            )
            .unwrap()
            .check()
            .unwrap();
        assert!(crate::typing::capture(&desktop).unwrap_err().contains("chat or mail"));
        desktop
            .conn
            .change_property8(
                PropMode::REPLACE,
                window,
                class,
                AtomEnum::STRING,
                b"keyhandoff\0KeyHandoff\0",
            )
            .unwrap()
            .check()
            .unwrap();
        assert!(
            desktop.capture().unwrap().is_none(),
            "never type into our own app"
        );
    }
}
