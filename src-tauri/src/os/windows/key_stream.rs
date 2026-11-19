//! Deadlines for the synchronous key pipe. A timed read polls how many bytes
//! are waiting and reads only those, so it never blocks and never needs to
//! change the pipe's mode; a timed write switches the handle to nonblocking.
//! Untimed reads (waiting for a runner to exit) use normal blocking I/O.
//!
//! Reads must not depend on changing the mode: once the other end has written
//! its last bytes and closed, `SetNamedPipeHandleState` fails, and a reader
//! that gave up there would lose a reply already sitting in the pipe.

use std::fs::File;
use std::io::{self, Read, Write};
use std::os::windows::io::AsRawHandle;
use std::time::{Duration, Instant};

use ::windows::Win32::Foundation::HANDLE;
use ::windows::Win32::System::Pipes::{PeekNamedPipe, SetNamedPipeHandleState, PIPE_NOWAIT, PIPE_READMODE_BYTE, PIPE_WAIT};

pub(super) struct KeyPipe {
    file: File,
    read_timeout: Option<Duration>,
    write_timeout: Option<Duration>,
    nonblocking: bool,
}

impl KeyPipe {
    pub(super) fn new(file: File) -> Self {
        Self { file, read_timeout: None, write_timeout: None, nonblocking: false }
    }

    fn handle(&self) -> HANDLE {
        HANDLE(self.file.as_raw_handle())
    }

    fn set_nonblocking(&mut self, nonblocking: bool) -> io::Result<()> {
        if self.nonblocking == nonblocking {
            return Ok(());
        }
        let mode = PIPE_READMODE_BYTE | if nonblocking { PIPE_NOWAIT } else { PIPE_WAIT };
        // SAFETY: this owns a connected byte pipe; only this worker uses it.
        unsafe { SetNamedPipeHandleState(self.handle(), Some(&mode), None, None) }
            .map_err(|e| io::Error::from_raw_os_error(e.code().0 & 0xffff))?;
        self.nonblocking = nonblocking;
        Ok(())
    }

    /// Bytes waiting to be read, or `None` once the pipe is closed or broken.
    fn waiting(&self) -> Option<usize> {
        let mut available = 0u32;
        // SAFETY: a peek of the count only; no buffer is passed.
        unsafe { PeekNamedPipe(self.handle(), None, 0, None, Some(&mut available), None) }.ok()?;
        Some(available as usize)
    }
}

fn pause(deadline: Instant) -> io::Result<()> {
    let left = deadline.saturating_duration_since(Instant::now());
    if left.is_zero() {
        return Err(io::ErrorKind::TimedOut.into());
    }
    std::thread::sleep(left.min(Duration::from_millis(10)));
    Ok(())
}

impl Read for KeyPipe {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let Some(timeout) = self.read_timeout else {
            // A failed switch back means the pipe is closing; the read below
            // then returns what is left or the end, without blocking.
            let _ = self.set_nonblocking(false);
            return self.file.read(buf);
        };
        let deadline = Instant::now() + timeout;
        loop {
            match self.waiting() {
                // Reading no more than is waiting returns at once in either mode.
                Some(0) => pause(deadline)?,
                Some(n) => {
                    let n = n.min(buf.len());
                    return self.file.read(&mut buf[..n]);
                }
                // Closed or broken: the read reports the end or the error at once.
                None => return self.file.read(buf),
            }
        }
    }
}

impl Write for KeyPipe {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        self.set_nonblocking(self.write_timeout.is_some())?;
        let deadline = self.write_timeout.map(|t| Instant::now() + t);
        loop {
            match self.file.write(buf) {
                // A full nonblocking byte pipe can write zero bytes. Wait only
                // until the caller's deadline; never spin or block on the peer.
                Ok(0) if deadline.is_some() => pause(deadline.unwrap())?,
                result => return result,
            }
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        // FlushFileBuffers would wait for the other process to read the reply.
        // File::flush is a no-op; the pipe's writes already queue the bytes.
        Ok(())
    }
}

impl crate::os::KeyStream for KeyPipe {
    fn set_read_timeout(&mut self, timeout: Option<Duration>) -> io::Result<()> {
        self.read_timeout = timeout;
        Ok(())
    }

    fn set_write_timeout(&mut self, timeout: Option<Duration>) -> io::Result<()> {
        self.write_timeout = timeout;
        Ok(())
    }
}
