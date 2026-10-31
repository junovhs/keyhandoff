//! Programs ask for kept keys over a local named pipe (`pipe.rs`). Windows reports the
//! client's process id; its executable comes from the process, and its
//! working folder from the process parameters in its PEB.

use std::ffi::c_void;
use std::os::windows::io::AsRawHandle;
use std::path::PathBuf;
use std::thread;

use ::windows::core::PWSTR;
use ::windows::Wdk::System::Threading::{NtQueryInformationProcess, ProcessBasicInformation};
use ::windows::Win32::Foundation::{CloseHandle, HANDLE};
use ::windows::Win32::System::Diagnostics::Debug::ReadProcessMemory;
use ::windows::Win32::System::Pipes::GetNamedPipeClientProcessId;
use ::windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_BASIC_INFORMATION, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_VM_READ,
};

use crate::os::{Peer, ServeKeys};

const BUFFER: u32 = 64 * 1024;

pub fn pipe(profile: &str) -> String {
    format!("{}-keys", super::instance::pipe(profile))
}

/// Serve programs asking for keys, each connection on its own thread.
pub fn listen_for_keys(profile: &str, serve: ServeKeys) -> Result<(), String> {
    super::pipe::serve(&pipe(profile), true, BUFFER, "keys", move |stream| {
        let Some(peer) = peer(HANDLE(stream.as_raw_handle())) else { return };
        let serve = serve.clone();
        let _ = thread::Builder::new().name("key-ask".into()).spawn(move || serve(peer, Box::new(stream)));
    })
}

/// The process on the other end of the pipe.
fn peer(pipe: HANDLE) -> Option<Peer> {
    let mut pid = 0u32;
    // SAFETY: `pipe` is a connected server end.
    unsafe { GetNamedPipeClientProcessId(pipe, &mut pid) }.ok()?;
    // SAFETY: plain process open; closed below.
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_VM_READ, false, pid) }
        .or_else(|_| unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) })
        .ok()?;
    let program = image_path(process);
    let folder = program.as_ref().and_then(|_| current_dir(process));
    let command = program.as_ref().and_then(|_| command_line(process));
    // SAFETY: we own the handle.
    let _ = unsafe { CloseHandle(process) };
    Some(Peer { program: program?, folder, command })
}

fn image_path(process: HANDLE) -> Option<PathBuf> {
    let mut buf = vec![0u16; 32 * 1024];
    let mut len = buf.len() as u32;
    // SAFETY: `buf` holds `len` units; on success `len` is the length written.
    unsafe { QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len) }.ok()?;
    Some(PathBuf::from(String::from_utf16_lossy(&buf[..len as usize])))
}

/// `RTL_USER_PROCESS_PARAMETERS.CurrentDirectory`. A 32-bit process or one we
/// can't read gives `None`.
fn current_dir(process: HANDLE) -> Option<PathBuf> {
    let path = parameter_string(process, 0x38)?;
    // Always ends in a backslash; keep it only for a drive root.
    let path = if path.len() > 3 { path.trim_end_matches('\\') } else { &path };
    Some(PathBuf::from(path))
}

/// `RTL_USER_PROCESS_PARAMETERS.CommandLine`, for display.
fn command_line(process: HANDLE) -> Option<String> {
    let line = parameter_string(process, 0x70)?;
    super::super::display_command(split_command_line(&line).iter().map(String::as_str))
}

/// Windows quoting, enough for display: spaces split, double quotes group.
fn split_command_line(line: &str) -> Vec<String> {
    let (mut args, mut arg, mut quoted) = (Vec::new(), String::new(), false);
    for c in line.chars() {
        match c {
            '"' => quoted = !quoted,
            ' ' | '\t' if !quoted => {
                if !arg.is_empty() {
                    args.push(std::mem::take(&mut arg));
                }
            }
            c => arg.push(c),
        }
    }
    if !arg.is_empty() {
        args.push(arg);
    }
    args
}

/// A UNICODE_STRING inside the process parameters, at its x64 offset.
#[cfg(target_pointer_width = "64")]
fn parameter_string(process: HANDLE, offset: usize) -> Option<String> {
    // SAFETY: all-zero is a valid PROCESS_BASIC_INFORMATION (null pointers, zero fields).
    let mut info: PROCESS_BASIC_INFORMATION = unsafe { std::mem::zeroed() };
    let size = std::mem::size_of::<PROCESS_BASIC_INFORMATION>() as u32;
    // SAFETY: `info` is `size` bytes.
    let status = unsafe { NtQueryInformationProcess(process, ProcessBasicInformation, (&raw mut info).cast(), size, std::ptr::null_mut()) };
    if status.is_err() {
        return None;
    }
    let params: usize = read(process, info.PebBaseAddress as usize + 0x20)?;
    let bytes: u16 = read(process, params + offset)?;
    let buffer: usize = read(process, params + offset + 8)?;
    let mut units = vec![0u16; usize::from(bytes) / 2];
    read_into(process, buffer, &mut units)?;
    String::from_utf16(&units).ok()
}

#[cfg(not(target_pointer_width = "64"))]
fn parameter_string(_process: HANDLE, _offset: usize) -> Option<String> {
    None
}

fn read<T: Copy + Default>(process: HANDLE, at: usize) -> Option<T> {
    let mut value = T::default();
    read_into(process, at, std::slice::from_mut(&mut value))?;
    Some(value)
}

fn read_into<T>(process: HANDLE, at: usize, out: &mut [T]) -> Option<()> {
    let size = std::mem::size_of_val(out);
    let mut got = 0usize;
    // SAFETY: `out` is `size` writable bytes; the read is checked by the OS.
    unsafe { ReadProcessMemory(process, at as *const c_void, out.as_mut_ptr().cast(), size, Some(&mut got)) }.ok()?;
    (got == size).then_some(())
}
