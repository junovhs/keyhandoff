//! A small TPM 2.0 client: just enough to seal the master key inside the
//! computer's TPM behind a PIN, and unseal it. Written out by hand (about
//! five commands) so KeyHandoff needs no TPM libraries installed.
//!
//! The sealed object is a keyed-hash object under a P-256 storage key that
//! the TPM derives from its owner seed, so it's recreated on each use and
//! nothing is kept inside the TPM. Its blobs only load into this TPM, and
//! unsealing needs the PIN's Argon2id hash as its password; the TPM counts
//! wrong ones and locks out a guesser (dictionary-attack protection).
//!
//! The PIN travels to the TPM in a password session, in the clear on the bus
//! between CPU and a discrete TPM chip. That bus needs someone at the
//! computer with a probe, which is outside what this guards against.

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};

use zeroize::Zeroizing;

const ST_NO_SESSIONS: u16 = 0x8001;
const ST_SESSIONS: u16 = 0x8002;
const CC_CREATE_PRIMARY: u32 = 0x131;
const CC_CREATE: u32 = 0x153;
const CC_LOAD: u32 = 0x157;
const CC_UNSEAL: u32 = 0x15E;
const CC_FLUSH_CONTEXT: u32 = 0x165;
const RH_OWNER: u32 = 0x4000_0001;
const RS_PW: u32 = 0x4000_0009;
const RC_LOCKOUT: u32 = 0x921;
const RC_RETRY: u32 = 0x922;
const RC_TESTING: u32 = 0x90A;
const RC_YIELDED: u32 = 0x908;

/// Sends one command and returns the TPM's response.
pub trait Transport {
    fn send(&mut self, command: &[u8]) -> io::Result<Vec<u8>>;
}

/// The kernel's resource-managed TPM device.
pub struct Device(File);

pub const DEVICE: &str = "/dev/tpmrm0";

impl Device {
    pub fn open() -> io::Result<Self> {
        OpenOptions::new().read(true).write(true).open(DEVICE).map(Self)
    }
}

impl Transport for Device {
    fn send(&mut self, command: &[u8]) -> io::Result<Vec<u8>> {
        self.0.write_all(command)?;
        let mut buf = vec![0u8; 4096];
        let n = self.0.read(&mut buf)?;
        buf.truncate(n);
        Ok(buf)
    }
}

/// Whether this computer has a TPM KeyHandoff can use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Ready,
    /// There's a TPM, but this user may not open it (the `tss` group).
    NoAccess,
    /// No TPM: there isn't one, or it's off in the firmware settings.
    Missing,
}

pub fn status() -> Status {
    match Device::open() {
        Ok(_) => Status::Ready,
        Err(e) if e.kind() == io::ErrorKind::PermissionDenied => Status::NoAccess,
        Err(_) => Status::Missing,
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    WrongPin,
    /// Too many wrong PINs: the TPM refuses tries for a while.
    LockedOut,
    Tpm(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Error::WrongPin => write!(f, "That PIN isn't right."),
            Error::LockedOut => write!(f, "Too many wrong PINs, so the TPM is refusing tries for a while. Use your passphrase."),
            Error::Tpm(e) => write!(f, "The TPM couldn't open the key ({e}). Use your passphrase."),
        }
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Tpm(e.to_string())
    }
}

/// The sealed object's blobs, as the TPM returned them.
pub struct Sealed {
    pub private: Vec<u8>,
    pub public: Vec<u8>,
}

/// Seal `secret` in the TPM, unsealable only with `auth`.
pub fn seal(t: &mut dyn Transport, auth: &[u8; 32], secret: &[u8; 32]) -> Result<Sealed, Error> {
    let parent = primary(t)?;
    let result = (|| {
        let mut sensitive = Buf::default();
        sensitive.b2b(auth).b2b(secret);
        let mut public = Buf::default();
        // Keyed hash, SHA-256 name; fixedTPM | fixedParent | userWithAuth;
        // no policy; null scheme; empty unique. DA protection stays on.
        public.u16(0x0008).u16(0x000B).u32(0x0000_0052).u16(0).u16(0x0010).u16(0);
        let mut params = Buf::default();
        params.b2b(&sensitive.0).b2b(&public.0).u16(0).u32(0);
        let mut r = run(t, CC_CREATE, &[parent], Some(&[]), &params.0, false)?;
        Ok(Sealed { private: r.b2b()?.to_vec(), public: r.b2b()?.to_vec() })
    })();
    flush(t, parent);
    result
}

/// The secret sealed in `sealed`, if `auth` is right.
pub fn unseal(t: &mut dyn Transport, sealed: &Sealed, auth: &[u8; 32]) -> Result<Zeroizing<Vec<u8>>, Error> {
    let parent = primary(t)?;
    let item = (|| {
        let mut params = Buf::default();
        params.b2b(&sealed.private).b2b(&sealed.public);
        run(t, CC_LOAD, &[parent], Some(&[]), &params.0, true)?.u32()
    })();
    flush(t, parent);
    let item = item?;
    let result = run(t, CC_UNSEAL, &[item], Some(auth), &[], false).and_then(|mut r| Ok(Zeroizing::new(r.b2b()?.to_vec())));
    flush(t, item);
    result
}

/// The storage key, rebuilt from the owner seed (same key every time until
/// the TPM is cleared).
fn primary(t: &mut dyn Transport) -> Result<u32, Error> {
    let mut public = Buf::default();
    // ECC P-256, SHA-256 name; fixedTPM | fixedParent | sensitiveDataOrigin |
    // userWithAuth | noDA | restricted | decrypt; AES-128-CFB; null scheme
    // and KDF; empty unique.
    public.u16(0x0023).u16(0x000B).u32(0x0003_0472).u16(0);
    public.u16(0x0006).u16(128).u16(0x0043).u16(0x0010).u16(0x0003).u16(0x0010).u16(0).u16(0);
    let mut params = Buf::default();
    params.u16(4).u16(0).u16(0).b2b(&public.0).u16(0).u32(0);
    run(t, CC_CREATE_PRIMARY, &[RH_OWNER], Some(&[]), &params.0, true)?.u32()
}

fn flush(t: &mut dyn Transport, handle: u32) {
    let mut params = Buf::default();
    params.u32(handle);
    let _ = run(t, CC_FLUSH_CONTEXT, &[], None, &params.0, false);
}

/// Send a command (with one password session when `auth` is given) and
/// return its response parameters; `handle_out` skips to after the handle
/// the command returns.
fn run(t: &mut dyn Transport, cc: u32, handles: &[u32], auth: Option<&[u8]>, params: &[u8], handle_out: bool) -> Result<Reader, Error> {
    let mut cmd = Buf::default();
    cmd.u16(if auth.is_some() { ST_SESSIONS } else { ST_NO_SESSIONS }).u32(0).u32(cc);
    handles.iter().for_each(|h| {
        cmd.u32(*h);
    });
    if let Some(pw) = auth {
        cmd.u32(4 + 2 + 1 + 2 + pw.len() as u32).u32(RS_PW).u16(0).u8(0).b2b(pw);
    }
    cmd.0.extend_from_slice(params);
    let len = cmd.0.len() as u32;
    cmd.0[2..6].copy_from_slice(&len.to_be_bytes());
    for _ in 0..10 {
        let resp = Zeroizing::new(t.send(&cmd.0)?);
        let mut r = Reader { buf: resp.to_vec(), at: 0 };
        let tag = r.u16()?;
        let _size = r.u32()?;
        let rc = r.u32()?;
        match rc {
            0 => {}
            RC_RETRY | RC_TESTING | RC_YIELDED => continue,
            RC_LOCKOUT => return Err(Error::LockedOut),
            // Format-one AUTH_FAIL or BAD_AUTH, for any session.
            rc if rc & 0x80 != 0 && matches!(rc & 0x3F, 0x0E | 0x22) => return Err(Error::WrongPin),
            rc => return Err(Error::Tpm(format!("TPM error 0x{rc:03x}"))),
        }
        if handle_out {
            // Leave the returned handle first: callers read it with u32().
            let handle = r.u32()?;
            if tag == ST_SESSIONS {
                r.u32()?;
            }
            return Ok(Reader { buf: handle.to_be_bytes().to_vec(), at: 0 });
        }
        if tag == ST_SESSIONS {
            r.u32()?;
        }
        return Ok(r);
    }
    Err(Error::Tpm("the TPM stayed busy".into()))
}

#[derive(Default)]
struct Buf(Vec<u8>);

impl Buf {
    fn u8(&mut self, v: u8) -> &mut Self {
        self.0.push(v);
        self
    }
    fn u16(&mut self, v: u16) -> &mut Self {
        self.0.extend_from_slice(&v.to_be_bytes());
        self
    }
    fn u32(&mut self, v: u32) -> &mut Self {
        self.0.extend_from_slice(&v.to_be_bytes());
        self
    }
    /// A TPM2B: a 16-bit size, then the bytes.
    fn b2b(&mut self, v: &[u8]) -> &mut Self {
        self.u16(v.len() as u16);
        self.0.extend_from_slice(v);
        self
    }
}

impl Drop for Buf {
    fn drop(&mut self) {
        zeroize::Zeroize::zeroize(&mut self.0);
    }
}

struct Reader {
    buf: Vec<u8>,
    at: usize,
}

impl Drop for Reader {
    fn drop(&mut self) {
        zeroize::Zeroize::zeroize(&mut self.buf);
    }
}

impl Reader {
    fn take(&mut self, n: usize) -> Result<&[u8], Error> {
        let end = self.at.checked_add(n).filter(|&e| e <= self.buf.len()).ok_or_else(|| Error::Tpm("short response".into()))?;
        let out = &self.buf[self.at..end];
        self.at = end;
        Ok(out)
    }
    fn u16(&mut self) -> Result<u16, Error> {
        Ok(u16::from_be_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u32(&mut self) -> Result<u32, Error> {
        Ok(u32::from_be_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn b2b(&mut self) -> Result<&[u8], Error> {
        let n = self.u16()? as usize;
        self.take(n)
    }
}

/// The TPM to use: the device, or in debug builds the software TPM at
/// `KEYHANDOFF_SWTPM=port`, so the PIN flow can be tried without one.
pub fn open() -> Result<Box<dyn Transport>, String> {
    #[cfg(debug_assertions)]
    if let Some(port) = std::env::var("KEYHANDOFF_SWTPM").ok().and_then(|p| p.parse().ok()) {
        let stream = std::net::TcpStream::connect(("127.0.0.1", port)).map_err(|e| e.to_string())?;
        return Ok(Box::new(sim::Sim(stream)));
    }
    Device::open().map(|d| Box::new(d) as Box<dyn Transport>).map_err(|e| format!("The TPM can't be opened: {e}"))
}

/// Whether there's a TPM to use, counting the debug software TPM.
pub fn available() -> Status {
    #[cfg(debug_assertions)]
    if std::env::var("KEYHANDOFF_SWTPM").is_ok() {
        return Status::Ready;
    }
    status()
}

/// swtpm, the software TPM, over its TCP socket; for tests and debug builds.
#[cfg(any(test, debug_assertions))]
pub mod sim {
    use super::*;
    use std::net::TcpStream;
    #[cfg(test)]
    use std::process::{Child, Command};

    #[cfg(test)]
    pub struct Swtpm {
        child: Child,
        dir: std::path::PathBuf,
        port: u16,
    }

    #[cfg(test)]
    impl Swtpm {
        /// A fresh software TPM, or `None` when swtpm isn't installed.
        pub fn start() -> Option<Self> {
            use rand_core::RngCore;
            let mut n = [0u8; 8];
            rand_core::OsRng.fill_bytes(&mut n);
            let dir = std::env::temp_dir().join(format!("handoff-swtpm-{}", u64::from_le_bytes(n)));
            std::fs::create_dir_all(&dir).ok()?;
            for _ in 0..20 {
                let port = 20000 + (rand_core::OsRng.next_u32() % 20000) as u16;
                let child = Command::new("swtpm")
                    .args(["socket", "--tpm2", "--flags", "not-need-init,startup-clear"])
                    .arg("--tpmstate")
                    .arg(format!("dir={}", dir.display()))
                    .args(["--server", &format!("type=tcp,port={port}"), "--ctrl", &format!("type=tcp,port={}", port + 1)])
                    .spawn()
                    .ok()?;
                let tpm = Self { child, dir: dir.clone(), port };
                for _ in 0..50 {
                    std::thread::sleep(std::time::Duration::from_millis(20));
                    if TcpStream::connect(("127.0.0.1", port)).is_ok() {
                        return Some(tpm);
                    }
                }
            }
            None
        }

        pub fn connect(&self) -> Sim {
            Sim(TcpStream::connect(("127.0.0.1", self.port)).unwrap())
        }
    }

    #[cfg(test)]
    impl Drop for Swtpm {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    /// The TCG simulator framing swtpm speaks: send-command, locality, size.
    pub struct Sim(pub TcpStream);

    impl Transport for Sim {
        fn send(&mut self, command: &[u8]) -> io::Result<Vec<u8>> {
            let mut out = Vec::with_capacity(9 + command.len());
            out.extend_from_slice(&8u32.to_be_bytes());
            out.push(0);
            out.extend_from_slice(&(command.len() as u32).to_be_bytes());
            out.extend_from_slice(command);
            self.0.write_all(&out)?;
            let mut len = [0u8; 4];
            self.0.read_exact(&mut len)?;
            let mut resp = vec![0u8; u32::from_be_bytes(len) as usize];
            self.0.read_exact(&mut resp)?;
            let mut ack = [0u8; 4];
            self.0.read_exact(&mut ack)?;
            Ok(resp)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::sim::Swtpm;
    use super::*;

    #[test]
    fn sealed_key_opens_only_with_the_pin_on_the_same_tpm() {
        let Some(tpm) = Swtpm::start() else {
            eprintln!("swtpm isn't installed; skipping");
            return;
        };
        let mut t = tpm.connect();
        let (auth, wrong, secret) = ([7u8; 32], [8u8; 32], [42u8; 32]);
        let sealed = seal(&mut t, &auth, &secret).unwrap();
        assert!(!sealed.private.windows(32).any(|w| w == secret), "the sealed blob holds the key in the clear");

        assert_eq!(&unseal(&mut t, &sealed, &auth).unwrap()[..], &secret[..]);
        assert_eq!(unseal(&mut t, &sealed, &wrong).err(), Some(Error::WrongPin));
        assert_eq!(&unseal(&mut t, &sealed, &auth).unwrap()[..], &secret[..], "the right PIN still works");

        // Another TPM can't load it.
        let other = Swtpm::start().unwrap();
        assert!(matches!(unseal(&mut other.connect(), &sealed, &auth), Err(Error::Tpm(_))));
    }
}
