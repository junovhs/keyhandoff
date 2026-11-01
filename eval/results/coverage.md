# Coverage matrix

The same keyed request, under `keyhandoff run`, to a server verified as the key's host and to a stranger with a valid certificate for another name. **Protected**: the key's server got the key and the stranger got nothing. **Key safe, not protected**: the key never left, but the swap didn't happen (the key's server got the placeholder) or the stranger got the placeholder. Request times with and without the hook are in coverage.json; at this scale they're dominated by TCP timing (a ±40 ms delayed-ACK effect that comes and goes with how a client splits its writes), so the hook's own cost is measured separately (EVAL-03).

| Client | TLS | Protocol | Key's server got | Stranger got | Result |
|---|---|---|---|---|---|
| Python http.client | OpenSSL (system) | HTTP/1.1, header | key | nothing | protected (claimed) |
| Python requests | OpenSSL (system) | HTTP/1.1, header | key | nothing | protected (claimed) |
| Python httpx | OpenSSL (system) | HTTP/1.1, header | key | nothing | protected (claimed) |
| Python | OpenSSL (system) | HTTP/1.1, chunked body | key | nothing | protected (claimed) |
| Python | OpenSSL (system) | HTTP/1.1, gzip body | placeholder | placeholder | key safe, not protected |
| Python | OpenSSL (system) | WebSocket upgrade, header | key | nothing | protected (claimed) |
| Python | OpenSSL (system) | WebSocket message (masked) | placeholder | placeholder | key safe, not protected |
| curl | OpenSSL (system) | HTTP/1.1, header | key | nothing | protected (claimed) |
| curl | OpenSSL (system) | HTTP/1.1, Basic auth (base64) | key | nothing | protected (claimed) |
| curl | OpenSSL (system), nghttp2 | HTTP/2, header (HPACK) | key | nothing | protected (claimed) |
| curl | OpenSSL (system), nghttp2 | HTTP/2, body | key | the request, without key or placeholder | protected (claimed) |
| curl | none | plain HTTP to a stranger | - | nothing | protected (claimed) |
| Ruby Net::HTTP | OpenSSL (system) | HTTP/1.1, header | key | nothing | protected (claimed) |
| Node.js https | OpenSSL (built into Node) | HTTP/1.1, header | key | nothing | protected (claimed) |
| Go net/http | Go crypto/tls | HTTP/1.1, header | placeholder | placeholder | key safe, not protected |

Not in this run: Postgres and Redis over TLS (no `psql` or `redis-cli` here), Java (JSSE), Rust with rustls, HTTP/3 (this curl has no QUIC), Windows (schannel). 

Environment: {"curl":"curl 7.81.0 (x86_64-pc-linux-gnu) libcurl/7.81.0 OpenSSL/3.0.2 zlib/1.2.11 brotli/1.0.9 zstd/1.4.8 libidn2/2.3.2 libpsl/0.21.0 (+libidn2/2.3.2) libssh/0.9.6/openssl/zlib nghttp2/1.43.0 librtmp/2.3 OpenLDAP/2.5.20","go":"go version go1.27.3 linux/amd64","kernel":"7.1.1-76070101-generic","node":"v22.17.0","openssl":"OpenSSL 3.0.2 15 Mar 2022 (Library: OpenSSL 3.0.2 15 Mar 2022)","os":"Pop!_OS 22.04 LTS","python":"Python 3.10.12","ruby":"ruby 3.0.2p107 (2021-07-07 revision 0db68f0233) [x86_64-linux-gnu]"}

Regenerate: `cd src-tauri && cargo test --lib coverage_matrix -- --ignored --nocapture`.
