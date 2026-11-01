"""Fill environment variables from KeyHandoff pointers in .env.

A .env line such as ``DIALPAD_API_KEY=handoff:Dialpad`` names a key kept in
KeyHandoff instead of holding it. ``load()`` asks the running app for every
pointed-to key at once. The first time a program asks from a folder,
KeyHandoff asks you to allow it. If KeyHandoff is locked, it comes forward
and this waits for you to unlock it. The values are set in ``os.environ`` for
this process only; nothing is written to disk.

    import keyhandoff_env; keyhandoff_env.load()

Everything this process and the programs it starts print to stdout and
stderr then passes through a filter that shows a key, plain, base64, hex,
URL-encoded or reversed, as ``[KeyHandoff: Name hidden]``. It catches a key
printed by accident; code that means to send it somewhere else can. Pass
``mask=False`` to leave output alone.

Standard library only: copy this file next to your script. Python strings
can't be wiped, so the value stays in this process's memory until it exits.
"""

import atexit
import base64
import json
import os
import re
import socket
import sys
import tempfile
import threading

__all__ = ["load", "KeyHandoffError"]

PREFIX = "handoff:"
NOT_RUNNING = "KeyHandoff isn't running. Open it and try again."


class KeyHandoffError(RuntimeError):
    pass


def load(path=".env", profile=None, mask=True):
    """Set every variable whose value is a ``handoff:`` pointer, from the
    environment or from the .env file at ``path``. A variable already set to
    a real value is left alone. Returns the names of the variables set."""
    pointers = {var: value[len(PREFIX):].strip() for var, value in os.environ.items() if value.startswith(PREFIX)}
    for var, value in _read_env(path):
        if value.startswith(PREFIX) and (var not in os.environ or var in pointers):
            pointers[var] = value[len(PREFIX):].strip()
    if not pointers:
        return []
    names = sorted(set(pointers.values()))
    reply = _ask(names, profile or os.environ.get("KEYHANDOFF_PROFILE") or "default")
    if "error" in reply:
        raise KeyHandoffError(reply["error"])
    for var, name in pointers.items():
        os.environ[var] = reply["values"][name]
    if mask:
        _mask_output(reply["values"])
    return sorted(pointers)


def _read_env(path):
    try:
        with open(path, encoding="utf-8") as f:
            lines = f.read().splitlines()
    except FileNotFoundError:
        return
    for line in lines:
        line = line.strip()
        if line.startswith("export "):
            line = line[len("export "):].lstrip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        var, value = (part.strip() for part in line.split("=", 1))
        if len(value) >= 2 and value[0] == value[-1] and value[0] in "'\"":
            value = value[1:-1]
        elif " #" in value:
            value = value.split(" #", 1)[0].rstrip()
        yield var, value


def _ask(names, profile):
    message = (json.dumps({"names": names}) + "\n").encode()
    if os.name == "nt":
        user = "".join(c for c in os.environ.get("USERNAME", "") if c.isascii() and c.isalnum())
        try:
            pipe = open(rf"\\.\pipe\handoff-{user}-{profile}-keys", "r+b", buffering=0)
        except OSError:
            raise KeyHandoffError(NOT_RUNNING) from None
        with pipe:
            pipe.write(message)
            return json.loads(_read_line(pipe.read))
    runtime = os.environ.get("XDG_RUNTIME_DIR") or tempfile.gettempdir()
    with socket.socket(socket.AF_UNIX) as conn:
        try:
            conn.connect(os.path.join(runtime, f"handoff-{profile}-keys.sock"))
        except OSError:
            raise KeyHandoffError(NOT_RUNNING) from None
        conn.sendall(message)
        return json.loads(_read_line(conn.recv))


def _read_line(read):
    data = b""
    while not data.endswith(b"\n"):
        chunk = read(64 * 1024)
        if not chunk:
            raise KeyHandoffError("KeyHandoff closed the connection.")
        data += chunk
    return data


# Hiding printed keys. Same rules as KeyHandoff's own masker (src-tauri/src/mask.rs).

FRAGMENT = 12


def _forms(key):
    """The ways a key gets printed."""
    forms = {key, key[::-1], key.hex().encode(), key.hex().upper().encode()}
    for encode in (base64.b64encode, base64.urlsafe_b64encode):
        for offset in range(3):
            encoded = encode(bytes(offset) + key)
            skip = -(-offset * 8 // 6) if offset else 0
            keep = (offset + len(key)) * 8 // 6
            if keep > skip:
                forms.add(encoded[skip:keep])
    forms.add("".join("%%%02X" % b for b in key).encode())
    forms.add("".join("%%%02x" % b for b in key).encode())
    return [f for f in forms if f]


class _Masker:
    def __init__(self, keys):
        self.forms = [(name, form) for name, value in keys.items() for form in _forms(value.encode())]
        windows = {f[i:i + min(FRAGMENT, len(f))] for _, f in self.forms for i in range(len(f) - min(FRAGMENT, len(f)) + 1)}
        pattern = b"|".join(re.escape(w) for w in sorted(windows, key=len, reverse=True))
        self.starts = re.compile(pattern) if windows else None
        self.held = b""

    def _within(self, data):
        return next((name for name, f in self.forms if data in f), None)

    def push(self, data, last=False):
        buf, self.held, out, i = self.held + data, b"", bytearray(), 0
        while self.starts is not None:
            m = self.starts.search(buf, i)
            if not m:
                break
            start, end = m.start(), m.end()
            while end < len(buf) and self._within(buf[start:end + 1]):
                end += 1
            if end == len(buf) and not last:
                out += buf[i:start]
                self.held = buf[start:]
                return bytes(out)
            label = self._within(buf[start:end]) or "A key"
            out += buf[i:start] + ("[KeyHandoff: %s hidden]" % label).encode()
            i = end
        hold = 0
        if not last:
            for k in range(min(FRAGMENT - 1, len(buf) - i), 0, -1):
                if self._within(buf[len(buf) - k:]):
                    hold = k
                    break
        out += buf[i:len(buf) - hold]
        self.held = buf[len(buf) - hold:]
        return bytes(out)


def _mask_output(keys):
    """Send fds 1 and 2 through a pipe and a masking thread, so os.write, C
    extensions and child processes are covered, not just print()."""
    for fd, stream in ((1, sys.stdout), (2, sys.stderr)):
        try:
            os.fstat(fd)
            if stream is not None:
                stream.flush()
        except (OSError, ValueError):
            continue
        original = os.dup(fd)
        read, write = os.pipe()
        os.dup2(write, fd)
        os.close(write)
        masker = _Masker(keys)

        def pump(read=read, original=original, masker=masker):
            while True:
                chunk = os.read(read, 65536)
                if not chunk:
                    break
                os.write(original, masker.push(chunk))
            os.write(original, masker.push(b"", last=True))
            os.close(read)

        thread = threading.Thread(target=pump, daemon=True)
        thread.start()

        def restore(fd=fd, stream=stream, original=original, thread=thread):
            try:
                if stream is not None:
                    stream.flush()
            except (OSError, ValueError):
                pass
            os.dup2(original, fd)  # closes this process's end of the pipe
            thread.join(2)

        atexit.register(restore)
