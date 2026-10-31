//! The local named pipe server that launch forwarding and key requests share.
//! The pipe's default security lets only this user (and admins) write to it,
//! and remote clients are rejected.

use std::fs::File;
use std::os::windows::io::FromRawHandle;
use std::thread;

use ::windows::core::HSTRING;
use ::windows::Win32::Foundation::{CloseHandle, ERROR_PIPE_CONNECTED};
use ::windows::Win32::Storage::FileSystem::{FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_ACCESS_DUPLEX, PIPE_ACCESS_INBOUND};
use ::windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE, PIPE_UNLIMITED_INSTANCES,
    PIPE_WAIT,
};

/// Serve the pipe `name` on a thread called `thread`, handing each client that
/// connects to `on_client` as a `File` that owns that pipe instance. With
/// `duplex` the server can reply; otherwise it only reads. The first instance
/// must be ours, so a second copy of the app can't take the name. If a pipe
/// can't be made, the error is logged and serving stops.
pub fn serve(name: &str, duplex: bool, buffer: u32, thread: &str, mut on_client: impl FnMut(File) + Send + 'static) -> Result<(), String> {
    let name = HSTRING::from(name);
    let access = if duplex { PIPE_ACCESS_DUPLEX } else { PIPE_ACCESS_INBOUND };
    let out_buffer = if duplex { buffer } else { 0 };
    thread::Builder::new()
        .name(thread.into())
        .spawn(move || {
            let mut first = true;
            loop {
                let flags = if first { access | FILE_FLAG_FIRST_PIPE_INSTANCE } else { access };
                first = false;
                // SAFETY: plain pipe creation; the handle is owned below.
                let handle = unsafe {
                    CreateNamedPipeW(
                        &name,
                        flags,
                        PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                        PIPE_UNLIMITED_INSTANCES,
                        out_buffer,
                        buffer,
                        0,
                        None,
                    )
                };
                if handle.is_invalid() {
                    eprintln!("keyhandoff: no pipe {name}: {}", ::windows::core::Error::from_thread());
                    return;
                }
                // SAFETY: a client may connect between create and connect; that is ERROR_PIPE_CONNECTED.
                let connected = match unsafe { ConnectNamedPipe(handle, None) } {
                    Ok(()) => true,
                    Err(e) => e.code() == ERROR_PIPE_CONNECTED.to_hresult(),
                };
                if !connected {
                    // SAFETY: we own the handle.
                    let _ = unsafe { CloseHandle(handle) };
                    continue;
                }
                // SAFETY: the File takes ownership of the handle and closes it.
                on_client(unsafe { File::from_raw_handle(handle.0) });
            }
        })
        .map(drop)
        .map_err(|e| e.to_string())
}
