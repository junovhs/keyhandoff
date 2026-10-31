# Capture tests: the copy moment

When you copy a key, it sits on the system clipboard until Safe Copy takes it. These tests measure how often another program, reading the clipboard as fast as it can, sees the key in that moment. The answer sets what KeyHandoff may say publicly about the copy moment ([threats.md](threats.md#the-copy-moment), rule 9). Fake keys only.

## How it's measured

- **The reader** is [`examples/clipboard_reader.rs`](../src-tauri/examples/clipboard_reader.rs), a separate process. It reacts to every clipboard change at once and prints each fake key it reads. On X11 it blocks on XFixes owner-change events and asks the new owner for the text. On Windows it uses a clipboard format listener and reads `CF_UNICODETEXT` on `WM_CLIPBOARDUPDATE`.
- **The run** arms the real Safe Copy capture, copies `sk-synthetic-race-NNNN` from another process, waits for the catch, and repeats. At the end it counts how many keys the reader printed. The run fails if Safe Copy misses any copy, so the count always compares against copies Safe Copy really caught.
- **Linux:** [`x11_fast_reader_race`](../src-tauri/src/os/linux/clipboard.rs) copies with `xclip`.
- **Windows:** [`fast_reader_race`](../src-tauri/src/os/windows/capture.rs) copies through the clipboard API.

```bash
cd src-tauri && cargo test fast_reader_race -- --ignored --test-threads=1 --nocapture
```

Set `CLIPBOARD_RACE_COPIES` for a shorter run.

## Results

| Date | System | Reader | Copies caught | Reader saw | Copy to catch |
|---|---|---|---|---|---|
| 2026-10-30 | Linux X11 (X.Org 11.0), Ryzen 9 3900X, no clipboard manager running | `clipboard_reader` | 1,000 | **1,000** | p50 40.7 ms, max 41.4 ms (includes starting `xclip`) |
| not run yet | Windows 11, clipboard history on | `clipboard_reader` | | | |
| not run yet | Windows 11, clipboard history on | Ditto | | | |
| not run yet | Windows 11, listening thread priority raised while armed | `clipboard_reader` | | | |

## What the Linux result means

**On X11, a program watching the clipboard sees every key you copy.** That isn't a matter of speed. X11 has no clipboard storage. The program you copied from keeps the text and hands it to every program that asks, until something else takes the clipboard. XFixes tells every listener about the copy at the same moment, so a listener asks for the text alongside Safe Copy, and the source answers both. Making Safe Copy faster wouldn't change this.

So on Linux the honest statement is: a program that watches the clipboard will get the key when you copy it. Safe Copy limits what happens after: nothing can paste it later, and it isn't left on the clipboard. It can't keep a watcher from reading the copy itself.

## Still to run on Windows

- The `clipboard_reader` row, with clipboard history on.
- Ditto, a common clipboard manager: install it, copy with Safe Copy armed, and count how many fake keys appear in Ditto's history.
- Raising the listening thread's priority while armed. Keep the change only if it lowers the count.

Windows keeps the clipboard's data in the system, and the copying app's write and the listeners' notifications happen in order. Safe Copy empties the clipboard as soon as it reads it, but a listener notified at the same time can read first. Expect a nonzero count until measured.
