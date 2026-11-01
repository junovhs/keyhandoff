//! Nearby: other KeyHandoffs on the local network. While it's on, this
//! computer announces itself over mDNS (`_keyhandoff._tcp`, with only its
//! fingerprint), finds the others, and listens for sealed keys. One connection
//! carries one object: a big-endian length, the object's bytes, then a reply
//! (0 for taken, or 1 and a reason). What arrives is still a sealed key: the
//! network only moves locked boxes, and the core decides whether to take it.
//!
//! It stops when dropped, which happens when KeyHandoff locks.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use rand_core::{OsRng, RngCore};

const SERVICE: &str = "_keyhandoff._tcp.local.";
/// A sealed key or a team's bundle fits easily; anything bigger isn't ours.
const MAX_OBJECT_BYTES: u32 = 256 * 1024;
const IO_TIMEOUT: Duration = Duration::from_secs(5);
const ACCEPT_POLL: Duration = Duration::from_millis(100);

/// What to do with an object that arrived: take it, or say why not.
pub type OnObject = Arc<dyn Fn(Vec<u8>) -> Result<(), String> + Send + Sync>;

pub struct Nearby {
    stop: Arc<AtomicBool>,
    #[cfg_attr(not(test), allow(dead_code))]
    port: u16,
    /// Fingerprint → where that computer listens, as last announced.
    seen: Arc<Mutex<HashMap<String, SocketAddr>>>,
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

        let stopping = stop.clone();
        thread::spawn(move || {
            while !stopping.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let on_object = on_object.clone();
                        thread::spawn(move || {
                            let _ = take(stream, &on_object);
                        });
                    }
                    Err(_) => thread::sleep(ACCEPT_POLL),
                }
            }
        });

        let mdns = if announce { Some(announce_and_find(fingerprint, port, &seen, &stop, changed)?) } else { None };
        Ok(Self { stop, port, seen, mdns })
    }

    #[cfg(test)]
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Where the computer with this fingerprint listens, if it's nearby.
    pub fn find(&self, fingerprint: &str) -> Option<SocketAddr> {
        self.seen.lock().unwrap_or_else(|p| p.into_inner()).get(fingerprint).copied()
    }

    /// Record where a computer listens, as discovery does.
    #[cfg(test)]
    pub fn note(&self, fingerprint: &str, at: SocketAddr) {
        self.seen.lock().unwrap().insert(fingerprint.to_string(), at);
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

fn announce_and_find(
    fingerprint: &str,
    port: u16,
    seen: &Arc<Mutex<HashMap<String, SocketAddr>>>,
    stop: &Arc<AtomicBool>,
    changed: Arc<dyn Fn() + Send + Sync>,
) -> Result<ServiceDaemon, String> {
    let mdns = ServiceDaemon::new().map_err(|e| format!("Nearby can't use this network: {e}"))?;
    // A fresh instance name each time, so the announcement isn't a fixed label.
    let mut n = [0u8; 6];
    OsRng.fill_bytes(&mut n);
    let instance: String = n.iter().map(|b| format!("{b:02x}")).collect();
    let info = ServiceInfo::new(SERVICE, &instance, &format!("keyhandoff-{instance}.local."), "", port, [("fp", fingerprint)].as_slice())
        .map_err(|e| e.to_string())?
        .enable_addr_auto();
    mdns.register(info).map_err(|e| format!("Nearby can't announce on this network: {e}"))?;
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
                        seen.insert(fp, SocketAddr::from((ip, info.get_port())));
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

/// Read one object from a connection, hand it over, and reply.
fn take(mut stream: TcpStream, on_object: &OnObject) -> std::io::Result<()> {
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(IO_TIMEOUT))?;
    stream.set_write_timeout(Some(IO_TIMEOUT))?;
    let mut len = [0u8; 4];
    stream.read_exact(&mut len)?;
    let len = u32::from_be_bytes(len);
    if len == 0 || len > MAX_OBJECT_BYTES {
        return reply(&mut stream, Err("That's not something KeyHandoff takes.".into()));
    }
    let mut object = vec![0u8; len as usize];
    stream.read_exact(&mut object)?;
    let result = on_object(object);
    reply(&mut stream, result)
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
    let mut why = vec![0u8; u16::from_be_bytes(len) as usize];
    stream.read_exact(&mut why).map_err(unreachable)?;
    Err(String::from_utf8_lossy(&why).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

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
