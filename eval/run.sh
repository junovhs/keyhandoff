#!/bin/sh
# Run KeyHandoff's evaluation from a checkout: the coverage matrix, the
# attack replays and the hook's overhead benchmarks. Needs Rust, Node.js, Python 3, openssl, curl and setsid;
# Ruby, Go, requests and httpx add their rows when present. Results go to
# eval/results/. Everything stays on 127.0.0.1 with a synthetic key.
set -e
cd "$(dirname "$0")/../src-tauri"
cargo test --lib coverage_matrix -- --ignored --nocapture
cargo test --lib attack_replays -- --ignored --nocapture
cargo test --release --lib hook_benchmarks -- --ignored --nocapture
echo "Results: eval/results/coverage.md, replays.md, benchmarks.md"
