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

## Keep times

When you accept a key you can keep it **once**, for **30 days**, or until you remove it. Your last choice becomes the default.

## In progress

- **Using a key in a field:** typing a kept key into the box you click, without using the clipboard. Confirmed password boxes get it directly, and anything else warns first.
- **Using a key in a program:** a `.env` file holds a pointer such as `DIALPAD_API_KEY=handoff:Dialpad`, and approved programs ask the running app for the value.

## Who controls what

- The **sender** keeps nothing after sealing.
- The **receiver** chooses how long to keep the key.
- The sender can't control what the receiver does with a key afterwards, and the app doesn't pretend otherwise.

## Limits

- **Before the copy.** Provider pages show the key to you before you copy it. That's outside what KeyHandoff can affect.
- **The copy moment.** A copied key is on the clipboard, unencrypted, until the wipe. Another program watching the clipboard could read it.
- **First contact trusts the channel.** If someone can post as you in a chat, they can request a key in your name. Check the name and fingerprint before you copy a key.
- **Malware running as you.** See the [README](../README.md#what-it-protects-and-what-it-doesnt).
- **No central revocation.** Replacing the key with the provider is the real fix.

## Not goals

Background clipboard monitoring, links or a website, servers or accounts, a phone app, or being a general password manager.
