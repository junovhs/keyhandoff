//! Nearby: other KeyHandoffs on the local network. While it's on, this
//! computer announces itself over mDNS (`_keyhandoff._tcp`, with only its
//! fingerprint), finds the others, and listens for sealed keys. One connection
//! carries one object: a big-endian length, the object's bytes, then a reply
//! (0 for taken, or 1 and a reason). What arrives is still a sealed key: the
//! network only moves locked boxes, and the core decides whether to take it.
//!
//! Pairing: while someone is adding people, the announcement also carries
//! their name, and the two sides swap cards over this same channel. Both show
//! [`pairing_code`] of the two cards; a card swapped in transit gives
//! different codes.
//!
//! It stops when dropped, which happens when KeyHandoff locks.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use rand_core::{OsRng, RngCore};
use sha2::{Digest, Sha256};

const SERVICE: &str = "_keyhandoff._tcp.local.";
/// A sealed key or a team's bundle fits easily; anything bigger isn't ours.
const MAX_OBJECT_BYTES: u32 = 256 * 1024;
/// The whole exchange, start to reply, not each read: a sender trickling a
/// byte at a time is cut off here.
const IO_TIMEOUT: Duration = Duration::from_secs(5);
const ACCEPT_POLL: Duration = Duration::from_millis(100);
/// Connections handled at once; more are closed straight away. Someone
/// flooding the network can delay a send, not wear the computer down.
const MAX_CONNECTIONS: usize = 8;
/// Computers remembered from announcements; more are ignored, so fake
/// announcements can't grow the list without end.
const MAX_SEEN: usize = 64;
/// A refusal's reason is a sentence; a longer reply is cut short.
const MAX_REASON_BYTES: usize = 512;

/// What to do with an object that arrived: take it, or say why not.
pub type OnObject = Arc<dyn Fn(Vec<u8>) -> Result<(), String> + Send + Sync>;

/// A computer seen nearby: where it listens, and the name it's pairing
/// under, if it's adding people right now.
#[derive(Clone, Debug)]
pub struct Seen {
    pub at: SocketAddr,
    pub pairing: Option<String>,
}

pub struct Nearby {
    stop: Arc<AtomicBool>,
    /// Connections being handled right now.
    active: Arc<AtomicUsize>,
    fingerprint: String,
    instance: String,
    #[cfg_attr(not(test), allow(dead_code))]
    port: u16,
    /// Fingerprint → that computer, as last announced.
    seen: Arc<Mutex<HashMap<String, Seen>>>,
    mdns: Option<ServiceDaemon>,
}

impl Nearby {
    /// Listen for objects and, with `announce`, announce `fingerprint` and
    /// find others. `changed` is called when the computers nearby change.
    pub fn start(fingerprint: &str, announce: bool, on_object: OnObject, changed: Arc<dyn Fn() + Send + Sync>) -> Result<Self, String> {
        let listener = TcpListener::bind("0.0.0.0:0").map_err(|e| format!("Nearby can't listen on this network: {e}"))?;
        listener.set_nonblocking(true).map_err(|e| e.to_string())?;
        let port = listener.local_addr().map_err(|e| e.to_string())?.port();
        let stop = Arc::new(AtomicBool::new(false));
        let seen = Arc::new(Mutex::new(HashMap::new()));

        let (stopping, active) = (stop.clone(), Arc::new(AtomicUsize::new(0)));
        let handling = active.clone();
        thread::spawn(move || {
            while !stopping.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        if handling.fetch_add(1, Ordering::SeqCst) >= MAX_CONNECTIONS {
                            handling.fetch_sub(1, Ordering::SeqCst);
                            drop(stream);
                            continue;
                        }
                        let (on_object, done) = (on_object.clone(), handling.clone());
                        thread::spawn(move || {
                            let _ = take(stream, &on_object);
                            done.fetch_sub(1, Ordering::SeqCst);
                        });
                    }
                    Err(_) => thread::sleep(ACCEPT_POLL),
                }
            }
        });

        let mut n = [0u8; 6];
        OsRng.fill_bytes(&mut n);
        // A fresh instance name each time, so the announcement isn't a fixed label.
        let instance: String = n.iter().map(|b| format!("{b:02x}")).collect();
        let mdns = if announce { Some(announce_and_find(fingerprint, &instance, port, &seen, &stop, changed)?) } else { None };
        Ok(Self { stop, active, fingerprint: fingerprint.to_string(), instance, port, seen, mdns })
    }

    #[cfg(test)]
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Where the computer with this fingerprint listens, if it's nearby.
    pub fn find(&self, fingerprint: &str) -> Option<SocketAddr> {
        self.seen.lock().unwrap_or_else(|p| p.into_inner()).get(fingerprint).map(|s| s.at)
    }

    /// Computers nearby that are adding people right now: fingerprint and name.
    pub fn pairing(&self) -> Vec<(String, String)> {
        let seen = self.seen.lock().unwrap_or_else(|p| p.into_inner());
        let mut people: Vec<_> = seen.iter().filter_map(|(fp, s)| Some((fp.clone(), s.pairing.clone()?))).collect();
        people.sort_by(|a, b| a.1.cmp(&b.1));
        people
    }

    /// Announce that you're adding people, under `name`, or stop with `None`.
    pub fn set_pairing(&self, name: Option<&str>) -> Result<(), String> {
        let Some(mdns) = &self.mdns else { return Ok(()) };
        mdns.register(service(&self.fingerprint, &self.instance, self.port, name)?).map_err(|e| format!("Nearby can't announce on this network: {e}"))
    }

    /// Connections being handled right now.
    #[cfg(test)]
    pub fn active(&self) -> usize {
        self.active.load(Ordering::SeqCst)
    }

    /// Record a computer, as discovery does.
    #[cfg(test)]
    pub fn note(&self, fingerprint: &str, at: SocketAddr, pairing: Option<&str>) {
        self.seen.lock().unwrap().insert(fingerprint.to_string(), Seen { at, pairing: pairing.map(str::to_string) });
    }
}

impl Drop for Nearby {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(mdns) = self.mdns.take() {
            let _ = mdns.shutdown();
        }
    }
}

/// The announcement: the fingerprint, and the name while adding people.
fn service(fingerprint: &str, instance: &str, port: u16, pairing: Option<&str>) -> Result<ServiceInfo, String> {
    let mut properties = vec![("fp", fingerprint)];
    if let Some(name) = pairing {
        properties.push(("pair", name));
    }
    Ok(ServiceInfo::new(SERVICE, instance, &format!("keyhandoff-{instance}.local."), "", port, properties.as_slice())
        .map_err(|e| e.to_string())?
        .enable_addr_auto())
}

fn announce_and_find(
    fingerprint: &str,
    instance: &str,
    port: u16,
    seen: &Arc<Mutex<HashMap<String, Seen>>>,
    stop: &Arc<AtomicBool>,
    changed: Arc<dyn Fn() + Send + Sync>,
) -> Result<ServiceDaemon, String> {
    let mdns = ServiceDaemon::new().map_err(|e| format!("Nearby can't use this network: {e}"))?;
    mdns.register(service(fingerprint, instance, port, None)?).map_err(|e| format!("Nearby can't announce on this network: {e}"))?;
    let events = mdns.browse(SERVICE).map_err(|e| e.to_string())?;

    let (seen, stop, me) = (seen.clone(), stop.clone(), fingerprint.to_string());
    // Full names map to fingerprints, so a removal knows whom to forget.
    let mut names: HashMap<String, String> = HashMap::new();
    thread::spawn(move || {
        while !stop.load(Ordering::SeqCst) {
            let Ok(event) = events.recv_timeout(Duration::from_millis(500)) else { continue };
            let mut seen = seen.lock().unwrap_or_else(|p| p.into_inner());
            match event {
                ServiceEvent::ServiceResolved(info) => {
                    let Some(fp) = info.get_property_val_str("fp").map(str::to_string) else { continue };
                    let Some(ip) = info.get_addresses_v4().into_iter().next().copied() else { continue };
                    if fp != me {
                        names.insert(info.get_fullname().to_string(), fp.clone());
                        if seen.len() >= MAX_SEEN && !seen.contains_key(&fp) {
                            continue;
                        }
                        let pairing = info.get_property_val_str("pair").map(str::to_string);
                        seen.insert(fp, Seen { at: SocketAddr::from((ip, info.get_port())), pairing });
                    }
                }
                ServiceEvent::ServiceRemoved(_, fullname) => {
                    if let Some(fp) = names.remove(&fullname) {
                        seen.remove(&fp);
                    }
                }
                _ => continue,
            }
            drop(seen);
            changed();
        }
    });
    Ok(mdns)
}

/// Read one object from a connection, hand it over, and reply. The whole
/// read has one deadline, and memory grows only as bytes actually arrive.
fn take(mut stream: TcpStream, on_object: &OnObject) -> std::io::Result<()> {
    let deadline = Instant::now() + IO_TIMEOUT;
    stream.set_nonblocking(false)?;
    stream.set_write_timeout(Some(IO_TIMEOUT))?;
    let mut len = [0u8; 4];
    read_by(&mut stream, &mut len, deadline)?;
    let len = u32::from_be_bytes(len);
    if len == 0 || len > MAX_OBJECT_BYTES {
        return reply(&mut stream, Err("That's not something KeyHandoff takes.".into()));
    }
    let mut object = Vec::new();
    let mut chunk = [0u8; 16 * 1024];
    while object.len() < len as usize {
        let want = (len as usize - object.len()).min(chunk.len());
        read_by(&mut stream, &mut chunk[..want], deadline)?;
        object.extend_from_slice(&chunk[..want]);
    }
    let result = on_object(object);
    reply(&mut stream, result)
}

/// Fill `buf` before `deadline`, however the bytes are spread out.
fn read_by(stream: &mut TcpStream, buf: &mut [u8], deadline: Instant) -> std::io::Result<()> {
    let mut filled = 0;
    while filled < buf.len() {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(std::io::ErrorKind::TimedOut.into());
        }
        stream.set_read_timeout(Some(left))?;
        match stream.read(&mut buf[filled..])? {
            0 => return Err(std::io::ErrorKind::UnexpectedEof.into()),
            n => filled += n,
        }
    }
    Ok(())
}

fn reply(stream: &mut TcpStream, result: Result<(), String>) -> std::io::Result<()> {
    match result {
        Ok(()) => stream.write_all(&[0]),
        Err(why) => {
            let why = why.as_bytes();
            let len = why.len().min(u16::MAX as usize);
            stream.write_all(&[1])?;
            stream.write_all(&(len as u16).to_be_bytes())?;
            stream.write_all(&why[..len])
        }
    }
}

/// The six digits both people compare when pairing: from both cards, in an
/// order both sides agree on, so each computer works it out the same way. If
/// either card was swapped in transit, the two computers get different codes.
pub fn pairing_code(ours: &[u8], theirs: &[u8]) -> String {
    let (first, second) = if ours <= theirs { (ours, theirs) } else { (theirs, ours) };
    let mut hash = Sha256::new();
    hash.update(b"handoff:pairing-code:v0");
    hash.update((first.len() as u32).to_be_bytes());
    hash.update(first);
    hash.update(second);
    let digest = hash.finalize();
    let n = u32::from_be_bytes([digest[0], digest[1], digest[2], digest[3]]) % 1_000_000;
    format!("{:03} {:03}", n / 1000, n % 1000)
}

/// Send one object to a computer nearby and wait for its answer.
pub fn deliver(to: SocketAddr, object: &[u8]) -> Result<(), String> {
    let unreachable = |_| "Their computer didn't answer. Check you're on the same network, or send the file instead.".to_string();
    let mut stream = TcpStream::connect_timeout(&to, IO_TIMEOUT).map_err(unreachable)?;
    stream.set_read_timeout(Some(IO_TIMEOUT)).map_err(unreachable)?;
    stream.set_write_timeout(Some(IO_TIMEOUT)).map_err(unreachable)?;
    let len = u32::try_from(object.len()).map_err(|_| "That's too big to send nearby.".to_string())?;
    stream.write_all(&len.to_be_bytes()).and_then(|_| stream.write_all(object)).map_err(unreachable)?;
    let mut status = [0u8; 1];
    stream.read_exact(&mut status).map_err(unreachable)?;
    if status[0] == 0 {
        return Ok(());
    }
    let mut len = [0u8; 2];
    stream.read_exact(&mut len).map_err(unreachable)?;
    let mut why = vec![0u8; (u16::from_be_bytes(len) as usize).min(MAX_REASON_BYTES)];
    stream.read_exact(&mut why).map_err(unreachable)?;
    Err(String::from_utf8_lossy(&why).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    /// Someone on the network opens many connections and trickles bytes into
    /// them. Only a few are ever handled, each is cut off at the deadline, and
    /// a real send gets through afterwards.
    #[test]
    fn a_flood_of_slow_connections_is_bounded_and_cut_off() {
        let took: OnObject = Arc::new(|bytes| if bytes == b"key" { Ok(()) } else { Err("no".into()) });
        let n = Nearby::start("aaaaaaaaaaaaaaaa", false, took, Arc::new(|| {})).unwrap();
        let at = SocketAddr::from(([127, 0, 0, 1], n.port()));
        let mut held = Vec::new();
        for _ in 0..40 {
            if let Ok(mut s) = TcpStream::connect(at) {
                // Claims a large object, then trickles.
                let _ = s.write_all(&MAX_OBJECT_BYTES.to_be_bytes());
                held.push(s);
            }
        }
        let trickle = Instant::now() + IO_TIMEOUT + Duration::from_secs(2);
        while Instant::now() < trickle {
            assert!(n.active() <= MAX_CONNECTIONS, "at most {MAX_CONNECTIONS} handled at once");
            for s in &mut held {
                let _ = s.write_all(&[0]);
            }
            thread::sleep(Duration::from_millis(500));
        }
        assert_eq!(n.active(), 0, "every slow connection was cut off at the deadline");
        drop(held);
        deliver(at, b"key").unwrap();
    }

    #[test]
    fn both_sides_get_one_code_and_a_swapped_card_changes_it() {
        let (ann, ben, mallory) = (b"Ann's card".as_slice(), b"Ben's card".as_slice(), b"Mallory's card".as_slice());
        let code = pairing_code(ann, ben);
        assert_eq!(pairing_code(ben, ann), code, "each side works it out the same way");
        assert_eq!(code.len(), 7);
        // Mallory swaps Ben's card for hers on the way to Ann: Ann's code differs from Ben's.
        assert_ne!(pairing_code(ann, mallory), code);
    }

    /// Real mDNS on this machine's network: two announcers find each other.
    /// Ignored by default: CI runners and sandboxes often block multicast.
    #[test]
    #[ignore]
    fn two_computers_find_each_other_over_mdns() {
        let none: OnObject = Arc::new(|_| Ok(()));
        let quiet = Arc::new(|| {});
        let a = Nearby::start("aaaaaaaaaaaaaaaa", true, none.clone(), quiet.clone()).unwrap();
        let b = Nearby::start("bbbbbbbbbbbbbbbb", true, none, quiet).unwrap();
        let until = Instant::now() + Duration::from_secs(15);
        while (a.find("bbbbbbbbbbbbbbbb").is_none() || b.find("aaaaaaaaaaaaaaaa").is_none()) && Instant::now() < until {
            thread::sleep(Duration::from_millis(200));
        }
        assert_eq!(a.find("bbbbbbbbbbbbbbbb").map(|s| s.port()), Some(b.port()));
        assert_eq!(b.find("aaaaaaaaaaaaaaaa").map(|s| s.port()), Some(a.port()));
    }
}
