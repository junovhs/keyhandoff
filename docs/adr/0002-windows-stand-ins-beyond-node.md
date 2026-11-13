# ADR 0002: Stand-in keys on Windows beyond Node.js

**Status:** proposed, not built. Review before anyone starts.
**Date:** 2026-11-12

## Context

The plan's outcome needs stand-in keys "on Linux and Windows". As of FEAT-13, Windows hooks Node.js only, through the JavaScript hook. Every other Windows program under `keyhandoff run` gets the real key in its environment, with its printed copies masked.

The backlog called the missing piece "the schannel hook". But schannel is the wrong target for most developer tools on Windows:

| Program on Windows | TLS it uses | Hook that would reach it |
|---|---|---|
| Node.js | OpenSSL, built in | JavaScript hook. **Done** (FEAT-13). |
| Python (python.org, Store) | Its own `libssl-3.dll` | Python-level hook, or native OpenSSL hook |
| Ruby, PHP | Their own OpenSSL DLLs | Native OpenSSL hook |
| curl.exe (Windows' own) | schannel | schannel hook |
| .NET, PowerShell `Invoke-RestMethod` | schannel | schannel hook |
| Go, Rust (rustls), Java | Their own TLS | none (as on Linux) |

Your tp repos are Python and Node, so Python is next.

## Options

1. **A Python-level hook, like the Node one.** The runner sets `PYTHONPATH` to a folder holding `sitecustomize.py`. At startup it wraps `ssl.SSLSocket.send`/`sendall` and `ssl.SSLObject.write`, which covers requests, urllib3, http.client, httpx and asyncio. Before a write that holds a placeholder, it checks the peer certificate against the key's hosts itself, then asks KeyHandoff over the pipe. It's pure Python with no injection, the same design as `keyhandoff_node.cjs`, and works for any CPython 3.8+.
   *Gaps:* a program that sets `-I`/`-S` or clears `PYTHONPATH` skips the hook, and its stand-in then fails safe. C extensions that call OpenSSL directly (rare in API clients) aren't covered.
2. **A native OpenSSL hook by DLL injection.** Start the program suspended, inject a DLL, and patch `SSL_write`/`SSL_write_ex` in each `libssl-*.dll` it loads. This covers Python, Ruby and PHP at once. It needs injection and IAT/inline patching, and antivirus tools commonly flag that.
3. **A schannel hook.** Patch `EncryptMessage` in `sspicli.dll`/`secur32.dll` via injection. It covers curl.exe, .NET and PowerShell. Same injection costs as option 2, plus parsing SSPI buffers.

## Proposal

Build **option 1** next: small, testable on the existing coverage matrix, and it reaches the real need. Keep options 2 and 3 as later issues. Decide them on what people actually run, and on whether antivirus tolerates injection from an unsigned app. The answer may change once the app is Microsoft-signed.

## Consequences

- Python on Windows would get the same protection as on Linux, minus the `-I`/`-S` gap, which `threats.md` must state.
- On Linux, Python stays on the C hook. The Python hook is only loaded on Windows unless it proves better.
