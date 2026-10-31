//! Every KeyHandoff flow, without Tauri: your card and contacts, Safe Copy,
//! seal, accept and vault, add for yourself, and giving kept keys to the
//! programs you allow. A request is the one-off form of a card: one lock.
//!
//! Secret values live here and in `os`; [`View`] is what the screens get:
//! names, states and ids only.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex, MutexGuard, Weak};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use keyhandoff_seal::{
    armor, Card, Identity, Keyring, Object, Payload, PublicIdentity, Request, Secret, DEFAULT_CARD_TTL,
    MAX_LABEL_BYTES, MAX_NAME_BYTES,
};
use serde::{Deserialize, Serialize};

use crate::os::{self, Captured, Clip, Peer};
use crate::store::{Approval, Contact, Settings, Store};
use crate::vault::{Item, Keep, Vault};

/// Safe Copy disarms itself after this long.
pub const ARM_TIMEOUT: Duration = Duration::from_secs(60);
const MAX_FILE_BYTES: u64 = 64 * 1024;
const ALREADY_ACCEPTED: &str = "This key was already accepted and can't be opened.";
/// How many keys someone can send you with one copy of your card.
pub const CARD_LOCKS: usize = 12;
/// A program waiting for Allow is refused after this long.
pub const PROGRAM_TIMEOUT: Duration = Duration::from_secs(60);

/// Called once when a Safe Copy capture ends.
pub type OnCaptured = Box<dyn FnOnce(Captured) + Send>;
/// Called when something reads the clipboard bait while Safe Copy listens.
pub type OnRead = Box<dyn Fn() + Send>;

/// The clipboard operations the flows need; tests swap in fakes.
pub struct Clipboard {
    pub arm: fn(Duration, Arc<AtomicBool>, OnCaptured, OnRead) -> Result<(), String>,
    pub offer: fn(&Path, String) -> Result<(), String>,
    pub read: fn() -> Result<Clip, String>,
    /// Known clipboard apps running now, named when Safe Copy arms.
    pub apps: fn() -> Vec<String>,
    /// Plain text that isn't secret, such as a pointer.
    pub text: fn(String) -> Result<(), String>,
}

impl Clipboard {
    pub fn system() -> Self {
        Self { arm: |t, c, done, read| os::arm_capture(t, c, done, read), offer: os::offer_file, read: os::read_clipboard, apps: os::clipboard_apps, text: os::offer_text }
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
    /// `apps`: known clipboard apps running when it armed. `read`: something
    /// read the bait, set from the capture thread.
    Listening { target: Target, ends_at: u64, cancel: Arc<AtomicBool>, apps: Vec<String>, read: Arc<AtomicBool> },
    TimedOut { target: Target },
    Failed { target: Target, message: String },
    /// Caught and wiped from the clipboard; waiting for a name (and, for
    /// yourself, a keep-time).
    Caught { target: Target, secret: Secret, note: Option<String> },
    Sealed { for_name: String, file: PathBuf, text: String },
}

/// The answer to a program asking for kept keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Answer {
    /// Whenever this program runs in this folder.
    Always,
    Once,
    Deny,
}

/// A program waiting for Allow.
struct ProgramAsk {
    id: u64,
    peer: Peer,
    keys: Vec<[u8; 16]>,
    reply: mpsc::Sender<Answer>,
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
    /// The file you last shared (your card, or a one-off request).
    asked: Option<PathBuf>,
    notice: Option<String>,
    /// Something went wrong outside a command, such as opening a file.
    problem: Option<String>,
    /// People whose cards you opened.
    contacts: Vec<Contact>,
    /// The contact whose card was just opened, for the sheet that says so.
    added: Option<[u8; 32]>,
    /// Programs allowed to get a key when they run in a folder.
    approvals: Vec<Approval>,
    program: Option<ProgramAsk>,
    asks: u64,
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
    /// Known clipboard apps that were running when Safe Copy armed.
    pub apps: Vec<String>,
    /// Something read the clipboard bait while Safe Copy listened.
    pub read: bool,
}

/// A program asking for kept keys, waiting for Allow.
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct ProgramView {
    /// `python3`, `node.exe`.
    pub name: String,
    pub path: String,
    /// `python3 report.py`, to recognise the script; `None` when the OS won't say.
    pub command: Option<String>,
    /// Where it runs, with the home folder as `~`; `None` when the OS won't say.
    pub folder: Option<String>,
    /// The names of the keys it wants.
    pub keys: Vec<String>,
}

/// Someone you can send keys to.
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct ContactView {
    pub id: String,
    pub name: String,
    /// Their computer's fingerprint, to check with them.
    pub computer: String,
    /// Keys you can still send with the card you have.
    pub sends_left: usize,
}

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct View {
    pub name: String,
    /// This computer's fingerprint, which people you ask will see.
    pub computer: String,
    pub protection: String,
    pub keep: Keep,
    pub keys: Vec<KeyView>,
    pub pending: usize,
    pub safe: SafeView,
    pub accepted: Option<KeyView>,
    pub asked: Option<String>,
    pub notice: Option<String>,
    pub problem: Option<String>,
    pub program: Option<ProgramView>,
    pub contacts: Vec<ContactView>,
    /// Whose card was just opened.
    pub added: Option<ContactView>,
}

impl Handoff {
    pub fn open(store: Store, settings: Settings, outbox: PathBuf, clipboard: Clipboard, notify: Box<dyn Fn() + Send + Sync>) -> Result<Arc<Self>, String> {
        let identity = store.identity()?;
        let keyring = store.keyring()?;
        let vault = store.vault()?;
        let approvals = store.approvals()?;
        let contacts = store.contacts()?;
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
            problem: None,
            contacts,
            added: None,
            approvals,
            program: None,
            asks: 0,
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

    /// A one-off request: one lock instead of a card's batch. The app shares
    /// cards now; this stays for the tests of the request-and-seal core.
    #[cfg(test)]
    pub fn ask(&self) -> Result<(), String> {
        let mut core = self.lock();
        let (request, pending) = keyhandoff_seal::create_request(&core.identity, &core.settings.name, now(), keyhandoff_seal::DEFAULT_REQUEST_TTL)
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

    /// Your card: a batch of one-time locks, each good for one key sent to you.
    /// Writes `KeyHandoff card - <you>.handoff` and puts it on the clipboard.
    pub fn copy_card(&self) -> Result<(), String> {
        let mut core = self.lock();
        let (card, pending) = keyhandoff_seal::create_card(&core.identity, &core.settings.name, now(), DEFAULT_CARD_TTL, CARD_LOCKS)
            .map_err(|e| e.to_string())?;
        pending.into_iter().for_each(|p| core.keyring.insert(p));
        core.store.save_keyring(&core.keyring)?;
        let text = armor::card_text(&card);
        let file = core.outbox.join(file_name("KeyHandoff card -", card.name()));
        os::write_private(&file, text.as_bytes()).map_err(|e| e.to_string())?;
        (self.clipboard.offer)(&file, text)?;
        core.asked = Some(file);
        core.accepted = None;
        Ok(())
    }

    /// Send a key to a contact: Safe Copy against one of their card's locks.
    /// The lock is used up only once the key is sealed.
    pub fn send_to(&self, id: &str) -> Result<(), String> {
        let request = {
            let mut core = self.lock();
            let t = now();
            let contact = core.contacts.iter_mut().find(|c| hex32(&c.identity) == id).ok_or("That contact is no longer here.")?;
            contact.locks.retain(|l| Request::parse(l).is_ok_and(|r| !r.is_expired(t)));
            let name = contact.name.clone();
            let lock = contact.locks.first().cloned();
            core.store.save_contacts(&core.contacts)?;
            let lock = lock.ok_or_else(|| format!("You've used up {name}'s card. Ask {name} for a new one."))?;
            Request::parse(&lock).map_err(|e| e.to_string())?
        };
        self.arm(Target::Request(request))
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
            Object::Card(card) => self.lock().add_contact(&card),
        }
    }

    fn arm(&self, target: Target) -> Result<(), String> {
        let mut core = self.lock();
        core.close_safe();
        core.generation += 1;
        let generation = core.generation;
        let cancel = Arc::new(AtomicBool::new(false));
        // Created first: a read can land before this function returns.
        let read = Arc::new(AtomicBool::new(false));
        let me = self.me.clone();
        let (tripped, me_read) = (read.clone(), self.me.clone());
        (self.clipboard.arm)(
            ARM_TIMEOUT,
            cancel.clone(),
            Box::new(move |result| {
                if let Some(handoff) = me.upgrade() {
                    handoff.captured(generation, result);
                }
            }),
            Box::new(move || {
                if !tripped.swap(true, Ordering::SeqCst) {
                    if let Some(handoff) = me_read.upgrade() {
                        (handoff.notify)();
                    }
                }
            }),
        )?;
        let apps = (self.clipboard.apps)();
        core.safe = Safe::Listening { target, ends_at: now_ms() + ARM_TIMEOUT.as_millis() as u64, cancel, apps, read };
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
        let used = request.id();
        core.use_lock(used)?;
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
            core.prune_approvals()?;
        }
        if core.accepted == Some(id) {
            core.accepted = None;
        }
        Ok(())
    }

    /// Put a ready `.env` line for a key on the clipboard, such as
    /// `DIALPAD_API_KEY=handoff:Dialpad`. It names the key without holding it,
    /// so it's fine in `.env` and on the clipboard.
    pub fn copy_pointer(&self, id: &str) -> Result<(), String> {
        let id = parse_id(id)?;
        let label = {
            let core = self.lock();
            core.vault.items().iter().find(|i| i.id == id).map(|i| i.label.clone()).ok_or("That key is no longer here.")?
        };
        let line = env_line(&label);
        (self.clipboard.text)(line.clone())?;
        self.lock().notice = Some(format!("Copied {line}"));
        Ok(())
    }

    pub fn set_name(&self, name: &str) -> Result<(), String> {
        let name = clean_text(name, MAX_NAME_BYTES).ok_or("Use a short name without special characters.")?;
        let mut core = self.lock();
        core.settings.name = name;
        core.store.save_settings(&core.settings)
    }

    /// Dismiss the accept, card and contact sheets and any notice.
    pub fn dismiss(&self) {
        let mut core = self.lock();
        core.accepted = None;
        core.asked = None;
        core.added = None;
        core.notice = None;
        core.problem = None;
    }

    /// Report a problem the screens didn't ask about, such as a file that
    /// couldn't be opened.
    pub fn problem(&self, text: String) {
        self.lock().problem = Some(text);
    }

    /// A program asks for kept keys by name (`handoff:Name` pointers). Keys
    /// it was allowed in this folder come back at once; otherwise the window
    /// asks, and this waits for the answer. Approval is per program, folder
    /// and key, because allowing `python3` alone would allow every script.
    pub fn give(&self, peer: &Peer, names: &[String]) -> Result<Vec<Secret>, String> {
        let (ids, new, id, answer) = {
            let mut core = self.lock();
            let _ = core.tick();
            let ids = names
                .iter()
                .map(|n| core.vault.find(n).ok_or_else(|| format!("No kept key is named {n}.")))
                .collect::<Result<Vec<_>, _>>()?;
            let mut new: Vec<_> = ids.iter().copied().filter(|&k| !core.allowed(peer, k)).collect();
            new.dedup();
            if new.is_empty() {
                return core.release(&ids);
            }
            if core.program.is_some() {
                return Err("KeyHandoff is asking about another program. Try again in a moment.".into());
            }
            let (reply, answer) = mpsc::channel();
            core.asks += 1;
            let id = core.asks;
            core.program = Some(ProgramAsk { id, peer: peer.clone(), keys: new.clone(), reply });
            (ids, new, id, answer)
        };
        (self.notify)();
        let answer = answer.recv_timeout(PROGRAM_TIMEOUT).unwrap_or(Answer::Deny);
        let result = {
            let mut core = self.lock();
            if core.program.as_ref().is_some_and(|a| a.id == id) {
                core.program = None;
            }
            match answer {
                Answer::Deny => Err("Not allowed in KeyHandoff.".into()),
                Answer::Once => core.release(&ids),
                Answer::Always => core.allow(peer, &new).and_then(|()| core.release(&ids)),
            }
        };
        (self.notify)();
        result
    }

    /// Answer the program waiting for Allow.
    pub fn answer(&self, answer: Answer) -> Result<(), String> {
        let ask = self.lock().program.take().ok_or("No program is waiting.")?;
        let _ = ask.reply.send(answer);
        Ok(())
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

    /// Open someone's card: they become a contact (replacing an older card of
    /// theirs) with its unexpired locks.
    fn add_contact(&mut self, card: &Card) -> Result<(), String> {
        if card.holder() == self.identity.public() {
            return Err("This is your own card. Send it to people who'll send you keys.".into());
        }
        let t = now();
        let locks: Vec<Vec<u8>> = card.locks().iter().filter(|l| !l.is_expired(t)).map(|l| l.to_bytes().to_vec()).collect();
        if locks.is_empty() {
            return Err("This card ran out. Ask them for a new one.".into());
        }
        let identity = *card.holder().as_bytes();
        self.contacts.retain(|c| c.identity != identity);
        self.contacts.push(Contact { name: card.name().to_string(), identity, locks });
        self.store.save_contacts(&self.contacts)?;
        self.added = Some(identity);
        self.accepted = None;
        self.asked = None;
        Ok(())
    }

    /// A key was sealed to this lock: if it came from a contact's card, it's spent.
    fn use_lock(&mut self, used: [u8; 16]) -> Result<(), String> {
        let before: usize = self.contacts.iter().map(|c| c.locks.len()).sum();
        for c in &mut self.contacts {
            c.locks.retain(|l| Request::parse(l).map_or(true, |r| r.id() != used));
        }
        if self.contacts.iter().map(|c| c.locks.len()).sum::<usize>() != before {
            self.store.save_contacts(&self.contacts)?;
        }
        Ok(())
    }

    fn contact_view(&self, c: &Contact) -> ContactView {
        let computer = PublicIdentity::from_bytes(c.identity).map(|p| p.fingerprint()).unwrap_or_default();
        ContactView { id: hex32(&c.identity), name: c.name.clone(), computer, sends_left: c.locks.len() }
    }

    fn allowed(&self, peer: &Peer, key: [u8; 16]) -> bool {
        self.approvals.iter().any(|a| a.key == key && a.program == peer.program && a.folder == peer.folder)
    }

    fn allow(&mut self, peer: &Peer, keys: &[[u8; 16]]) -> Result<(), String> {
        for &key in keys {
            self.approvals.push(Approval { program: peer.program.clone(), folder: peer.folder.clone(), key });
        }
        self.prune_approvals()
    }

    /// Drop approvals for keys that are gone, and save.
    fn prune_approvals(&mut self) -> Result<(), String> {
        let vault = &self.vault;
        self.approvals.retain(|a| vault.items().iter().any(|i| i.id == a.key));
        self.store.save_approvals(&self.approvals)
    }

    /// The values for a program. Keys kept Once are then destroyed.
    fn release(&mut self, ids: &[[u8; 16]]) -> Result<Vec<Secret>, String> {
        let secrets = ids
            .iter()
            .map(|&id| self.vault.secret(id).ok_or_else(|| "That key is no longer here.".to_string()))
            .collect::<Result<Vec<_>, _>>()?;
        let once: Vec<_> = self.vault.items().iter().filter(|i| i.keep == Keep::Once && ids.contains(&i.id)).map(|i| i.id).collect();
        if !once.is_empty() {
            for id in once {
                self.vault.remove(id);
            }
            self.store.save_vault(&self.vault)?;
            self.prune_approvals()?;
        }
        Ok(secrets)
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
            computer: self.identity.public().fingerprint(),
            protection: self.settings.protection.clone().unwrap_or_default(),
            keep: self.settings.keep,
            keys,
            pending: self.keyring.len(),
            safe: self.safe_view(),
            accepted,
            asked: self.asked.as_ref().map(|p| display_name(p)),
            notice: self.notice.clone(),
            problem: self.problem.clone(),
            program: self.program.as_ref().map(|a| self.program_view(a)),
            contacts: self.contacts.iter().map(|c| self.contact_view(c)).collect(),
            added: self.added.and_then(|id| self.contacts.iter().find(|c| c.identity == id)).map(|c| self.contact_view(c)),
        }
    }

    fn program_view(&self, ask: &ProgramAsk) -> ProgramView {
        let labels = ask.keys.iter().filter_map(|id| self.vault.items().iter().find(|i| i.id == *id)).map(|i| i.label.clone());
        ProgramView {
            name: display_name(&ask.peer.program),
            path: ask.peer.program.to_string_lossy().into_owned(),
            command: ask.peer.command.clone(),
            folder: ask.peer.folder.as_deref().map(home_relative),
            keys: labels.collect(),
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
            apps: Vec::new(),
            read: false,
        };
        match &self.safe {
            Safe::Off => {
                SafeView { state: "off", for_name: None, for_computer: None, ends_at: 0, file_name: None, message: None, apps: Vec::new(), read: false }
            }
            Safe::Listening { target, ends_at, apps, read, .. } => {
                SafeView { ends_at: *ends_at, apps: apps.clone(), read: read.load(Ordering::SeqCst), ..base("listening", target) }
            }
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
                apps: Vec::new(),
                read: false,
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

fn hex32(id: &[u8; 32]) -> String {
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

/// A `.env` line pointing at a kept key: `Dialpad` gives
/// `DIALPAD_API_KEY=handoff:Dialpad`. The value is quoted when the name has
/// spaces or other characters a `.env` reader might split on.
fn env_line(label: &str) -> String {
    let words: Vec<String> =
        label.split(|c: char| !c.is_ascii_alphanumeric()).filter(|w| !w.is_empty()).map(str::to_ascii_uppercase).collect();
    let mut var = words.join("_");
    if !["KEY", "TOKEN", "SECRET", "PASSWORD"].iter().any(|end| var.ends_with(end)) {
        var = if var.is_empty() { "API_KEY".into() } else { format!("{var}_API_KEY") };
    }
    if var.starts_with(|c: char| c.is_ascii_digit()) {
        var = format!("KEY_{var}");
    }
    let pointer = format!("handoff:{label}");
    if pointer.chars().all(|c| c.is_ascii_alphanumeric() || "_-.:".contains(c)) {
        format!("{var}={pointer}")
    } else {
        format!("{var}=\"{}\"", pointer.replace('"', ""))
    }
}

/// `/home/ann/proj` as `~/proj`.
fn home_relative(path: &Path) -> String {
    match dirs::home_dir().and_then(|home| path.strip_prefix(home).ok().map(Path::to_path_buf)) {
        Some(rest) if rest.as_os_str().is_empty() => "~".into(),
        Some(rest) => format!("~{}{}", std::path::MAIN_SEPARATOR, rest.display()),
        None => path.display().to_string(),
    }
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
    static BAIT_READ: StdMutex<Option<OnRead>> = StdMutex::new(None);
    static OFFERED: StdMutex<Option<(PathBuf, String)>> = StdMutex::new(None);

    fn fake() -> Clipboard {
        Clipboard {
            arm: |_, _, done, read| {
                *ARMED.lock().unwrap() = Some(done);
                *BAIT_READ.lock().unwrap() = Some(read);
                Ok(())
            },
            offer: |file, text| {
                *OFFERED.lock().unwrap() = Some((file.to_path_buf(), text));
                Ok(())
            },
            read: || Ok(Clip::Empty),
            apps: || vec!["CopyQ".into()],
            text: |text| {
                *OFFERED.lock().unwrap() = Some((PathBuf::new(), text));
                Ok(())
            },
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
        assert_eq!(safe.for_computer, Some(ann.handoff.view().computer), "Ann can read Ben the fingerprint he sees");

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
    fn a_card_lets_ben_send_ann_keys_each_opening_once() {
        let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
        let ann = app("Ann");
        let ben = app("Ben");

        // Ann copies her card once; it carries a batch of locks.
        ann.handoff.copy_card().unwrap();
        let (card_file, card_text) = offered();
        assert_eq!(card_file.file_name().unwrap(), "KeyHandoff card - Ann.handoff");
        assert!(card_text.contains("Ann's KeyHandoff card"));
        assert!(ann.handoff.open_path(&card_file).unwrap_err().contains("your own card"));

        // Ben opens it: Ann is a contact, with Ann's fingerprint to check.
        ben.handoff.open_path(&card_file).unwrap();
        let view = ben.handoff.view();
        let contact = view.added.clone().unwrap();
        assert_eq!((contact.name.as_str(), contact.sends_left), ("Ann", CARD_LOCKS));
        assert_eq!(contact.computer, ann.handoff.view().computer);

        // Send a key: Safe Copy for Ann. Cancelling doesn't spend a lock.
        ben.handoff.send_to(&contact.id).unwrap();
        assert_eq!(ben.handoff.view().safe.for_name.as_deref(), Some("Ann"));
        ben.handoff.close_safe();
        assert_eq!(ben.handoff.view().contacts[0].sends_left, CARD_LOCKS);

        // Two keys, two locks; each sealed file opens once on Ann's computer.
        let mut sealed_files = Vec::new();
        for (i, key) in [b"sk-synthetic-card-0001".as_slice(), b"sk-synthetic-card-0002"].into_iter().enumerate() {
            ben.handoff.send_to(&contact.id).unwrap();
            copy(key);
            ben.handoff.seal(&format!("Key {i}")).unwrap();
            ben.handoff.copy_sealed().unwrap();
            let (file, _) = offered();
            let kept = temp(&format!("card-sealed-{i}.handoff"));
            fs::copy(&file, &kept).unwrap();
            ben.handoff.close_safe();
            sealed_files.push(kept);
        }
        assert_eq!(ben.handoff.view().contacts[0].sends_left, CARD_LOCKS - 2, "a lock is spent per key sealed");
        assert!(!files_contain(&ben.data, b"sk-synthetic-card"), "Ben keeps nothing");

        for (i, file) in sealed_files.iter().enumerate() {
            ann.handoff.open_path(file).unwrap();
            assert_eq!(ann.handoff.view().accepted.unwrap().label, format!("Key {i}"));
            assert_eq!(ann.handoff.open_path(file).unwrap_err(), ALREADY_ACCEPTED, "opens once");
        }
        assert!(!files_contain(&ann.data, b"sk-synthetic-card"), "Ann's vault holds no plaintext");

        for a in [ann, ben] {
            let _ = fs::remove_dir_all(a.data);
        }
        sealed_files.iter().for_each(|f| drop(fs::remove_file(f)));
    }

    #[test]
    fn add_for_yourself_and_timeouts() {
        let _serial = SERIAL.lock().unwrap_or_else(|p| p.into_inner());
        let me = app("Juno");

        // Add, copy, choose a keep-time.
        me.handoff.add().unwrap();
        let safe = me.handoff.view().safe;
        assert_eq!((safe.state, safe.for_name), ("listening", None));
        assert_eq!(safe.apps, ["CopyQ"], "running clipboard apps are named while listening");
        assert!(!safe.read);
        (BAIT_READ.lock().unwrap().as_ref().unwrap())();
        assert!(me.handoff.view().safe.read, "a read of the bait shows while listening");
        copy(KEY);
        assert!(me.handoff.seal("X").is_err(), "your own key is never sealed");
        assert!(me.handoff.keep_own("  ", Keep::Once).is_err());
        me.handoff.keep_own("Anthropic", Keep::Once).unwrap();
        let view = me.handoff.view();
        assert_eq!(view.safe.state, "off");
        assert_eq!((view.keys[0].label.as_str(), view.keys[0].from.as_deref(), view.keys[0].keep), ("Anthropic", None, Keep::Once));
        assert_eq!(view.keep, Keep::Once);
        assert!(!files_contain(&me.data, KEY));
        me.handoff.copy_pointer(&view.keys[0].id).unwrap();
        assert_eq!(offered().1, "ANTHROPIC_API_KEY=handoff:Anthropic", "a .env line, never the key, goes on the clipboard");

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

    fn peer(program: &str, folder: &str) -> Peer {
        Peer { program: program.into(), folder: Some(folder.into()), command: Some(format!("{program} report.py")) }
    }

    /// Wait for a program to be asking, then answer it.
    fn answer_when_asked(h: &Handoff, answer: Answer) -> ProgramView {
        for _ in 0..500 {
            if let Some(program) = h.view().program {
                h.answer(answer).unwrap();
                return program;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("no program asked");
    }

    #[test]
    fn programs_get_only_the_keys_they_were_allowed_in_that_folder() {
        let me = app("Juno");
        let h = me.handoff.clone();
        {
            let mut core = h.lock();
            core.vault.add("Dialpad", None, None, &Secret::new(KEY.to_vec()), Keep::Forever, now());
            core.vault.add("Temp", None, None, &Secret::new(b"sk-synthetic-once".to_vec()), Keep::Once, now());
        }
        let script = peer("/usr/bin/python3", "/home/juno/report");
        let ask = |p: Peer, names: &[&str]| {
            let (h, names) = (h.clone(), names.iter().map(|n| n.to_string()).collect::<Vec<_>>());
            std::thread::spawn(move || h.give(&p, &names).map(|s| s.iter().map(|s| s.expose().to_vec()).collect::<Vec<_>>()))
        };

        // First use asks; Always allow gives the key, and the next run is silent.
        let first = ask(script.clone(), &["Dialpad"]);
        let shown = answer_when_asked(&h, Answer::Always);
        assert_eq!((shown.name.as_str(), shown.keys.as_slice()), ("python3", ["Dialpad".to_string()].as_slice()));
        assert_eq!(first.join().unwrap().unwrap(), [KEY.to_vec()]);
        assert_eq!(h.give(&script, &["dialpad".into()]).unwrap()[0].expose(), KEY, "allowed: no prompt");

        // The same program in another folder, or another program, has to ask.
        let elsewhere = ask(peer("/usr/bin/python3", "/tmp/evil"), &["Dialpad"]);
        assert_eq!(answer_when_asked(&h, Answer::Deny).folder.as_deref(), Some("/tmp/evil"));
        assert!(elsewhere.join().unwrap().is_err());
        let other = ask(peer("/usr/bin/node", "/home/juno/report"), &["Dialpad"]);
        answer_when_asked(&h, Answer::Deny);
        assert!(other.join().unwrap().is_err());

        // Allowing one key doesn't allow another; a key kept Once goes after use.
        let once = ask(script.clone(), &["Dialpad", "Temp"]);
        assert_eq!(answer_when_asked(&h, Answer::Once).keys, ["Temp"]);
        assert_eq!(once.join().unwrap().unwrap()[1], b"sk-synthetic-once");
        assert!(h.give(&script, &["Temp".into()]).unwrap_err().contains("No kept key is named Temp"));
        assert!(h.view().keys.iter().all(|k| k.label != "Temp"));

        assert!(!files_contain(&me.data, KEY), "approvals and vault hold no plaintext");
        let _ = fs::remove_dir_all(me.data);
    }

    /// The pointer check, for real, for each loader: a script whose .env holds
    /// only a pointer, with the loader copied next to it, gets the key over the
    /// socket after Always allow; another process asking over the same socket
    /// gets nothing and a prompt.
    #[cfg(target_os = "linux")]
    fn loader_gets_the_key_through_the_socket(program: &str, loader: &str, script: (&str, &str)) {
        let script_name = script.0;
        use sha2::{Digest, Sha256};
        use std::io::{BufRead, BufReader, Write};
        use std::process::Command;

        if Command::new(program).arg("--version").output().is_err() {
            eprintln!("skipped: no {program}");
            return;
        }
        let me = app("Juno");
        let h = me.handoff.clone();
        h.lock().vault.add("Dialpad live", None, None, &Secret::new(KEY.to_vec()), Keep::Forever, now());
        let profile = format!("test-{}", OsRng.next_u32());
        let served = h.clone();
        os::listen_for_keys(&profile, Arc::new(move |peer, stream| crate::pointer::serve(&served, &peer, stream))).unwrap();

        let project = me.data.join("project");
        fs::create_dir_all(&project).unwrap();
        // The project's .env holds exactly the line Copy pointer gives.
        let line = env_line("Dialpad live");
        assert_eq!(line, "DIALPAD_LIVE_API_KEY=\"handoff:Dialpad live\"");
        fs::write(project.join(".env"), format!("{line}\nOTHER=plain\n")).unwrap();
        fs::copy(Path::new(env!("CARGO_MANIFEST_DIR")).join("../loaders").join(loader), project.join(loader)).unwrap();
        fs::write(project.join(script.0), script.1).unwrap();
        let run = || {
            let out = Command::new(program)
                .arg(script.0)
                .current_dir(&project)
                .env("KEYHANDOFF_PROFILE", &profile)
                .env_remove("DIALPAD_LIVE_API_KEY")
                .output()
                .unwrap();
            assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
            String::from_utf8(out.stdout).unwrap().trim().to_string()
        };
        let expected: String = Sha256::digest(KEY).iter().map(|b| format!("{b:02x}")).collect();

        let first = std::thread::scope(|s| {
            let script = s.spawn(run);
            let shown = answer_when_asked(&h, Answer::Always);
            assert!(shown.name.starts_with(program.trim_end_matches('3')), "{shown:?}");
            assert_eq!(shown.command, Some(format!("{program} {script_name}")), "the prompt names the script");
            assert_eq!(shown.folder, Some(home_relative(&project.canonicalize().unwrap())));
            script.join().unwrap()
        });
        assert_eq!(first, expected);
        assert_eq!(run(), expected, "allowed: the second run needs no prompt");

        // This test binary is a different program: it gets a prompt and, denied, nothing.
        let sock = os::runtime_dir().join(format!("handoff-{profile}-keys.sock"));
        let reply = std::thread::scope(|s| {
            let asking = s.spawn(|| {
                let mut conn = std::os::unix::net::UnixStream::connect(&sock).unwrap();
                conn.write_all(b"{\"names\":[\"Dialpad live\"]}\n").unwrap();
                let mut line = String::new();
                BufReader::new(conn).read_line(&mut line).unwrap();
                line
            });
            answer_when_asked(&h, Answer::Deny);
            asking.join().unwrap()
        });
        assert!(reply.contains("\"error\"") && !reply.contains("sk-synthetic"), "{reply}");

        assert!(!files_contain(&project, KEY), "the project holds no key");
        let _ = fs::remove_file(sock);
        let _ = fs::remove_dir_all(me.data);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn python_loader_gets_the_key_through_the_socket() {
        let script = "import hashlib, os, keyhandoff_env\nkeyhandoff_env.load()\nprint(hashlib.sha256(os.environ['DIALPAD_LIVE_API_KEY'].encode()).hexdigest())\n";
        loader_gets_the_key_through_the_socket("python3", "keyhandoff_env.py", ("check.py", script));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn node_loader_gets_the_key_through_the_socket() {
        let script = "import { createHash } from \"node:crypto\";\nimport { load } from \"./keyhandoff_env.mjs\";\nawait load();\nconsole.log(createHash(\"sha256\").update(process.env.DIALPAD_LIVE_API_KEY).digest(\"hex\"));\n";
        loader_gets_the_key_through_the_socket("node", "keyhandoff_env.mjs", ("check.mjs", script));
    }

    #[test]
    fn helpers() {
        assert_eq!(file_name("Key for", "a/b:c."), "Key for a_b_c.handoff");
        assert_eq!(parse_id(&hex(&[7; 16])).unwrap(), [7; 16]);
        assert!(parse_id("zz").is_err());
        assert_eq!(clean_text("  Dialpad ", 64).as_deref(), Some("Dialpad"));
        assert_eq!(env_line("Dialpad"), "DIALPAD_API_KEY=handoff:Dialpad");
        assert_eq!(env_line("GitHub token"), "GITHUB_TOKEN=\"handoff:GitHub token\"");
        assert_eq!(env_line("API key"), "API_KEY=\"handoff:API key\"");
        assert_eq!(env_line("1Password"), "KEY_1PASSWORD=handoff:1Password");
        assert!(clean_text("a\nb", 64).is_none());
    }
}
