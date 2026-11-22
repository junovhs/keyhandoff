//! Typing a key into another app's field, where no backend is built yet
//! (Windows: HAND-14). Same interface as `linux/typing.rs` and `macos/typing.rs`.

#[derive(Clone, PartialEq)]
pub struct Target {
    pub title: String,
    /// The app's identity, which the shared controller checks against the
    /// chat and mail apps it refuses.
    pub class: String,
}

pub struct Desktop;

impl Desktop {
    pub fn open() -> Result<Self, String> {
        Err("Typing is available on macOS and Linux X11. Windows support is still to come.".into())
    }

    pub fn capture(&self) -> Result<Option<Target>, String> {
        unreachable!()
    }

    pub fn type_text(&self, _: &Target, _: &str, _: &dyn Fn() -> bool, _: &mut dyn FnMut() -> Result<(), String>) -> Result<(), String> {
        unreachable!()
    }
}
