# Coverage matrix

The same keyed request, under `keyhandoff run`, to a server verified as the key's host and to a stranger with a valid certificate for another name. **Protected**: the key's server got the key and the stranger got nothing. **Key safe, not protected**: the key never left, but the swap didn't happen (the key's server got the placeholder) or the stranger got the placeholder. Request times with and without the hook are in coverage.json; at this scale they're dominated by TCP timing (a ±40 ms delayed-ACK effect that comes and goes with how a client splits its writes), so the hook's own cost is measured separately (EVAL-03).

| Client | TLS | Protocol | Key's server got | Stranger got | Result |
|---|---|---|---|---|---|
| Python http.client | OpenSSL (system) | HTTP/1.1, header | key | key | exposed: not hooked here (ISS-13) |
| Python requests | OpenSSL (system) | HTTP/1.1, header | - | - | not run: not installed here |
| Python httpx | OpenSSL (system) | HTTP/1.1, header | - | - | not run: not installed here |
| Python | OpenSSL (system) | HTTP/1.1, chunked body | key | key | exposed: not hooked here (ISS-13) |
| Python | OpenSSL (system) | HTTP/1.1, gzip body | key | key | exposed: not hooked here (ISS-13) |
| Python | OpenSSL (system) | WebSocket upgrade, header | key | key | exposed: not hooked here (ISS-13) |
| Python | OpenSSL (system) | WebSocket message (masked) | key | key | exposed: not hooked here (ISS-13) |
| curl | OpenSSL (system) | HTTP/1.1, header | key | key | exposed: not hooked here (ISS-13) |
| curl | OpenSSL (system) | HTTP/1.1, Basic auth (base64) | key | key | exposed: not hooked here (ISS-13) |
| curl | OpenSSL (system), nghttp2 | HTTP/2, header (HPACK) | key | key | exposed: not hooked here (ISS-13) |
| curl | OpenSSL (system), nghttp2 | HTTP/2, body | key | key | exposed: not hooked here (ISS-13) |
| curl | none | plain HTTP to a stranger | - | key | exposed: not hooked here (ISS-13) |
| Ruby Net::HTTP | OpenSSL (system) | HTTP/1.1, header | nothing | nothing | not run: the client couldn't connect here |
| Node.js https | OpenSSL (built into Node) | HTTP/1.1, header | key | nothing | protected (claimed) |
| Go net/http | Go crypto/tls | HTTP/1.1, header | - | - | not run: not installed here |

On macOS only Node.js is hooked so far: System Integrity Protection strips `DYLD_INSERT_LIBRARIES` from Apple's own programs (python3, curl, ruby), so the TLS hook can't be a promise there, and every other program gets the real key under `keyhandoff run`, with its printed copies masked (ISS-13). **Exposed** cells show that: a stranger that program sends the key to gets it.

Not in this run: Postgres and Redis over TLS (no `psql` or `redis-cli` here), Java (JSSE), Rust with rustls, HTTP/3 (this curl has no QUIC), schannel (curl.exe, .NET). 

Environment: {"curl":"curl 8.7.1 (x86_64-apple-darwin21.0) libcurl/8.7.1 (SecureTransport) LibreSSL/3.3.6 zlib/1.2.11 nghttp2/1.45.1","go":"","kernel":"21.6.0","node":"v22.23.5","openssl":"LibreSSL 2.8.3","os":"macOS 12.7.6","python":"Python 3.8.2","ruby":"ruby 2.6.10p210 (2022-04-12 revision 67958) [universal.x86_64-darwin21]"}

Regenerate: `cd src-tauri && cargo test --lib coverage_matrix -- --ignored --nocapture`.
