# How KeyHandoff works

## The idea

Encrypting a key at rest is the easy part. The real exposure comes from people handling it: looking at it, copying it, pasting it into a chat, saving it in a `.env` file. Every one of those is a place it can leak.

KeyHandoff keeps custody of the key from the provider's page onward, so people and their tools handle it as little as possible. Encryption is the protection. Sealed files and the vault are ordinary, clearly named files, with nothing disguised.

## What you handle

| Thing | What it is | Secret inside? |
|---|---|---|
| **Request file** (`Key request from Ann.handoff`) | "Ann is asking for a key." Holds Ann's public identity and a fresh one-time lock. | No |
| **Sealed key file** (`Key for Ann.handoff`) | The key, locked so only Ann's computer can open it, and only once. | Yes, encrypted |
| **Kept key** | An entry in your vault: its name and source, never its value. | Stored encrypted |

Both files contain damage-resistant text, so they can also be pasted as text when a channel can't carry files.

## Getting a key from someone

1. **Ann clicks Request a key.** A request file goes on the clipboard, ready to paste into a chat.
2. **Ben opens the file.** KeyHandoff shows who is asking and their computer's fingerprint, and Safe Copy turns on.
3. **Ben copies the key** on the provider's page. KeyHandoff catches it, wipes the clipboard, and removes the entry from clipboard history where the system allows it.
4. **Ben names the key.** It is sealed for Ann, and Ben sends back the sealed file. Ben's computer keeps nothing.
5. **Ann opens the file.** The key goes into Ann's vault, and the copy in the chat can never be opened again.

## Adding a key for yourself

Click **Add your own**, copy the key on the provider's page, and name it. No files are involved.

## Safe Copy

Safe Copy reads the clipboard only after you ask it to. It catches one item, then switches off, or times out after 60 seconds. Nothing watches the clipboard the rest of the time.

When it turns on, it checks the running programs against a short list of known clipboard managers, such as Ditto or CopyQ, and names any it finds, since they can read the key when you copy it. Clipboard extensions built into a desktop shell don't show up as programs, so an empty list isn't a promise.

## Keep times

When you accept a key you can keep it **once**, for **30 days**, or until you remove it. Your last choice becomes the default.

## Using a key in a program

1. **Point at the key.** Your `.env` holds `DIALPAD_API_KEY=handoff:Dialpad` instead of the key. The name matches a kept key, ignoring case. Names are unique, so a pointer always means one key.
2. **Load it.** The program calls a one-line loader: `keyhandoff_env.load()` for Python, or `await load()` from `keyhandoff_env.mjs` for Node.js. The loader runs inside the program and sets the variable there, rather than launching the program with the key, because a program's starting environment can be read by other programs running as you. It asks the running app for every pointed-to key at once, over a local socket (a named pipe on Windows).
3. **Allow it.** KeyHandoff asks the operating system which program is asking and which folder it runs in. The first time, the window shows both, and you choose **Always allow**, **Just this once** or **Deny**. Always allow covers that program, in that folder, for those keys. The same program in another folder asks again, because allowing `python` everywhere would allow every script.
4. **Use it.** The value goes into the program's own environment. Nothing is written to disk, and a key kept **once** is destroyed after this first use.

## In progress

- **Using a key in a field:** typing a kept key into the box you click, without using the clipboard. Confirmed password boxes get it directly, and anything else warns first.

## Who controls what

- The **sender** keeps nothing after sealing.
- The **receiver** chooses how long to keep the key.
- The sender can't control what the receiver does with a key afterwards, and the app doesn't pretend otherwise.

## Limits

- **Before the copy.** Provider pages show the key to you before you copy it. That's outside what KeyHandoff can affect.
- **The copy moment.** A copied key is on the clipboard, unencrypted, until the wipe. Another program watching the clipboard could read it, and on Linux (X11) one that watches always does ([capture tests](capture-tests.md)).
- **First contact trusts the channel.** If someone can post as you in a chat, they can request a key in your name. Check the name and fingerprint before you copy a key.
- **A program you allowed** can do anything with the key once it has it. Anything running as you could also start that program in that folder, so the prompt is your decision, not a lock.
- **Malware running as you.** See [threats.md](threats.md).
- **No central revocation.** Replacing the key with the provider is the real fix.

## Not goals

Background clipboard monitoring, links or a website, servers or accounts, a phone app, or being a general password manager.
