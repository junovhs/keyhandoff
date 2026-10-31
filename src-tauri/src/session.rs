//! Locked or open. The profile starts locked; the passphrase (or recovery
//! code) opens it and starts the `Handoff` core. Locking drops the core,
//! which zeroizes the master key, the identity and the loaded vault, and goes
//! back to the locked profile. It locks after a quiet spell, when the
//! computer sleeps or the screen locks, and from the Lock button.
//!
//! Programs asking for keys while locked wait here for an unlock, so a script
//! started before you unlock still gets its keys once you do.

use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::Duration;

use zeroize::Zeroizing;

use crate::core::Handoff;
use crate::store::{Locked, Settings, Store};

enum State {
    /// Its own lock, so a slow unlock doesn't hold up everything else.
    Locked(Arc<Mutex<Locked>>),
    Open(Arc<Handoff>),
}

/// Starts the core once a profile opens.
pub type Start = Box<dyn Fn(Store, Settings) -> Result<Arc<Handoff>, String> + Send + Sync>;

pub struct Session {
    dir: PathBuf,
    profile: String,
    state: Mutex<State>,
    opened: Condvar,
    /// When you last did something in the window, Unix ms.
    last: Mutex<u64>,
    /// Lock after this long with nothing done in the window; off unless set.
    idle: Mutex<Option<Duration>>,
    /// "Remember on this computer": it opens without asking, so locking on
    /// sleep or screen lock would protect nothing.
    remembered: Mutex<bool>,
    /// Files opened before unlock, opened right after it.
    pub files: Mutex<Vec<PathBuf>>,
    /// The recovery code from setup, shown until you say it's written down.
    pub recovery: Mutex<Option<Zeroizing<String>>>,
    start: Start,
}

/// How a locked profile opens: with the passphrase, or the recovery code.
pub type How<'a> = Box<dyn FnOnce(&mut Locked, &Session) -> Result<(Store, Settings), String> + Send + 'a>;

fn guard<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Session {
    pub fn new(dir: PathBuf, profile: &str, start: Start) -> Result<Self, String> {
        let locked = Locked::open_in(dir.clone(), profile)?;
        Ok(Self {
            dir,
            profile: profile.to_string(),
            state: Mutex::new(State::Locked(Arc::new(Mutex::new(locked)))),
            opened: Condvar::new(),
            last: Mutex::new(0),
            idle: Mutex::new(None),
            remembered: Mutex::new(false),
            files: Mutex::new(Vec::new()),
            recovery: Mutex::new(None),
            start,
        })
    }

    /// setup (no passphrase yet), locked or open, and whether an older
    /// profile's key moves behind the passphrase.
    pub fn state(&self) -> (&'static str, bool) {
        let locked = match &*guard(&self.state) {
            State::Locked(l) => l.clone(),
            State::Open(_) => return ("open", false),
        };
        // An unlock in progress holds the profile; it was locked, not new.
        let Ok(l) = locked.try_lock() else { return ("locked", false) };
        if l.needs_setup() {
            ("setup", l.migrating())
        } else {
            ("locked", false)
        }
    }

    /// Open the profile with `how` and start the core.
    /// Argon2id takes about a second, and starting the core may report a
    /// change that reads this session, so neither runs under `state`.
    pub fn open(&self, how: How, now: u64) -> Result<(), String> {
        let locked = match &*guard(&self.state) {
            State::Locked(l) => l.clone(),
            State::Open(_) => return Err("KeyHandoff is already unlocked.".into()),
        };
        let mut locked = guard(&locked);
        if matches!(&*guard(&self.state), State::Open(_)) {
            return Err("KeyHandoff is already unlocked.".into());
        }
        let (store, settings) = how(&mut locked, self)?;
        *guard(&self.idle) = settings.lock_after_minutes.map(|m| Duration::from_secs(60 * m as u64));
        *guard(&self.remembered) = settings.remembered.is_some();
        let handoff = (self.start)(store, settings)?;
        *guard(&self.state) = State::Open(handoff);
        *guard(&self.last) = now;
        self.opened.notify_all();
        Ok(())
    }

    /// Remembered profiles open at launch without asking.
    pub fn open_if_remembered(&self, now: u64) -> Result<bool, String> {
        let remembered = match &*guard(&self.state) {
            State::Locked(l) => guard(l).remembered(),
            State::Open(_) => false,
        };
        if remembered {
            self.open(Box::new(|l, _| l.open_remembered()), now)?;
        }
        Ok(remembered)
    }

    /// Locked with a TPM PIN set up: the lock screen asks for the PIN.
    pub fn has_pin(&self) -> bool {
        match &*guard(&self.state) {
            State::Locked(l) => l.try_lock().is_ok_and(|l| l.has_pin()),
            State::Open(_) => false,
        }
    }

    pub fn remembered(&self) -> bool {
        *guard(&self.remembered)
    }

    /// Turn "Remember on this computer" on or off while open.
    pub fn remember(&self, on: bool) -> Result<(), String> {
        self.current().ok_or("KeyHandoff is locked.")?.remember(on)?;
        *guard(&self.remembered) = on;
        Ok(())
    }

    /// Lock after `minutes` without activity from now on, or never; saved
    /// for next time too.
    pub fn set_lock_after(&self, minutes: Option<u32>, now: u64) -> Result<(), String> {
        self.touch(now)?.set_lock_after(minutes)?;
        *guard(&self.idle) = minutes.map(|m| Duration::from_secs(60 * m as u64));
        Ok(())
    }

    /// The open core, for something you did in the window: it counts as
    /// activity, so the idle lock waits.
    pub fn touch(&self, now: u64) -> Result<Arc<Handoff>, String> {
        let handoff = self.current().ok_or("KeyHandoff is locked.")?;
        *guard(&self.last) = now;
        Ok(handoff)
    }

    /// The open core, if any, without counting as activity.
    pub fn current(&self) -> Option<Arc<Handoff>> {
        match &*guard(&self.state) {
            State::Open(h) => Some(h.clone()),
            State::Locked(_) => None,
        }
    }

    /// The open core, waiting up to `timeout` for an unlock. Programs use
    /// this; their requests aren't activity, so a script can't keep the
    /// vault open while you're away.
    pub fn wait_open(&self, timeout: Duration) -> Option<Arc<Handoff>> {
        let state = guard(&self.state);
        let (state, _) = self
            .opened
            .wait_timeout_while(state, timeout, |s| matches!(s, State::Locked(_)))
            .unwrap_or_else(|e| e.into_inner());
        match &*state {
            State::Open(h) => Some(h.clone()),
            State::Locked(_) => None,
        }
    }

    /// Lock now: stop Safe Copy, refuse a waiting program and drop the core.
    pub fn lock(&self) -> Result<(), String> {
        let locked = Locked::open_in(self.dir.clone(), &self.profile)?;
        let old = std::mem::replace(&mut *guard(&self.state), State::Locked(Arc::new(Mutex::new(locked))));
        if let State::Open(handoff) = old {
            handoff.close_safe();
            let _ = handoff.answer(crate::core::Answer::Deny);
        }
        Ok(())
    }

    /// Lock if nothing was done in the window for the idle time you set,
    /// unless Safe Copy is listening or a program is waiting for an answer.
    /// True if it locked.
    pub fn lock_if_idle(&self, now: u64) -> bool {
        let Some(idle) = *guard(&self.idle) else { return false };
        if self.remembered() {
            return false;
        }
        let Some(handoff) = self.current() else { return false };
        let quiet = now.saturating_sub(*guard(&self.last)) >= idle.as_millis() as u64;
        quiet && !handoff.busy() && self.lock().is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Clipboard;
    use crate::os::{self, Clip};
    use rand_core::{OsRng, RngCore};

    const PASS: &str = "correct horse battery staple";

    fn session() -> (Arc<Session>, PathBuf) {
        let mut n = [0u8; 8];
        OsRng.fill_bytes(&mut n);
        let dir = std::env::temp_dir().join(format!("handoff-test-session-{}", u64::from_le_bytes(n)));
        os::private_dir(&dir).unwrap();
        let outbox = dir.join("out");
        let start: Start = Box::new(move |store, settings| {
            let clipboard = Clipboard {
                arm: |_, _, _, _| Ok(()),
                offer: |_, _| Ok(()),
                read: || Ok(Clip::Empty),
                apps: Vec::new,
                text: |_| Ok(()),
            };
            Handoff::open(store, settings, outbox.clone(), clipboard, Box::new(|| {}))
        });
        let session = Arc::new(Session::new(dir.clone(), "t", start).unwrap());
        session.open(Box::new(|l, _| l.set_up(PASS).map(|(s, st, _)| (s, st))), 0).unwrap();
        session.lock().unwrap();
        (session, dir)
    }

    fn unlock(session: &Session, now: u64) {
        session
            .open(
                Box::new(|l, _| {
                    let (store, mut settings) = l.unlock(PASS)?;
                    settings.lock_after_minutes = Some(1);
                    Ok((store, settings))
                }),
                now,
            )
            .unwrap();
    }

    #[test]
    fn idle_locks_and_drops_the_key_until_unlock() {
        let (session, dir) = session();
        assert!(session.touch(1).is_err(), "locked after setup and lock");
        unlock(&session, 1_000);
        let weak = Arc::downgrade(&session.current().unwrap());

        assert!(!session.lock_if_idle(30_000), "not idle yet");
        session.touch(50_000).unwrap();
        assert!(!session.lock_if_idle(100_000), "activity restarts the clock");
        assert!(session.lock_if_idle(110_000));

        assert!(weak.upgrade().is_none(), "the core, and the master key in it, is gone");
        assert!(session.touch(110_001).is_err());
        assert!(session.wait_open(Duration::from_millis(20)).is_none());
        unlock(&session, 120_000);
        assert!(session.touch(120_001).is_ok());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn lock_after_set_in_the_window_applies_now_and_never_stops_it() {
        let (session, dir) = session();
        session.open(Box::new(|l, _| l.unlock(PASS)), 0).unwrap();
        const MIN: u64 = 60_000;
        assert!(!session.lock_if_idle(24 * 60 * MIN), "off by default");

        session.set_lock_after(Some(15), 24 * 60 * MIN).unwrap();
        let t = 24 * 60 * MIN;
        assert!(!session.lock_if_idle(t + 14 * MIN));
        assert!(session.lock_if_idle(t + 15 * MIN), "locks after 15 quiet minutes");

        session.open(Box::new(|l, _| l.unlock(PASS)), t).unwrap();
        assert!(session.current().is_some());
        assert_eq!(session.current().unwrap().view().lock_after, Some(15), "saved: the next unlock keeps it");
        assert!(session.lock_if_idle(t + 15 * MIN), "and applies it");

        session.open(Box::new(|l, _| l.unlock(PASS)), t).unwrap();
        session.set_lock_after(None, t).unwrap();
        assert!(!session.lock_if_idle(t + 1000 * MIN), "Never stops it");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_program_waiting_while_locked_gets_the_core_after_unlock() {
        let (session, dir) = session();
        let waiter = {
            let session = session.clone();
            std::thread::spawn(move || session.wait_open(Duration::from_secs(10)).is_some())
        };
        std::thread::sleep(Duration::from_millis(50));
        unlock(&session, 1);
        assert!(waiter.join().unwrap(), "the waiting program got through once unlocked");
        std::fs::remove_dir_all(dir).unwrap();
    }
}
