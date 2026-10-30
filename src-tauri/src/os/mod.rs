//! Per-OS backends behind one small interface: the clipboard (Safe Copy
//! capture, putting a file on the clipboard, reading a paste), the master key
//! the OS protects for each profile, and dragging a file out of a window.
//!
//! Linux (X11) was built first, then Windows (clipboard history, DPAPI); each
//! is a folder with the same functions.

use std::path::PathBuf;

use keyhandoff_seal::Secret;
use zeroize::Zeroizing;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::*;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use self::windows::*;

#[cfg(not(any(target_os = "linux", windows)))]
mod unsupported;
#[cfg(not(any(target_os = "linux", windows)))]
pub use unsupported::*;

/// How a Safe Copy capture ended.
pub enum Captured {
    /// The key, and a note for the user if wiping it fell short (for
    /// example, Windows clipboard history had to be cleared).
    Caught(Secret, Option<String>),
    TimedOut,
    Cancelled,
    /// Linux only so far; Windows reports setup errors from `arm_capture`.
    #[cfg_attr(windows, allow(dead_code))]
    Failed(String),
}

/// What a paste found on the clipboard.
pub enum Clip {
    Files(Vec<PathBuf>),
    Text(Zeroizing<String>),
    Empty,
}

/// A profile's 32-byte master key, held by the OS and zeroized on drop.
pub struct MasterKey(pub(crate) Zeroizing<[u8; 32]>);

impl MasterKey {
    #[cfg(test)]
    pub fn for_tests() -> Self {
        use rand_core::{OsRng, RngCore};
        let mut key = Zeroizing::new([0u8; 32]);
        OsRng.fill_bytes(&mut key[..]);
        Self(key)
    }
}
