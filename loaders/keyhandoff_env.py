"""Fill environment variables from KeyHandoff pointers in .env.

A .env line such as ``DIALPAD_API_KEY=handoff:Dialpad`` names a key kept in
KeyHandoff instead of holding it. ``load()`` asks the running app for every
pointed-to key at once. The first time a program asks from a folder,
KeyHandoff asks you to allow it. If KeyHandoff is locked, it comes forward
and this waits for you to unlock it. The values are set in ``os.environ`` for
this process only; nothing is written to disk.

    import keyhandoff_env; keyhandoff_env.load()

Standard library only: copy this file next to your script. Python strings
can't be wiped, so the value stays in this process's memory until it exits.
"""

import json
import os
import socket
import tempfile

__all__ = ["load", "KeyHandoffError"]

PREFIX = "handoff:"
NOT_RUNNING = "KeyHandoff isn't running. Open it and try again."


class KeyHandoffError(RuntimeError):
    pass


def load(path=".env", profile=None):
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
