//! Every KeyHandoff flow, without Tauri: ask, Safe Copy,
//! seal, accept and vault, and add for yourself.
//!
//! Secret values live here and in `os`; [`View`] is what the screens get:
//! names, states and ids only.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use keyhandoff_seal::{armor, Identity, Keyring, Object, Payload, Request, Secret, DEFAULT_REQUEST_TTL, MAX_LABEL_BYTES, MAX_NAME_BYTES};
use serde::Serialize;

use crate::os::{self, Captured, Clip};
use crate::store::{Settings, Store};
use crate::vault::{Item, Keep, Vault};

/// Safe Copy disarms itself after this long.
pub const ARM_TIMEOUT: Duration = Duration::from_secs(60);
const MAX_FILE_BYTES: u64 = 64 * 1024;
const ALREADY_ACCEPTED: &str = "This key was already accepted and can't be opened.";

/// Called once when a Safe Copy capture ends.
pub type OnCaptured = Box<dyn FnOnce(Captured) + Send>;

/// The clipboard operations the flows need; tests swap in fakes.
pub struct Clipboard {
    pub arm: fn(Duration, Arc<AtomicBool>, OnCaptured) -> Result<(), String>,
    pub offer: fn(&Path, String) -> Result<(), String>,
    pub read: fn() -> Result<Clip, String>,
}

impl Clipboard {
    pub fn system() -> Self {
        Self { arm: |t, c, done| os::arm_capture(t, c, done), offer: os::offer_file, read: os::read_clipboard }
    }
}

/// Who a Safe Copy capture is for.
enum Target {
    You,
    Request(Request),
}

impl Target {
    fn for_name(&self) -> Option<String> {
        match self {
            Target::You => None,
            Target::Request(r) => Some(r.name().to_string()),
        }
    }

    fn for_computer(&self) -> Option<String> {
        match self {
            Target::You => None,
            Target::Request(r) => Some(r.requester().fingerprint()),
        }
    }
}

enum Safe {
    Off,
    Listening { target: Target, ends_at: u64, cancel: Arc<AtomicBool> },
    TimedOut { target: Target },
    Failed { target: Target, message: String },
    /// Caught and wiped from the clipboard; waiting for a name (and, for
    /// yourself, a keep-time).
    Caught { target: Target, secret: Secret, note: Option<String> },
    Sealed { for_name: String, file: PathBuf, text: String },
}

struct Core {
    store: Store,
    settings: Settings,
    identity: Identity,
    keyring: Keyring,
    vault: Vault,
    outbox: PathBuf,
    safe: Safe,
    /// Bumped on every arm so a late capture result is ignored.
    generation: u64,
    accepted: Option<[u8; 16]>,
    asked: Option<PathBuf>,
    notice: Option<String>,
}

pub struct Handoff {
    core: Mutex<Core>,
    clipboard: Clipboard,
    notify: Box<dyn Fn() + Send + Sync>,
    me: Weak<Handoff>,
}

// ---- what the screens see ----

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct KeyView {
    pub id: String,
    pub label: String,
    pub from: Option<String>,
    pub added_at: u64,
    pub expires_at: Option<u64>,
    pub keep: Keep,
}

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct SafeView {
    /// off, listening, timeout, failed, caught or sealed.
    pub state: &'static str,
    /// The requester's name; `None` when catching a key for yourself.
    pub for_name: Option<String>,
    /// The requester's computer fingerprint, to check who is asking.
    pub for_computer: Option<String>,
    /// When listening stops, in Unix milliseconds.
    pub ends_at: u64,
    pub file_name: Option<String>,
    pub message: Option<String>,
}

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct View {
    pub name: String,
    pub protection: String,
    pub keep: Keep,
    pub keys: Vec<KeyView>,
    pub pending: usize,
    pub safe: SafeView,
    pub accepted: Option<KeyView>,
    pub asked: Option<String>,
    pub notice: Option<String>,
}

impl Handoff {
    pub fn open(store: Store, settings: Settings, outbox: PathBuf, clipboard: Clipboard, notify: Box<dyn Fn() + Send + Sync>) -> Result<Arc<Self>, String> {
        let identity = store.identity()?;
        let keyring = store.keyring()?;
        let vault = store.vault()?;
        os::private_dir(&outbox).map_err(|e| e.to_string())?;
        clear_outbox(&outbox);
        let core = Core {
            store,
            settings,
            identity,
            keyring,
            vault,
            outbox,
            safe: Safe::Off,
            generation: 0,
            accepted: None,
            asked: None,
            notice: None,
        };
        let handoff = Arc::new_cyclic(|me| Self { core: Mutex::new(core), clipboard, notify, me: me.clone() });
        handoff.lock().tick()?;
        Ok(handoff)
    }

    fn lock(&self) -> MutexGuard<'_, Core> {
        self.core.lock().unwrap_or_else(|p| p.into_inner())
    }

    pub fn view(&self) -> View {
        let mut core = self.lock();
        let _ = core.tick();
        core.view()
    }

    /// Ask: make a request, write `Key request from <you>.handoff` and put it
    /// on the clipboard.
    pub fn ask(&self) -> Result<(), String> {
        let mut core = self.lock();
        let (request, pending) = keyhandoff_seal::create_request(&core.identity, &core.settings.name, now(), DEFAULT_REQUEST_TTL)
            .map_err(|e| e.to_string())?;
        core.keyring.insert(pending);
        core.store.save_keyring(&core.keyring)?;
        let text = armor::request_text(&request);
        let file = core.outbox.join(file_name("Key request from", request.name()));
        os::write_private(&file, text.as_bytes()).map_err(|e| e.to_string())?;
        (self.clipboard.offer)(&file, text)?;
        core.asked = Some(file);
        core.accepted = None;
        Ok(())
    }

    /// The request or sealed file to drag out, with its text.
    pub fn drag_source(&self, what: &str) -> Option<(PathBuf, String)> {
        let core = self.lock();
        let file = match (what, &core.safe) {
            ("sealed", Safe::Sealed { file, .. }) => file.clone(),
            ("request", _) => core.asked.clone()?,
            _ => return None,
        };
        let text = fs::read_to_string(&file).ok()?;
        Some((file, text))
    }

    /// Add: arm Safe Copy for a key of your own.
    pub fn add(&self) -> Result<(), String> {
        self.arm(Target::You)
    }

    /// Arm again after a timeout or failure, for the same person.
    pub fn retry(&self) -> Result<(), String> {
        let target = {
            let mut core = self.lock();
            match std::mem::replace(&mut core.safe, Safe::Off) {
                Safe::TimedOut { target } | Safe::Failed { target, .. } => target,
                other => {
                    core.safe = other;
                    return Err("Safe copy is not waiting to try again.".into());
                }
            }
        };
        self.arm(target)
    }

    /// Open a `.handoff` file (double-click, drop or paste).
    pub fn open_path(&self, path: &Path) -> Result<(), String> {
        let unreadable = || "This file could not be read.".to_string();
        let meta = fs::metadata(path).map_err(|_| unreadable())?;
        if meta.len() > MAX_FILE_BYTES {
            return Err("This is not a KeyHandoff file.".into());
        }
        let bytes = fs::read(path).map_err(|_| unreadable())?;
        if bytes.starts_with(b"HOF") {
            return self.open_object(&bytes);
        }
        let text = String::from_utf8(bytes).map_err(|_| "This is not a KeyHandoff file.".to_string())?;
        self.open_text(&text)
    }

    /// Paste: a copied `.handoff` file or its text.
    pub fn paste(&self) -> Result<(), String> {
        match (self.clipboard.read)()? {
            Clip::Files(files) => {
                let file = files
                    .iter()
                    .find(|f| f.extension().is_some_and(|e| e == "handoff"))
                    .or(files.first())
                    .ok_or("Nothing to paste.")?;
                self.open_path(file)
            }
            Clip::Text(text) => self.open_text(&text),
            Clip::Empty => Err("The clipboard is empty. Copy a KeyHandoff file or message first.".into()),
        }
    }

    pub fn open_text(&self, text: &str) -> Result<(), String> {
        let bytes = armor::decode(text).map_err(|e| e.to_string())?;
        self.open_object(&bytes)
    }

    fn open_object(&self, bytes: &[u8]) -> Result<(), String> {
        match keyhandoff_seal::parse_object(bytes).map_err(|e| e.to_string())? {
            Object::Request(request) => {
                let core = self.lock();
                if request.requester() == core.identity.public() {
                    return Err("This is your own request. Send it to the person who has the key.".into());
                }
                if request.is_expired(now()) {
                    return Err("This request ran out. Ask them to send a new one.".into());
                }
                drop(core);
                self.arm(Target::Request(request))
            }
            Object::Sealed(sealed) => self.lock().accept(&sealed),
        }
    }

    fn arm(&self, target: Target) -> Result<(), String> {
        let mut core = self.lock();
        core.close_safe();
        core.generation += 1;
        let generation = core.generation;
        let cancel = Arc::new(AtomicBool::new(false));
        let me = self.me.clone();
        (self.clipboard.arm)(
            ARM_TIMEOUT,
            cancel.clone(),
            Box::new(move |result| {
                if let Some(handoff) = me.upgrade() {
                    handoff.captured(generation, result);
                }
            }),
        )?;
        core.safe = Safe::Listening { target, ends_at: now_ms() + ARM_TIMEOUT.as_millis() as u64, cancel };
        core.accepted = None;
        Ok(())
    }

    fn captured(&self, generation: u64, result: Captured) {
        {
            let mut core = self.lock();
            if core.generation != generation {
                return;
            }
            let Safe::Listening { .. } = core.safe else { return };
            let Safe::Listening { target, .. } = std::mem::replace(&mut core.safe, Safe::Off) else { unreachable!() };
            core.safe = match result {
                Captured::Caught(secret, note) => Safe::Caught { target, secret, note },
                Captured::TimedOut => Safe::TimedOut { target },
                Captured::Failed(message) => Safe::Failed { target, message },
                Captured::Cancelled => Safe::Off,
            };
        }
        (self.notify)();
    }

    /// Seal the caught key for its requester under `label`, and forget it.
    pub fn seal(&self, label: &str) -> Result<(), String> {
        let label = clean_text(label, MAX_LABEL_BYTES).ok_or("Give the key a short name.")?;
        let mut core = self.lock();
        let Safe::Caught { target: Target::Request(request), secret, .. } = &core.safe else {
            return Err("There is no caught key to seal.".into());
        };
        let payload = Payload { label, note: core.settings.name.clone(), secret: Secret::new(secret.expose().to_vec()) };
        let sealed = keyhandoff_seal::seal(&core.identity, request, &payload, now()).map_err(|e| e.to_string())?;
        drop(payload);
        let for_name = request.name().to_string();
        let text = armor::sealed_text(&sealed, &for_name);
        let file = core.outbox.join(file_name("Key for", &for_name));
        os::write_private(&file, text.as_bytes()).map_err(|e| e.to_string())?;
        // Dropping the Caught state zeroizes the plaintext.
        core.safe = Safe::Sealed { for_name, file, text };
        Ok(())
    }

    /// Keep the key you caught for yourself.
    pub fn keep_own(&self, label: &str, keep: Keep) -> Result<(), String> {
        let label = clean_text(label, MAX_LABEL_BYTES).ok_or("Give the key a short name.")?;
        let mut core = self.lock();
        let Safe::Caught { target: Target::You, secret, .. } = &core.safe else {
            return Err("There is no caught key to keep.".into());
        };
        let secret = Secret::new(secret.expose().to_vec());
        let label = core.vault.free_name(&label);
        core.vault.add(&label, None, None, &secret, keep, now());
        core.store.save_vault(&core.vault)?;
        core.remember_keep(keep)?;
        core.safe = Safe::Off;
        core.notice = Some(format!("{label} is in your keys."));
        Ok(())
    }

    /// Put the sealed file on the clipboard.
    pub fn copy_sealed(&self) -> Result<(), String> {
        let core = self.lock();
        let Safe::Sealed { file, text, .. } = &core.safe else { return Err("Nothing is sealed.".into()) };
        (self.clipboard.offer)(file, text.clone())
    }

    /// Copy the request file again.
    pub fn copy_request(&self) -> Result<(), String> {
        let core = self.lock();
        let file = core.asked.as_ref().ok_or("No request was made.")?;
        let text = fs::read_to_string(file).map_err(|e| e.to_string())?;
        (self.clipboard.offer)(file, text)
    }

    /// Hide Safe Copy: stop listening, forget a caught key, delete a sealed file.
    pub fn close_safe(&self) {
        self.lock().close_safe();
    }

    pub fn set_keep(&self, id: &str, keep: Keep) -> Result<(), String> {
        let mut core = self.lock();
        let id = parse_id(id)?;
        if !core.vault.set_keep(id, keep) {
            return Err("That key is no longer here.".into());
        }
        core.store.save_vault(&core.vault)?;
        core.remember_keep(keep)
    }

    pub fn forget(&self, id: &str) -> Result<(), String> {
        let mut core = self.lock();
        let id = parse_id(id)?;
        if core.vault.remove(id) {
            core.store.save_vault(&core.vault)?;
        }
        if core.accepted == Some(id) {
            core.accepted = None;
        }
        Ok(())
    }

    pub fn set_name(&self, name: &str) -> Result<(), String> {
        let name = clean_text(name, MAX_NAME_BYTES).ok_or("Use a short name without special characters.")?;
        let mut core = self.lock();
        core.settings.name = name;
        core.store.save_settings(&core.settings)
    }

    /// Dismiss the accept sheet, the request sheet and any notice.
    pub fn dismiss(&self) {
        let mut core = self.lock();
        core.accepted = None;
        core.asked = None;
        core.notice = None;
    }

    pub fn notice(&self, text: String) {
        self.lock().notice = Some(text);
    }
}

impl Core {
    /// Drop whatever Safe Copy holds.
    fn close_safe(&mut self) {
        match std::mem::replace(&mut self.safe, Safe::Off) {
            Safe::Listening { cancel, .. } => cancel.store(true, Ordering::SeqCst),
            Safe::Sealed { file, .. } => {
                let _ = fs::remove_file(file);
            }
            _ => {}
        }
        self.generation += 1;
    }

    /// Open a sealed key: the one-time key is destroyed and saved first
    /// (sealing-adr.md), then the key goes into the vault with the sticky
    /// keep-time. The sheet that follows lets the receiver change it.
    fn accept(&mut self, sealed: &keyhandoff_seal::Sealed) -> Result<(), String> {
        let request_id = sealed.request_id();
        let opened = match self.keyring.open(sealed, now()) {
            Ok(opened) => opened,
            Err(keyhandoff_seal::Error::UnknownRequest) if self.vault.was_opened(request_id) => {
                return Err(ALREADY_ACCEPTED.into());
            }
            Err(e) => {
                // An expired entry was destroyed by the attempt.
                self.store.save_keyring(&self.keyring)?;
                return Err(e.to_string());
            }
        };
        self.store.save_keyring(&self.keyring)?;
        let from = Some(opened.payload.note.clone()).filter(|n| !n.is_empty());
        let keep = self.settings.keep;
        let id = self.vault.add(&opened.payload.label, from, Some(opened.sender.fingerprint()), &opened.payload.secret, keep, now());
        self.vault.mark_opened(request_id);
        self.store.save_vault(&self.vault)?;
        self.accepted = Some(id);
        self.asked = None;
        Ok(())
    }

    fn remember_keep(&mut self, keep: Keep) -> Result<(), String> {
        if self.settings.keep != keep {
            self.settings.keep = keep;
            self.store.save_settings(&self.settings)?;
        }
        Ok(())
    }

    /// Delete kept keys and one-time keys that ran out.
    fn tick(&mut self) -> Result<(), String> {
        let t = now();
        if self.vault.purge(t) > 0 {
            self.store.save_vault(&self.vault)?;
            if self.accepted.is_some_and(|id| !self.vault.items().iter().any(|i| i.id == id)) {
                self.accepted = None;
            }
        }
        if self.keyring.purge_expired(t) > 0 {
            self.store.save_keyring(&self.keyring)?;
        }
        Ok(())
    }

    fn view(&self) -> View {
        let keys: Vec<KeyView> = self.vault.items().iter().rev().map(key_view).collect();
        let accepted = self.accepted.and_then(|id| keys.iter().find(|k| k.id == hex(&id)).cloned());
        View {
            name: self.settings.name.clone(),
            protection: self.settings.protection.clone().unwrap_or_default(),
            keep: self.settings.keep,
            keys,
            pending: self.keyring.len(),
            safe: self.safe_view(),
            accepted,
            asked: self.asked.as_ref().map(|p| display_name(p)),
            notice: self.notice.clone(),
        }
    }

    fn safe_view(&self) -> SafeView {
        let base = |state, target: &Target| SafeView {
            state,
            for_name: target.for_name(),
            for_computer: target.for_computer(),
            ends_at: 0,
            file_name: None,
            message: None,
        };
        match &self.safe {
            Safe::Off => SafeView { state: "off", for_name: None, for_computer: None, ends_at: 0, file_name: None, message: None },
            Safe::Listening { target, ends_at, .. } => SafeView { ends_at: *ends_at, ..base("listening", target) },
            Safe::TimedOut { target } => base("timeout", target),
            Safe::Failed { target, message } => SafeView { message: Some(message.clone()), ..base("failed", target) },
            Safe::Caught { target, note, .. } => SafeView { message: note.clone(), ..base("caught", target) },
            Safe::Sealed { for_name, file, .. } => SafeView {
                state: "sealed",
                for_name: Some(for_name.clone()),
                for_computer: None,
                ends_at: 0,
                file_name: Some(display_name(file)),
                message: None,
            },
        }
    }
}

fn key_view(item: &Item) -> KeyView {
    KeyView {
        id: hex(&item.id),
        label: item.label.clone(),
        from: item.from.clone(),
        added_at: item.added_at,
        expires_at: item.expires_at,
        keep: item.keep,
    }
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

fn hex(id: &[u8; 16]) -> String {
    id.iter().map(|b| format!("{b:02x}")).collect()
}

fn parse_id(s: &str) -> Result<[u8; 16], String> {
    let bad = || "Unknown key.".to_string();
    if s.len() != 32 {
        return Err(bad());
    }
    let mut id = [0u8; 16];
    for (i, b) in id.iter_mut().enumerate() {
        *b = u8::from_str_radix(s.get(i * 2..i * 2 + 2).ok_or_else(bad)?, 16).map_err(|_| bad())?;
    }
    Ok(id)
}

/// Trim, refuse empty, overlong or control characters.
fn clean_text(s: &str, max: usize) -> Option<String> {
    let s = s.trim();
    (!s.is_empty() && s.len() <= max && !s.chars().any(char::is_control)).then(|| s.to_string())
}

/// `Key for Ann.handoff`, with characters file systems reject replaced.
fn file_name(prefix: &str, name: &str) -> String {
    let safe: String = name.chars().map(|c| if c.is_control() || "/\\:*?\"<>|".contains(c) { '_' } else { c }).collect();
    format!("{prefix} {}.handoff", safe.trim_matches('.'))
}

fn display_name(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// Sealed and request files from an earlier run hold no plaintext, but the
/// sender keeps nothing: remove them.
fn clear_outbox(dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        if entry.path().extension().is_some_and(|e| e == "handoff") {
            let _ = fs::remove_file(entry.path());
        }
    }
}

#[cfg(test)]
mod tests {
    //! The whole flow between two identities, with a fake clipboard.

    use super::*;
    use crate::os::MasterKey;
    use rand_core::{OsRng, RngCore};
    use std::sync::Mutex as StdMutex;

    const KEY: &[u8] = b"sk-synthetic-handoff-flow-0001";

    // The fake clipboard is process-wide; tests that use it take this lock.
    static SERIAL: StdMutex<()> = StdMutex::new(());
    static ARMED: StdMutex<Option<OnCaptured>> = StdMutex::new(None);
    static OFFERED: StdMutex<Option<(PathBuf, String)>> = StdMutex::new(None);

    fn fake() -> Clipboard {
        Clipboard {
            arm: |_, _, done| {
                *ARMED.lock().unwrap() = Some(done);
                Ok(())
            },
            offer: |file, text| {
                *OFFERED.lock().unwrap() = Some((file.to_path_buf(), text));
                Ok(())
            },
            read: || Ok(Clip::Empty),
        }
    }

    fn copy(bytes: &[u8]) {
        let done = ARMED.lock().unwrap().take().expect("safe copy armed");
        done(Captured::Caught(Secret::new(bytes.to_vec()), None));
    }

    fn offered() -> (PathBuf, String) {
        OFFERED.lock().unwrap().take().expect("something on the clipboard")
    }

    fn temp(tag: &str) -> PathBuf {
        let mut n = [0u8; 8];
        OsRng.fill_bytes(&mut n);
        std::env::temp_dir().join(format!("handoff-flow-{tag}-{}", u64::from_le_bytes(n)))
    }

    struct App {
        handoff: Arc<Handoff>,
        data: PathBuf,
        outbox: PathBuf,
    }

    fn app(name: &str) -> App {
        let data = temp(name);
        os::private_dir(&data).unwrap();
        let outbox = data.join("outbox");
        let store = Store::in_dir(&data, MasterKey::for_tests());
        let settings = Settings { name: name.into(), ..Settings::default() };
        let handoff = Handoff::open(store, settings, outbox.clone(), fake(), Box::new(|| {})).unwrap();
        App { handoff, data, outbox }
    }

    fn files_contain(dir: &Path, needle: &[u8]) -> bool {
        fs::read_dir(dir).unwrap().flatten().any(|e| {
            let p = e.path();
            if p.is_dir() { files_contain(&p, needle) } else { fs::read(&p).unwrap().windows(needle.len()).any(|w| w == needle) }
        })
    }

    #[test]
    fn request_safe_copy_seal_accept_and_once_only() {
        let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
        let ann = app("Ann");
        let ben = app("Ben");

        // Ask puts a request file on the clipboard.
        ann.handoff.ask().unwrap();
        let (request_file, request_text) = offered();
        assert_eq!(request_file.file_name().unwrap(), "Key request from Ann.handoff");
        assert!(request_text.contains("Ann is asking for a key"));
        assert_eq!(ann.handoff.view().pending, 1);
        assert!(ann.handoff.open_path(&request_file).unwrap_err().contains("your own request"));

        // Ben opens it: Safe Copy arms for Ann.
        ben.handoff.open_path(&request_file).unwrap();
        let safe = ben.handoff.view().safe;
        assert_eq!((safe.state, safe.for_name.as_deref()), ("listening", Some("Ann")));

        // the copy is caught; sealed into a file.
        copy(KEY);
        assert_eq!(ben.handoff.view().safe.state, "caught");
        ben.handoff.seal("Dialpad").unwrap();
        let safe = ben.handoff.view().safe;
        assert_eq!((safe.state, safe.file_name.as_deref()), ("sealed", Some("Key for Ann.handoff")));
        ben.handoff.copy_sealed().unwrap();
        let (sealed_file, _) = offered();
        let sealed_copy = temp("sealed-copy.handoff");
        fs::copy(&sealed_file, &sealed_copy).unwrap();
        assert!(!files_contain(&ben.data, KEY), "sender data holds the key");
        ben.handoff.close_safe();
        assert!(!sealed_file.exists(), "sender keeps no sealed file after Done");
        assert_eq!(ben.handoff.view().keys.len(), 0);

        // Ann opens it; the key is kept with the sticky default.
        ann.handoff.open_path(&sealed_copy).unwrap();
        let view = ann.handoff.view();
        let accepted = view.accepted.clone().unwrap();
        assert_eq!((accepted.label.as_str(), accepted.from.as_deref(), accepted.keep), ("Dialpad", Some("Ben"), Keep::Forever));
        assert_eq!(view.pending, 0, "one-time key destroyed");
        assert!(!files_contain(&ann.data, KEY), "vault holds no plaintext");
        assert!(!serde_json::to_string(&view).unwrap().contains("sk-synthetic"), "no value reaches the screens");

        // The keep choice changes the item and becomes the next default.
        ann.handoff.set_keep(&accepted.id, Keep::Days30).unwrap();
        let view = ann.handoff.view();
        assert_eq!(view.keep, Keep::Days30);
        assert!(view.keys[0].expires_at.is_some());

        // The same sealed file never opens again.
        assert_eq!(ann.handoff.open_path(&sealed_copy).unwrap_err(), ALREADY_ACCEPTED);
        let id = parse_id(&accepted.id).unwrap();
        assert_eq!(ann.handoff.lock().vault.secret(id).unwrap().expose(), KEY);

        for a in [ann, ben] {
            let _ = fs::remove_dir_all(a.data);
        }
        let _ = fs::remove_file(sealed_copy);
    }

    #[test]
    fn add_for_yourself_and_timeouts() {
        let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
        let me = app("Juno");

        // Add, copy, choose a keep-time.
        me.handoff.add().unwrap();
        let safe = me.handoff.view().safe;
        assert_eq!((safe.state, safe.for_name), ("listening", None));
        copy(KEY);
        assert!(me.handoff.seal("X").is_err(), "your own key is never sealed");
        assert!(me.handoff.keep_own("  ", Keep::Once).is_err());
        me.handoff.keep_own("Anthropic", Keep::Once).unwrap();
        let view = me.handoff.view();
        assert_eq!(view.safe.state, "off");
        assert_eq!((view.keys[0].label.as_str(), view.keys[0].from.as_deref(), view.keys[0].keep), ("Anthropic", None, Keep::Once));
        assert_eq!(view.keep, Keep::Once);
        assert!(!files_contain(&me.data, KEY));

        // A timeout offers Try again for the same target.
        me.handoff.add().unwrap();
        let done = ARMED.lock().unwrap().take().unwrap();
        done(Captured::TimedOut);
        assert_eq!(me.handoff.view().safe.state, "timeout");
        me.handoff.retry().unwrap();
        assert_eq!(me.handoff.view().safe.state, "listening");

        // A capture that lands after Safe Copy was closed is ignored.
        let late = ARMED.lock().unwrap().take().unwrap();
        me.handoff.close_safe();
        late(Captured::Caught(Secret::new(KEY.to_vec()), None));
        assert_eq!(me.handoff.view().safe.state, "off");

        assert!(me.outbox.exists());
        let _ = fs::remove_dir_all(me.data);
    }

    #[test]
    fn sealed_for_someone_else_and_garbage_are_refused() {
        let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
        let (a, b, c) = (app("Ann"), app("Ben"), app("Cat"));
        a.handoff.ask().unwrap();
        let (request, _) = offered();
        b.handoff.open_path(&request).unwrap();
        copy(KEY);
        b.handoff.seal("Key").unwrap();
        b.handoff.copy_sealed().unwrap();
        let (sealed, _) = offered();
        assert!(c.handoff.open_path(&sealed).is_err(), "Cat cannot open Ann's key");
        assert!(c.handoff.open_text("hello there").is_err());
        assert_eq!(a.handoff.view().pending, 1, "Ann's one-time key untouched");
        for x in [a, b, c] {
            let _ = fs::remove_dir_all(x.data);
        }
    }

    #[test]
    fn helpers() {
        assert_eq!(file_name("Key for", "a/b:c."), "Key for a_b_c.handoff");
        assert_eq!(parse_id(&hex(&[7; 16])).unwrap(), [7; 16]);
        assert!(parse_id("zz").is_err());
        assert_eq!(clean_text("  Dialpad ", 64).as_deref(), Some("Dialpad"));
        assert!(clean_text("a\nb", 64).is_none());
    }
}
