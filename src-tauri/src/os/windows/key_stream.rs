//! Deadlines for the synchronous key pipe. Timed operations poll a nonblocking
//! handle; untimed reads (waiting for a runner to exit) use normal blocking I/O.

use std::fs::File;
use std::io::{self, Read, Write};
use std::os::windows::io::AsRawHandle;
use std::time::{Duration, Instant};

use ::windows::Win32::Foundation::{ERROR_NO_DATA, HANDLE};
use ::windows::Win32::System::Pipes::{SetNamedPipeHandleState, PIPE_NOWAIT, PIPE_READMODE_BYTE, PIPE_WAIT};

pub(super) struct KeyPipe {
    file: File,
    read_timeout: Option<Duration>,
    write_timeout: Option<Duration>,
}

impl KeyPipe {
    pub(super) fn new(file: File) -> Self {
        Self { file, read_timeout: None, write_timeout: None }
    }

    fn wait_mode(&self, timed: bool) -> io::Result<()> {
        let mode = PIPE_READMODE_BYTE | if timed { PIPE_NOWAIT } else { PIPE_WAIT };
        // SAFETY: this owns a connected byte pipe; only this worker uses it.
        unsafe { SetNamedPipeHandleState(HANDLE(self.file.as_raw_handle()), Some(&mode), None, None) }
            .map_err(|e| io::Error::from_raw_os_error(e.code().0 & 0xffff))
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
        if buf.is_empty() { return Ok(0); }
        self.wait_mode(self.read_timeout.is_some())?;
        let deadline = self.read_timeout.map(|t| Instant::now() + t);
        loop {
            match self.file.read(buf) {
                Err(e) if e.raw_os_error() == Some(ERROR_NO_DATA.0 as i32) && deadline.is_some() => pause(deadline.unwrap())?,
                result => return result,
            }
        }
    }
}

impl Write for KeyPipe {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if buf.is_empty() { return Ok(0); }
        self.wait_mode(self.write_timeout.is_some())?;
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
