# Hook overhead

One keep-alive TLS connection to a local server (Python client, system OpenSSL), 2000 small requests after 50 warm-up ones, then a 64 MiB upload in 64 KiB writes. **Direct**: the key in the environment, no hook, started the same way (detached, through the runner, from a folder without pointers). **Hooked**: under `keyhandoff run`. **Keyed**: the key in an `Authorization` header, so under the hook every request is checked against the certificate and the key fetched from KeyHandoff and swapped in. **Plain**: no key, so the hook only looks. Each figure is the median over 9 alternating rounds, with the range across rounds.

| Mode | Median per request (µs) | p95 (µs) | Mean ± sd within a round (µs) | Upload (MiB/s) |
|---|---|---|---|---|
| direct keyed | 63 (49–64) | 74 | 64 ± 14 | 938 (797–1173) |
| direct plain | 61 (51–65) | 75 | 60 ± 12 | 941 (715–1259) |
| hooked keyed | 324 (285–357) | 413 | 335 ± 51 | 817 (663–939) |
| hooked plain | 63 (42–65) | 74 | 65 ± 8 | 914 (708–1118) |

Added per request: **262 µs** when the key is swapped in (keyed), **2 µs** when there's nothing to swap (plain). Upload throughput under the hook: -12.9% (keyed run), -2.9% (plain run).

Not compared here: `op run` and placeholder proxies (neither installed on this machine). Environment: {"build":"release","cpu":"AMD Ryzen 9 3900X 12-Core Processor","kernel":"7.1.1-76070101-generic","openssl":"OpenSSL 3.0.2 15 Mar 2022 (Library: OpenSSL 3.0.2 15 Mar 2022)","python":"Python 3.10.12"}

Regenerate: `cd src-tauri && cargo test --release --lib hook_benchmarks -- --ignored --nocapture`.
