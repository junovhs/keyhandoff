# Coverage matrix

The same keyed request, under `keyhandoff run`, to a server verified as the key's host and to a stranger with a valid certificate for another name. **Protected**: the key's server got the key and the stranger got nothing. **Key safe, not protected**: the key never left, but the swap didn't happen (the key's server got the placeholder) or the stranger got the placeholder. Request times with and without the hook are in coverage.json; at this scale they're dominated by TCP timing (a ±40 ms delayed-ACK effect that comes and goes with how a client splits its writes), so the hook's own cost is measured separately (EVAL-03).

| Client | TLS | Protocol | Key's server got | Stranger got | Result |
|---|---|---|---|---|---|
| Python http.client | OpenSSL (system) | HTTP/1.1, header | key | key | exposed: not hooked here (PAR-08) |
| Python requests | OpenSSL (system) | HTTP/1.1, header | key | key | exposed: not hooked here (PAR-08) |
| Python httpx | OpenSSL (system) | HTTP/1.1, header | key | key | exposed: not hooked here (PAR-08) |
| Python | OpenSSL (system) | HTTP/1.1, chunked body | key | key | exposed: not hooked here (PAR-08) |
| Python | OpenSSL (system) | HTTP/1.1, gzip body | key | key | exposed: not hooked here (PAR-08) |
| Python | OpenSSL (system) | WebSocket upgrade, header | key | key | exposed: not hooked here (PAR-08) |
| Python | OpenSSL (system) | WebSocket message (masked) | key | key | exposed: not hooked here (PAR-08) |
| curl | OpenSSL (system) | HTTP/1.1, header | nothing | nothing | key safe, not protected |
| curl | OpenSSL (system) | HTTP/1.1, Basic auth (base64) | nothing | nothing | key safe, not protected |
| curl | OpenSSL (system), nghttp2 | HTTP/2, header (HPACK) | nothing | nothing | key safe, not protected |
| curl | OpenSSL (system), nghttp2 | HTTP/2, body | nothing | nothing | key safe, not protected |
| curl | none | plain HTTP to a stranger | - | key | exposed: not hooked here (PAR-08) |
| Ruby Net::HTTP | OpenSSL (system) | HTTP/1.1, header | - | - | not run: not installed here |
| Node.js https | OpenSSL (built into Node) | HTTP/1.1, header | key | nothing | protected (claimed) |
| Go net/http | Go crypto/tls | HTTP/1.1, header | - | - | not run: not installed here |

On Windows only Node.js is hooked so far, so every other program gets the real key under `keyhandoff run`, with its printed copies masked (PAR-08). **Exposed** cells show that: a stranger that program sends the key to gets it.

Not in this run: Postgres and Redis over TLS (no `psql` or `redis-cli` here), Java (JSSE), Rust with rustls, HTTP/3 (this curl has no QUIC), schannel (curl.exe, .NET). 

Environment: {"curl":"curl 8.23.0 (Windows) libcurl/8.23.0 Schannel zlib/1.3.2 WinIDN WinLDAP","go":"go version go1.26.1 windows/amd64","kernel":"3.5.7-463ebcdc.x86_64","node":"v24.11.1","openssl":"OpenSSL 3.2.4 11 Feb 2025 (Library: OpenSSL 3.2.4 11 Feb 2025)","os":null,"python":"Python 3.12.10","ruby":""}

Regenerate: `cd src-tauri && cargo test --lib coverage_matrix -- --ignored --nocapture`.
