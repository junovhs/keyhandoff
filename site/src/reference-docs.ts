import type { DocReference } from "dopedocs";

const GITHUB = "https://github.com/junovhs/keyhandoff";

/** KeyHandoff's developer reference: what a developer's code calls, sends or runs. */
export const reference: DocReference = {
  id: "reference",
  title: "Developer reference",
  question: "How do I use KeyHandoff from my own code?",
  answer: "KeyHandoff gives code three ways in: a one-line loader for Python and Node.js, a local socket any language can speak, and keyhandoff-seal, the Rust crate that makes and opens .handoff files. Everything runs on your computer.",
  blocks: [
    { kind: "p", text: "Your program never reads a key from a file. A `.env` line points at a kept key, the loader asks the running app over a private local socket, and the app answers once you've allowed that program in that folder. See the [user manual](#env-pointers) for the setup a person does." },
    { kind: "p", text: "Nothing here makes a network request. There are no API keys for KeyHandoff itself, no accounts and no servers." },
  ],
  code: [
    {
      title: "Quick start",
      samples: [
        { label: "Python", language: "python", code: "# .env\n# DIALPAD_API_KEY=handoff:Dialpad\n\nimport os\nimport keyhandoff_env\n\nkeyhandoff_env.load()\nprint(len(os.environ[\"DIALPAD_API_KEY\"]))\n" },
        { label: "Node.js", language: "js", code: "// .env\n// DIALPAD_API_KEY=handoff:Dialpad\n\nimport { load } from \"./keyhandoff_env.mjs\";\n\nawait load();\nconsole.log(process.env.DIALPAD_API_KEY.length);\n" },
        { label: "Shell", language: "sh", code: "printf '{\"names\":[\"Dialpad\"]}\\n' \\\n  | socat - UNIX-CONNECT:\"$XDG_RUNTIME_DIR/handoff-default-keys.sock\"\n" },
      ],
    },
  ],
  groups: [
    {
      id: "loaders",
      title: "Loaders",
      entries: [
        {
          id: "py-load",
          title: "Python: load()",
          badge: "py",
          signature: "keyhandoff_env.load(path=\".env\", profile=None) -> list[str]",
          question: "How does the KeyHandoff Python loader work?",
          answer: "keyhandoff_env.load() finds every handoff: pointer in the environment and in the .env file, asks the running KeyHandoff app for all of them in one request, and sets the variables in os.environ for this process. It returns the names it set.",
          guide: "loaders",
          blocks: [
            { kind: "p", text: "Copy [keyhandoff_env.py](" + GITHUB + "/blob/main/loaders/keyhandoff_env.py) next to your script. It uses the standard library only. Call it once at startup, after `load_dotenv()` if you use python-dotenv." },
            { kind: "p", text: "**Which value wins:** a variable already set to a real value is left alone. A variable set to a `handoff:` pointer, in the environment or in `.env`, is filled. A pointer in `.env` is used only when the variable isn't set, or is itself a pointer." },
          ],
          fields: [
            { title: "Parameters", fields: [
              { name: "path", type: "str", default: "\".env\"", description: "The .env file to read pointers from. A missing file isn't an error; only pointers already in the environment are used." },
              { name: "profile", type: "str | None", default: "None", description: "The KeyHandoff profile to ask. `None` uses `KEYHANDOFF_PROFILE`, then `default`." },
            ] },
            { title: "Returns", fields: [
              { name: "list[str]", description: "The names of the variables it set, sorted. Empty when there were no pointers, in which case it doesn't contact the app." },
            ] },
          ],
          errors: [
            { code: "KeyHandoffError", description: "Raised with the app's message: not allowed, no such key, locked, or busy. See [the reply](#socket-reply)." },
            { code: "KeyHandoff isn't running. Open it and try again.", description: "No socket or pipe for that profile." },
            { code: "KeyHandoff closed the connection.", description: "The app quit or refused mid-request." },
          ],
          code: [
            {
              title: "Example",
              samples: [
                { label: "Python", language: "python", code: "import os\nfrom dotenv import load_dotenv\nimport keyhandoff_env\n\nload_dotenv()                     # your usual .env loader\nset_vars = keyhandoff_env.load()  # fills handoff: pointers\n\nclient = Dialpad(api_key=os.environ[\"DIALPAD_API_KEY\"])\n" },
              ],
            },
            {
              title: "Handling errors",
              samples: [
                { label: "Python", language: "python", code: "import sys\nimport keyhandoff_env\n\ntry:\n    keyhandoff_env.load(profile=\"work\")\nexcept keyhandoff_env.KeyHandoffError as e:\n    sys.exit(f\"Couldn't get keys: {e}\")\n" },
              ],
            },
          ],
        },
        {
          id: "js-load",
          title: "Node.js: load()",
          badge: "js",
          signature: "await load(path = \".env\", profile = undefined): Promise<string[]>",
          question: "How does the KeyHandoff Node.js loader work?",
          answer: "The KeyHandoff Node.js loader's load() reads handoff: pointers from process.env and the .env file, asks the running app for all of them at once, and sets them in process.env. It resolves to the names it set and has no dependencies.",
          guide: "loaders",
          blocks: [
            { kind: "p", text: "Copy [keyhandoff_env.mjs](" + GITHUB + "/blob/main/loaders/keyhandoff_env.mjs) next to your script and import it as an ES module. The rules for which value wins are the same as [Python's](#py-load)." },
          ],
          fields: [
            { title: "Parameters", fields: [
              { name: "path", type: "string", default: "\".env\"", description: "The .env file to read. A missing file isn't an error." },
              { name: "profile", type: "string | undefined", default: "undefined", description: "The profile to ask; falls back to `KEYHANDOFF_PROFILE`, then `default`." },
            ] },
            { title: "Returns", fields: [
              { name: "Promise<string[]>", description: "The variable names it set, sorted." },
            ] },
          ],
          errors: [
            { code: "KeyHandoffError", description: "Rejected with the app's message, or \"KeyHandoff isn't running. Open it and try again.\"" },
          ],
          code: [
            {
              title: "Example",
              samples: [
                { label: "Node.js", language: "js", code: "import \"dotenv/config\";\nimport { load, KeyHandoffError } from \"./keyhandoff_env.mjs\";\n\ntry {\n  await load();\n} catch (e) {\n  if (e instanceof KeyHandoffError) {\n    console.error(`Couldn't get keys: ${e.message}`);\n    process.exit(1);\n  }\n  throw e;\n}\n" },
              ],
            },
          ],
        },
        {
          id: "pointer-syntax",
          title: "Pointer syntax",
          badge: "env",
          signature: "NAME=handoff:<key name>",
          question: "What is the syntax of a KeyHandoff .env pointer?",
          answer: "A KeyHandoff pointer is a .env value that starts with handoff: followed by a kept key's name, like DIALPAD_API_KEY=handoff:Dialpad. The name matches a kept key ignoring case; spaces around it are trimmed.",
          guide: "env-pointers",
          blocks: [
            { kind: "list", items: [
              "Blank lines and lines starting with `#` are skipped; an `export ` prefix is allowed.",
              "A value in matching single or double quotes is unquoted. Otherwise ` #` starts an inline comment.",
              "Key names are unique in a vault, so a pointer always means one key. **Copy .env line** on a key's row writes a correct line for you.",
              "A pointer can also come from the environment itself, for example set by a process manager.",
            ] },
          ],
          code: [
            {
              title: ".env",
              samples: [
                { label: ".env", language: "sh", code: "# Kept in KeyHandoff, not in this file\nDIALPAD_API_KEY=handoff:Dialpad\nexport OPENAI_API_KEY=\"handoff:OpenAI\"\nSTRIPE_SECRET_KEY=handoff:Stripe (billing)   # spaces in names are fine\n\n# Ordinary values are left alone\nLOG_LEVEL=debug\n" },
              ],
            },
          ],
        },
      ],
    },
    {
      id: "socket",
      title: "Key socket",
      entries: [
        {
          id: "socket-endpoint",
          title: "Where to connect",
          badge: "socket",
          signature: "$XDG_RUNTIME_DIR/handoff-<profile>-keys.sock",
          question: "Where does KeyHandoff listen for key requests?",
          answer: "KeyHandoff listens on a Unix socket on Linux, handoff-<profile>-keys.sock in the runtime directory, and on a local named pipe on Windows, \\\\.\\pipe\\handoff-<user>-<profile>-keys. Only programs running as you can connect.",
          blocks: [
            { kind: "table", head: ["", "Linux", "Windows"], rows: [
              ["Path", "`$XDG_RUNTIME_DIR/handoff-<profile>-keys.sock`, or the temp folder without one", "`\\\\.\\pipe\\handoff-<user>-<profile>-keys`, where user is `%USERNAME%` with only letters and digits kept"],
              ["Access", "Mode `0600`; the peer must be the same user", "Remote clients are rejected"],
            ] },
            { kind: "p", text: "The socket is up whenever the app runs, locked or not. To support a language without a loader, connect, send one [request](#socket-request) line and read one [reply](#socket-reply) line." },
          ],
          code: [
            {
              title: "Minimal client",
              samples: [
                { label: "Python", language: "python", code: "import json, os, socket\n\npath = os.path.join(os.environ[\"XDG_RUNTIME_DIR\"], \"handoff-default-keys.sock\")\nwith socket.socket(socket.AF_UNIX) as s:\n    s.connect(path)\n    s.sendall(b'{\"names\": [\"Dialpad\"]}\\n')\n    reply = json.loads(s.makefile().readline())\n" },
                { label: "Go", language: "go", code: "conn, err := net.Dial(\"unix\",\n\tfilepath.Join(os.Getenv(\"XDG_RUNTIME_DIR\"), \"handoff-default-keys.sock\"))\nif err != nil {\n\treturn err // KeyHandoff isn't running\n}\ndefer conn.Close()\nfmt.Fprintln(conn, `{\"names\": [\"Dialpad\"]}`)\nline, err := bufio.NewReader(conn).ReadBytes('\\n')\n" },
                { label: "Rust", language: "rust", code: "use std::io::{BufRead, BufReader, Write};\nuse std::os::unix::net::UnixStream;\n\nlet dir = std::env::var(\"XDG_RUNTIME_DIR\")?;\nlet mut s = UnixStream::connect(format!(\"{dir}/handoff-default-keys.sock\"))?;\ns.write_all(b\"{\\\"names\\\": [\\\"Dialpad\\\"]}\\n\")?;\nlet mut line = String::new();\nBufReader::new(s).read_line(&mut line)?;\n" },
                { label: "Shell", language: "sh", code: "printf '{\"names\":[\"Dialpad\"]}\\n' \\\n  | socat - UNIX-CONNECT:\"$XDG_RUNTIME_DIR/handoff-default-keys.sock\"\n" },
              ],
            },
          ],
        },
        {
          id: "socket-request",
          title: "The request",
          badge: "msg",
          signature: "{\"names\": [string, ...]}\\n",
          question: "What does a program send to KeyHandoff to get keys?",
          answer: "A program sends KeyHandoff one line of JSON with the names of the kept keys it wants, {\"names\": [\"Dialpad\"]}, ending in a newline. Names may be bare or whole pointers, and one request asks for up to 64 keys.",
          fields: [
            { title: "Fields", fields: [
              { name: "names", type: "string[]", required: true, description: "1 to 64 kept-key names. `handoff:` is stripped and spaces trimmed, so `handoff:Dialpad` and `Dialpad` are the same." },
            ] },
          ],
          blocks: [
            { kind: "p", text: "One line, at most 16 KiB. Anything that isn't one JSON object with 1 to 64 names gets an error reply." },
          ],
          code: [
            { title: "Request", samples: [{ label: "JSON", language: "json", code: "{\"names\": [\"Dialpad\", \"handoff:OpenAI\"]}\n" }] },
          ],
        },
        {
          id: "socket-reply",
          title: "The reply",
          badge: "msg",
          signature: "{\"values\": {name: string}} | {\"error\": string}",
          question: "What does KeyHandoff send back to a program?",
          answer: "KeyHandoff answers a key request with one line of JSON: either values, mapping each requested name to its key, or error, a sentence to show the person. It's all or nothing; a request never gets some of its keys.",
          guide: "allowing-programs",
          blocks: [
            { kind: "p", text: "**Waiting:** if any key isn't yet allowed for this program in this folder, the window asks and the reply waits up to 60 seconds. If KeyHandoff is locked, the window comes forward and the reply waits up to two minutes for the unlock. Read with no timeout shorter than that." },
            { kind: "p", text: "A key kept **Once** is destroyed after the reply that carries it. The reply is written from a buffer KeyHandoff zeroes afterwards; what your program does with the value is up to it." },
          ],
          errors: [
            { code: "No kept key is named X.", description: "Check the name, or use Copy .env line." },
            { code: "Not allowed in KeyHandoff.", description: "The person chose Deny, or nobody answered within 60 seconds." },
            { code: "KeyHandoff is asking about another program. Try again in a moment.", description: "One program waits for an answer at a time." },
            { code: "KeyHandoff is locked. Unlock it and try again.", description: "Nobody unlocked within two minutes." },
            { code: "X isn't text, so it can't go in an environment variable.", description: "The key isn't valid UTF-8." },
            { code: "KeyHandoff couldn't read the request.", description: "Not one line of JSON with 1 to 64 names." },
          ],
          code: [
            { title: "Reply", samples: [
              { label: "Values", language: "json", code: "{\"values\": {\"Dialpad\": \"dp_…\", \"OpenAI\": \"sk-proj-…\"}}\n" },
              { label: "Error", language: "json", code: "{\"error\": \"Not allowed in KeyHandoff.\"}\n" },
            ] },
          ],
        },
        {
          id: "socket-caller",
          title: "Who is asking",
          badge: "os",
          signature: "(program path, working folder, key) → allowed",
          question: "How does KeyHandoff know which program is asking for a key?",
          answer: "KeyHandoff asks the operating system for the connecting process, then reads its executable path and working folder. An approval is that program path, that folder and one key; the command line is shown only so a person recognises the script.",
          blocks: [
            { kind: "table", head: ["", "Linux", "Windows"], rows: [
              ["Process", "`SO_PEERCRED` on the socket", "`GetNamedPipeClientProcessId`"],
              ["Program", "`/proc/<pid>/exe`", "The process image path"],
              ["Folder", "`/proc/<pid>/cwd`", "The current directory in the process's PEB"],
            ] },
            { kind: "callout", tone: "warn", text: "An approval is a decision, not a lock: any program running as you can start the same interpreter in the same folder and match it." },
          ],
          code: [
            { title: "What the window shows", samples: [{ label: "Prompt", language: "text", code: "python3 wants Dialpad\n\n  /usr/bin/python3\n  python3 dialpad_report.py\n  ~/code/report-bot\n\n[ Deny ]  [ Just once ]  [ Always allow ]\n" }] },
          ],
        },
      ],
    },
    {
      id: "app",
      title: "Command line",
      entries: [
        {
          id: "cli",
          title: "keyhandoff",
          badge: "cli",
          signature: "keyhandoff [--profile NAME] [FILE...]",
          question: "What are KeyHandoff's command-line options?",
          answer: "keyhandoff takes an optional --profile NAME and any number of .handoff files to open. If KeyHandoff is already running for that profile, the files go to the running window instead of starting a second copy.",
          guide: "install",
          fields: [
            { title: "Options", fields: [
              { name: "--profile NAME", type: "a-z 0-9 - _, 1 to 32", default: "default", description: "Which identity and vault to use. `--profile=NAME` works too. Each profile is a separate person to everyone else." },
              { name: "FILE", type: "path", description: "Cards and sealed key files to open. Double-clicking a `.handoff` file passes it here." },
            ] },
            { title: "Environment", fields: [
              { name: "KEYHANDOFF_PROFILE", type: "string", description: "The profile when `--profile` isn't given. The loaders read it too." },
              { name: "KEYHANDOFF_SWTPM", type: "port", description: "Debug builds only: use the swtpm software TPM on this TCP port for the PIN." },
            ] },
          ],
          errors: [
            { code: "exit 2", description: "A bad argument, such as `--profile` without a name or a name with other characters." },
          ],
          code: [
            { title: "Examples", samples: [{ label: "Shell", language: "sh", code: "keyhandoff                          # the default profile\nkeyhandoff --profile ben            # a second identity\nKEYHANDOFF_PROFILE=ben keyhandoff\nkeyhandoff \"Key for Ann.handoff\"    # open a file\n" }] },
          ],
        },
        {
          id: "profile-files",
          title: "Profile folder",
          badge: "dir",
          signature: "<data dir>/handoff/<profile>/",
          question: "Where does KeyHandoff keep its files?",
          answer: "KeyHandoff keeps each profile in a handoff folder under the system's data directory: ~/.local/share/handoff/<profile> on Linux, %APPDATA%\\handoff\\<profile> on Windows. Every file except settings.json is encrypted.",
          blocks: [
            { kind: "table", head: ["File", "Holds"], rows: [
              ["keyslots.bin", "The master key, wrapped by the passphrase and by the recovery code."],
              ["keyslot-tpm.bin", "The master key sealed in the TPM, when a PIN is set."],
              ["identity.bin", "This computer's signing key."],
              ["pending.bin", "The one-time locks of your cards, until used."],
              ["vault.bin", "Kept keys, each sealed again on its own."],
              ["contacts.bin, teams.bin, approvals.bin", "People, teams and allowed programs."],
              ["settings.json", "Plain JSON, no secrets."],
            ] },
            { kind: "p", text: "Back up the whole folder together; the files only open with the passphrase or recovery code. See [key storage](" + GITHUB + "/blob/main/docs/sealing-format.md#device-key-storage) for the layouts." },
          ],
          fields: [
            { title: "settings.json", fields: [
              { name: "name", type: "string", description: "Your name, shown on your cards." },
              { name: "keep", type: "\"once\" | \"days30\" | \"keep\"", description: "The keep time last chosen; the next default." },
              { name: "protection", type: "string", description: "`passphrase` once the profile has key slots." },
              { name: "remembered", type: "string | null", description: "The OS backend holding a copy of the master key, when Remember on this computer is on." },
              { name: "lock_after_minutes", type: "number | null", default: "null", description: "Lock after this many minutes without use. Off by default; set from Lock after under Security." },
            ] },
          ],
          code: [
            { title: "A profile", samples: [
              { label: "Folder", language: "text", code: "~/.local/share/handoff/default/\n├── keyslots.bin\n├── identity.bin\n├── pending.bin\n├── vault.bin\n├── contacts.bin\n├── teams.bin\n├── approvals.bin\n└── settings.json\n" },
              { label: "settings.json", language: "json", code: "{\n  \"name\": \"Ann\",\n  \"keep\": \"keep\",\n  \"protection\": \"passphrase\",\n  \"remembered\": null,\n  \"lock_after_minutes\": 15\n}\n" },
            ] },
          ],
        },
      ],
    },
    {
      id: "seal",
      title: "Seal crate (Rust)",
      entries: [
        {
          id: "seal-install",
          title: "Add the crate",
          badge: "rs",
          signature: "keyhandoff-seal = { git = \"" + GITHUB + "\" }",
          question: "How do I use KeyHandoff's sealing library in Rust?",
          answer: "keyhandoff-seal is KeyHandoff's sealing library: identities, cards, sealing, opening and the .handoff format, with no UI, OS or network code. It's Apache 2.0. Add it from the repository; it isn't on crates.io yet.",
          blocks: [
            { kind: "p", text: "Built on RustCrypto and dalek: Ed25519 identities, X25519 one-time keys, HKDF-SHA256 and ChaCha20-Poly1305. Secrets are zeroized on drop, and `Debug` output never prints one. Time is passed in as Unix seconds, so tests are deterministic." },
          ],
          code: [
            { title: "Cargo.toml", samples: [{ label: "TOML", language: "toml", code: "[dependencies]\nkeyhandoff-seal = { git = \"" + GITHUB + "\" }\n" }] },
          ],
        },
        {
          id: "seal-identity",
          title: "Identity",
          badge: "struct",
          signature: "Identity::generate() -> Identity",
          question: "What is an identity in keyhandoff-seal?",
          answer: "In keyhandoff-seal, an Identity is one computer's long-term Ed25519 signing key. It signs cards and sealed keys; its public half, PublicIdentity, is what others see, shortened to a 16-character fingerprint for people to compare.",
          fields: [
            { title: "Methods", fields: [
              { name: "Identity::generate()", description: "A fresh identity from the OS's randomness." },
              { name: "Identity::from_bytes(&[u8; 32])", description: "Restore one you stored." },
              { name: "to_bytes()", type: "Zeroizing<[u8; 32]>", description: "The secret half, to store under your own protection." },
              { name: "public()", type: "PublicIdentity", description: "The public half." },
              { name: "PublicIdentity::fingerprint()", type: "String", description: "16 hex characters, like `fe812c12f3ab4ce6`." },
            ] },
          ],
          code: [
            { title: "Example", samples: [{ label: "Rust", language: "rust", code: "use keyhandoff_seal::Identity;\n\nlet ann = Identity::generate();\nstore(&ann.to_bytes());          // keep it secret\nprintln!(\"{}\", ann.public().fingerprint());\n" }] },
          ],
        },
        {
          id: "seal-card",
          title: "Cards and requests",
          badge: "fn",
          signature: "create_card(identity, name, now, ttl, count) -> Result<(Card, Vec<PendingRequest>), Error>",
          question: "How do I make a KeyHandoff card in Rust?",
          answer: "keyhandoff-seal's create_card returns a Card to send, holding count one-time locks signed by your identity, and the PendingRequest for each lock to keep in your Keyring. create_request makes a single lock the same way.",
          fields: [
            { title: "Parameters", fields: [
              { name: "identity", type: "&Identity", description: "Signs every lock." },
              { name: "name", type: "&str", description: "Your display name, 1 to 64 bytes, no control characters." },
              { name: "now", type: "u64", description: "Unix seconds." },
              { name: "ttl", type: "u64", description: "Seconds each lock lives. `DEFAULT_CARD_TTL` is 180 days; `DEFAULT_REQUEST_TTL` is 7." },
              { name: "count", type: "usize", description: "1 to `MAX_CARD_LOCKS` (32). KeyHandoff uses 12." },
            ] },
          ],
          code: [
            { title: "Example", samples: [{ label: "Rust", language: "rust", code: "use keyhandoff_seal::{armor, create_card, Keyring, DEFAULT_CARD_TTL};\n\nlet (card, pending) = create_card(&ann, \"Ann\", now, DEFAULT_CARD_TTL, 12)?;\nlet mut keyring = Keyring::new();\npending.into_iter().for_each(|p| keyring.insert(p));\nsave(&keyring.to_bytes());              // the one-time private keys\n\nlet text = armor::card_text(&card);     // send this\n" }] },
          ],
        },
        {
          id: "seal-seal",
          title: "Sealing a key",
          badge: "fn",
          signature: "seal(sender, request, payload, now) -> Result<Sealed, Error>",
          question: "How do I seal a key to someone's lock with keyhandoff-seal?",
          answer: "keyhandoff-seal's seal() encrypts a Payload (a label, a note and the secret) to one of the receiver's locks, and signs it with the sender's identity. It checks the lock's signature and expiry first. The sender keeps nothing.",
          fields: [
            { title: "Payload", fields: [
              { name: "label", type: "String", description: "The key's name, up to 64 bytes. It's inside the encryption." },
              { name: "note", type: "String", description: "Up to 200 bytes, may be empty." },
              { name: "secret", type: "Secret", description: "1 to 4096 bytes, zeroized on drop. `Secret::new(bytes)`, read with `expose()`." },
            ] },
          ],
          code: [
            { title: "Example", samples: [{ label: "Rust", language: "rust", code: "use keyhandoff_seal::{armor, parse_object, seal, Object, Payload, Secret};\n\nlet Object::Card(card) = parse_object(&armor::decode(&card_text)?)? else { return Err(..) };\nlet lock = &card.locks()[0];\nlet payload = Payload { label: \"Dialpad\".into(), note: String::new(), secret: Secret::new(key) };\nlet sealed = seal(&ben, lock, &payload, now)?;\nlet text = armor::sealed_text(&sealed, card.name());\n" }] },
          ],
        },
        {
          id: "seal-open",
          title: "Opening a key",
          badge: "fn",
          signature: "Keyring::open(&mut self, sealed, now) -> Result<Opened, Error>",
          question: "How do I open a sealed KeyHandoff key in Rust?",
          answer: "keyhandoff-seal's Keyring::open finds the one-time lock a sealed key was made for, verifies it, decrypts it and destroys that lock, returning the payload and the sender's identity. Opening the same file again gives Error::UnknownRequest.",
          blocks: [
            { kind: "callout", tone: "warn", text: "Save the keyring after every open. If you don't, a restart restores the destroyed lock and the file opens twice." },
          ],
          fields: [
            { title: "Keyring", fields: [
              { name: "insert(pending)", description: "Keep a lock you made." },
              { name: "open(&sealed, now)", type: "Result<Opened, Error>", description: "`Opened { payload, sender }`." },
              { name: "destroy(id)", type: "bool", description: "Withdraw one lock." },
              { name: "purge_expired(now)", type: "usize", description: "Drop expired locks; returns how many." },
              { name: "to_bytes() / from_bytes()", description: "Persist it, encrypted by you." },
            ] },
          ],
          code: [
            { title: "Example", samples: [{ label: "Rust", language: "rust", code: "use keyhandoff_seal::{armor, Sealed};\n\nlet sealed = Sealed::parse(&armor::decode(&text)?)?;\nlet opened = keyring.open(&sealed, now)?;\nsave(&keyring.to_bytes());          // the lock is gone for good\n\nlet label = &opened.payload.label;\nlet secret = opened.payload.secret.expose();\n" }] },
          ],
        },
        {
          id: "seal-armor",
          title: "Armored text",
          badge: "mod",
          signature: "armor::decode(text: &str) -> Result<Vec<u8>, Error>",
          question: "How do I read and write .handoff text with keyhandoff-seal?",
          answer: "keyhandoff-seal's armor module turns objects into the damage-resistant text inside .handoff files and back. decode() repairs what chat apps do to pasted text, and parse_object() tells you which kind of object you have.",
          fields: [
            { title: "Functions", fields: [
              { name: "armor::card_text(&card)", type: "String", description: "A card, with a plain-language first line." },
              { name: "armor::sealed_text(&sealed, recipient)", type: "String", description: "A sealed key, naming who it's for." },
              { name: "armor::bundle_text(&bundle, line)", type: "Result<String, Error>", description: "A team bundle." },
              { name: "armor::decode(text)", type: "Result<Vec<u8>, Error>", description: "Back to bytes, repaired or refused." },
              { name: "parse_object(&bytes)", type: "Result<Object, Error>", description: "`Object::Request`, `Sealed`, `Card` or `Bundle`, verified." },
            ] },
          ],
          code: [
            { title: "Example", samples: [{ label: "Rust", language: "rust", code: "use keyhandoff_seal::{armor, parse_object, Object};\n\nmatch parse_object(&armor::decode(&pasted)?)? {\n    Object::Card(card) => add_contact(card),\n    Object::Sealed(sealed) => accept(sealed),\n    Object::Bundle(bundle) => accept_all(bundle.parts()),\n    Object::Request(request) => send_to(request),\n}\n" }] },
          ],
        },
        {
          id: "seal-errors",
          title: "Errors",
          badge: "enum",
          signature: "enum keyhandoff_seal::Error",
          question: "What errors can keyhandoff-seal return?",
          answer: "keyhandoff-seal returns one Error enum for every failure: malformed or tampered objects, expired or already-opened locks, invalid fields, missing randomness, and armored text that's too damaged to repair.",
          errors: [
            { code: "Malformed", description: "Not a KeyHandoff object, or truncated." },
            { code: "UnsupportedVersion", description: "Made by a newer KeyHandoff." },
            { code: "WrongKind", description: "A request where a sealed key was expected, or the other way round." },
            { code: "BadSignature", description: "Changed or forged." },
            { code: "Expired", description: "The lock ran out." },
            { code: "UnknownRequest", description: "Already opened, expired, or made on another computer." },
            { code: "DecryptFailed", description: "The lock doesn't open this object." },
            { code: "InvalidField", description: "A name, label, note or secret is empty, too long or has control characters." },
            { code: "Randomness", description: "The OS gave no randomness." },
            { code: "NotHandoffText", description: "No BEGIN HANDOFF marker." },
            { code: "TooDamaged", description: "Beyond what the parity can repair." },
          ],
          code: [
            { title: "Example", samples: [{ label: "Rust", language: "rust", code: "use keyhandoff_seal::Error;\n\nmatch keyring.open(&sealed, now) {\n    Ok(opened) => keep(opened),\n    Err(Error::UnknownRequest) => println!(\"Already opened, or not for this computer.\"),\n    Err(Error::Expired) => println!(\"The card ran out. Ask for a new one.\"),\n    Err(e) => println!(\"{e}\"),\n}\n" }] },
          ],
        },
      ],
    },
    {
      id: "format",
      title: "File format",
      entries: [
        {
          id: "format-header",
          title: "Objects and header",
          badge: "bin",
          signature: "HOF ‖ version ‖ kind ‖ …",
          question: "How does a KeyHandoff .handoff object start?",
          answer: "Every KeyHandoff object starts with the bytes HOF, a version byte (0) and a kind byte: 1 request, 2 sealed key, 3 card, 4 bundle. Integers are big-endian, and requests and sealed keys end with an Ed25519 signature.",
          blocks: [
            { kind: "table", head: ["Kind", "Object", "Size"], rows: [
              ["1", "Request: one lock", "About 170 bytes plus the name"],
              ["2", "Sealed key", "163 bytes plus the encrypted payload"],
              ["3", "Card: 1 to 32 requests", "About 2 KB for 12 locks"],
              ["4", "Bundle: a team name and up to 200 sealed keys", "Varies"],
            ] },
            { kind: "p", text: "The dump beside this is a real request for Ann, made by the crate: 169 bytes. Its fields, in order: header, 16-byte request id, created and expiry times, Ann's 32-byte Ed25519 identity, the 32-byte X25519 one-time public key, the name, and the 64-byte signature over `handoff:request:v0` and everything before it." },
          ],
          code: [
            { title: "A request, 169 bytes", samples: [{ label: "Hex", language: "text", code: "48 4f 46 00 01                    HOF, version 0, kind 1\ne5 92 dd dd fc 9c 7e 5e\n04 b2 95 36 7b 37 68 d9           request id\n00 00 00 00 6b 49 d2 00           created  1800000000\n00 00 00 00 6b 53 0c 80           expires  1800604800 (+7 days)\nea 4a 6c 63 e2 9c 52 0a be f5 50 7b 13 2e c5 f9\n95 47 76 ae be be 7b 92 42 1e ea 69 14 46 d2 2c\n                                  requester identity (Ed25519)\n5a 33 72 65 99 20 22 7b 04 8c fc 05 a9 0a 49 c4\nec 35 ba c2 5d 6c 16 26 40 49 c0 85 e0 d5 02 25\n                                  one-time key (X25519)\n03 41 6e 6e                       name: 3 bytes, \"Ann\"\nad f5 e1 16 d2 8f 2b ed …         signature, 64 bytes\n" }] },
          ],
        },
        {
          id: "format-sealed",
          title: "Sealed key",
          badge: "bin",
          signature: "kind 2: request id ‖ ephemeral ‖ sender ‖ nonce ‖ ciphertext ‖ signature",
          question: "How is a key sealed in a KeyHandoff file?",
          answer: "A KeyHandoff sealed key is encrypted with ChaCha20-Poly1305 under a key from X25519 between a fresh ephemeral key and the lock's one-time key, through HKDF-SHA256 salted with the hash of the signed request. The sender signs the whole object.",
          blocks: [
            { kind: "steps", items: [
              { title: "Agree", text: "`shared = X25519(ephemeral, one-time public)`; an all-zero result is refused." },
              { title: "Derive", text: "`key = HKDF-SHA256(salt = SHA-256(signed request), ikm = shared, info = \"handoff:seal:v0\" ‖ ephemeral public ‖ one-time public)`." },
              { title: "Encrypt", text: "ChaCha20-Poly1305, random 12-byte nonce, every header byte before the nonce as associated data." },
              { title: "Sign", text: "Ed25519 by the sender over `handoff:sealed:v0` and every byte before the signature." },
            ] },
          ],
          code: [
            { title: "Layout", samples: [{ label: "Layout", language: "text", code: "magic, version, kind      5   HOF 00 02\nrequest id               16   which lock opens it\nephemeral key            32   X25519 public\nsender identity          32   Ed25519 public\nnonce                    12   random\nciphertext length         2\nciphertext                n   payload + 16-byte tag\nsignature                64   Ed25519\n\npayload = label (≤64) ‖ note (≤200) ‖ secret (1–4096)\n" }] },
          ],
        },
        {
          id: "format-card-bundle",
          title: "Card and bundle",
          badge: "bin",
          signature: "kind 3: count ‖ requests · kind 4: team ‖ count ‖ sealed keys",
          question: "What is inside a KeyHandoff card or team file?",
          answer: "A KeyHandoff card (kind 3) is a count and that many complete, signed requests from one identity under one name. A bundle (kind 4) is a team name and up to 200 complete sealed keys; each recipient opens the parts sealed to its own locks.",
          blocks: [
            { kind: "p", text: "Each item is prefixed with its length as two big-endian bytes. A card whose locks don't all share one identity and name is refused. A bundle's team name is a label, not signed by anyone." },
          ],
          code: [
            { title: "Layout", samples: [
              { label: "Card", language: "text", code: "HOF 00 03\ncount                     1   1 to 32\nrepeat count times:\n  length                  2\n  request                 n   a complete kind-1 object\n" },
              { label: "Bundle", language: "text", code: "HOF 00 04\nteam length               1\nteam                      n   UTF-8, 1 to 64 bytes\ncount                     1   1 to 200\nrepeat count times:\n  length                  2\n  sealed key              n   a complete kind-2 object\n" },
            ] },
          ],
        },
        {
          id: "format-armor",
          title: "Armored text",
          badge: "text",
          signature: "-----BEGIN HANDOFF <KIND>-----",
          question: "What is the text inside a .handoff file?",
          answer: "A .handoff file is armored text: a plain-language line, BEGIN and END markers, and the object in Crockford base32 groups of four with interleaved Reed-Solomon parity. Damage from chat apps is repaired; anything beyond repair is refused, never misread.",
          blocks: [
            { kind: "p", text: "Rewrapped lines, quote markers, curly quotes and some damaged characters are repaired, so a file pasted as text and copied back still opens. An integrity check rejects anything repaired wrongly." },
          ],
          code: [
            { title: "The same request, as text", samples: [{ label: ".handoff", language: "text", code: "Ann is asking for a key. Open this with KeyHandoff. It holds no secret.\n-----BEGIN HANDOFF REQUEST-----\n000A TTT8 AJDE Q1HD 917M C001\nWP9D VQFW KHZ5 W15J JMV7 PDV8\nV400 0000 DD4X 4000 0000 0TTK\n1J0E MJKC CFH9 RMGA QVTN 0YRK\n5V2Z K5A7 ETQB XFKV J911 XTK9\n2H3D 4B2T 6DS6 B690 49XG 937W\n0PMG MJE4 XGTV NGJX DGB2 CG29\nR22Y 1N82 4M1M 2VKE NQTY 25PJ\nHWNY T8AT 975Z AA62 KBP4 MQAW\nMTPT 4ZM2 57HA VQ11 Q0N5 HRAJ\n1WJA WT5K 8NDM BB78 Y947 ETVR\n9JS5 8J45 HDYY GGG7 WGH2 J0AV\nHHPD W6B9 4AN6 B378 B3RD Z9NA\nW1T3 RC83 APA4 1AZK 3TYB FKSS\nNEW1 QG04 BTAB SM5X 74RA Z4F2\nK88J P5KQ KRHN 0G72\n-----END HANDOFF REQUEST-----\n" }] },
          ],
        },
      ],
    },
    {
      id: "building",
      title: "Building",
      entries: [
        {
          id: "build",
          title: "Build from source",
          badge: "cli",
          signature: "npm run tauri build",
          question: "How do I build KeyHandoff from source?",
          answer: "KeyHandoff builds with Rust, Node.js and the Tauri prerequisites: npm install, then npm run tauri dev to run with hot reload, or npm run tauri build for an installer. Outside contributions aren't being accepted right now.",
          blocks: [
            { kind: "p", text: "The app is under the PolyForm Shield License 1.0.0, free to read and use, including at work; the sealing crate is Apache 2.0. The code layout is in [architecture.md](" + GITHUB + "/blob/main/docs/architecture.md)." },
          ],
          code: [
            { title: "Build", samples: [{ label: "Shell", language: "sh", code: "git clone " + GITHUB + "\ncd keyhandoff\nnpm install\nnpm run tauri dev                    # run with hot reload\nnpm run tauri build                  # installer\nnpm run tauri build -- --no-bundle   # just the binary\n" }] },
            { title: "A TPM without one", samples: [{ label: "Shell", language: "sh", code: "swtpm socket --tpm2 --tpmstate dir=/tmp/tpm \\\n  --flags not-need-init,startup-clear \\\n  --server type=tcp,port=2321 --ctrl type=tcp,port=2322 &\nKEYHANDOFF_SWTPM=2321 npm run tauri dev\n" }] },
          ],
        },
        {
          id: "tests",
          title: "Tests",
          badge: "cli",
          signature: "cargo test",
          question: "How do I run KeyHandoff's tests?",
          answer: "KeyHandoff's tests run with cargo test in crates/seal and in src-tauri, with synthetic keys only. Real clipboard and keyring checks are opt-in with --ignored, and TPM tests use swtpm when it's installed.",
          blocks: [
            { kind: "p", text: "Each claim on the [threats page](" + GITHUB + "/blob/main/docs/threats.md) links to the test that checks it. Never use a real key in a test or fixture." },
          ],
          code: [
            { title: "Run", samples: [{ label: "Shell", language: "sh", code: "cd crates/seal && cargo test\ncd src-tauri && cargo test\n# real clipboard and keyring checks\ncd src-tauri && cargo test -- --ignored --test-threads=1\n" }] },
          ],
        },
      ],
    },
  ],
};
