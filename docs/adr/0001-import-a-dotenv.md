# ADR 0001: Import a .env file

**Status:** proposed, built (FEAT-12). Review it. If you want it gone, revert the FEAT-12 commits.
**Date:** 2026-11-12

## Context

Until now the only way to add your own key was Safe Copy: one clipboard round per key. Real projects already hold plaintext keys in files. tp-dialpad's `.env`, for example, has seven. So moving in is slow, and every key passes through the clipboard on the way. On a locked or remote Windows session the clipboard isn't usable at all.

## Decision

**Import .env** (a tile on the empty home, a link under the main buttons once you have keys):

1. You pick a file with the system picker. KeyHandoff never goes looking for files.
2. Rust reads it. The screen shows **variable names only**. Likely keys start ticked: the name contains KEY, TOKEN, SECRET, PASSWORD, AUTH and the like, and the value isn't a plain URL. Values that match a key you already keep are tagged.
3. **Seal N keys** puts the ticked values in the vault, reusing any key you already keep with that exact value, and saves the vault. Then it rewrites only those lines as `handoff:` pointers. Every other byte stays: comments, other variables, quotes, CRLF.
4. If the file changed after you opened it, nothing is rewritten.
5. Keep: 30 days or forever. Once isn't offered, because an imported project key is meant to keep working.
6. Labels are the variable names, so `DIALPAD_API_KEY=handoff:DIALPAD_API_KEY`.

## Why

- Rule 1: keys leave plaintext on disk in one step.
- Rule 9: no clipboard at all, strictly better than copying each key.
- Rule 8: values never reach TypeScript. The view carries names, and a test checks the serialized view holds no value.

## Alternatives rejected

- **Safe Copy only.** Slower, and worse for keys that are already in files.
- **Scan projects for .env files.** KeyHandoff shouldn't search through folders.
- **Readable labels ("Dialpad API key").** The variable name is what code uses and stays predictable. Easy to change later.
- **Keep a backup of the original .env.** The backup would be the very plaintext we're removing. The vault is saved before the rewrite instead, so a failed rewrite loses nothing.

## Consequences

- Old copies of the file still hold the keys: git history, backups, editor undo. Rotate any key that was ever committed.
- Rule 2 ("one flow") gains a second way in for your own keys. If you accept this, update the rule's wording.

Test: `importing_a_dotenv_seals_its_keys_and_leaves_pointers` (core.rs) and `envfile::tests`. Also run by hand on the real Windows app (see HAND-19's comment).
