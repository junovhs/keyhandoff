import { defineDocs, defineFacts } from "dopedocs";

/** The Strange Systems glyph, for the maker card: the same vectors No Ceremony's uses. */
const STRANGE_SYSTEMS = `<svg viewBox="0 0 505 474" xmlns="http://www.w3.org/2000/svg" fill="currentColor">
  <path d="M0,0 L0,411.878 L216.914,411.878 L216.914,350.713 L62.937,350.713 L62.937,61.163 L216.914,61.163 L216.914,0 Z" />
  <path d="M216.914,153.657 L288.081,122.522 L288.081,319.256 L216.914,350.391 Z" />
  <path d="M505,61.503 L505,473.381 L288.081,473.381 L288.081,412.216 L442.058,412.216 L442.058,122.666 L288.081,122.666 L288.081,61.503 Z" />
</svg>`;

const GITHUB = "https://github.com/junovhs/keyhandoff";
const REVIEWED = "2026-10-31";

export const docs = defineDocs({
  entity: {
    name: "KeyHandoff",
    url: "https://keyhandoff.dev",
    legalName: "Strange Systems",
    notToBeConfusedWith: ["Other products and services named KeyHandoff or Key Handoff"],
  },
  identity: { name: "KeyHandoff", version: "0.1", channel: "Pre-release", maker: { name: "Strange Systems", href: "#strange-systems" } },
  title: "KeyHandoff",
  lead: "Hand off API keys without exposing them. The user manual, the developer documentation, and articles on why it works the way it does.",
  facts: defineFacts({
    platforms: { value: "Windows and Linux (X11)", reviewed: REVIEWED },
    status: { value: "pre-release and has not had a security audit", reviewed: REVIEWED },
    network: { value: "no servers, accounts, telemetry or network code", reviewed: REVIEWED },
    cardLocks: { value: "12", reviewed: REVIEWED },
    cardLife: { value: "180 days", reviewed: REVIEWED },
    safeCopyTime: { value: "60 seconds", reviewed: REVIEWED },
    programWait: { value: "60 seconds", reviewed: REVIEWED },
    unlockWait: { value: "two minutes", reviewed: REVIEWED },
    minPassphrase: { value: "12 characters", reviewed: REVIEWED },
    minPin: { value: "4 characters", reviewed: REVIEWED },
    suggested: { value: "five random words from the EFF long list, about 64 bits", reviewed: REVIEWED },
  }),
  sections: [
    // ------------------------------------------------------------------ manual
    {
      id: "manual",
      title: "User manual",
      landing: {
        id: "user-manual",
        title: "User manual",
        question: "How do I use KeyHandoff?",
        answer: "KeyHandoff's user manual covers installing it, choosing a passphrase, sharing your card, sending and keeping keys, teams, using keys in code through .env pointers, and locking.",
        blocks: [
          { kind: "callout", tone: "warn", text: "KeyHandoff is {fact:status}. Use it with test keys for now." },
          {
            kind: "cards",
            items: [
              { title: "Getting started", text: "What KeyHandoff is, how to build it, and the first start.", href: "#what-is-it", label: "Start" },
              { title: "Sharing keys", text: "Your card, sending a key, keeping one, and teams.", href: "#how-it-works", label: "Share" },
              { title: "Your keys", text: "Adding your own keys, Safe Copy, and keep times.", href: "#add-a-key", label: "Keep" },
              { title: "Keys in your code", text: "Pointers in .env, the loaders, and allowing programs.", href: "#env-pointers", label: "Code" },
              { title: "Locking", text: "The passphrase, a TPM PIN, and remembering.", href: "#locking", label: "Lock" },
              { title: "Help", text: "When something goes wrong, and what KeyHandoff can't do.", href: "#troubleshooting", label: "Help" },
            ],
          },
        ],
      },
      children: [
        {
          id: "manual-start",
          title: "Getting started",
          children: [
            {
              id: "what-is-it",
              title: "What is KeyHandoff?",
              question: "What is KeyHandoff?",
              answer: "KeyHandoff is a small desktop app for handing someone an API key, or any other secret, without pasting it into a chat or leaving it in a plain file. It has {fact:network}.",
              blocks: [
                { kind: "p", text: "Encrypting a key at rest is the easy part. The real exposure comes from people handling it: looking at it, copying it, pasting it into a chat, saving it in a `.env` file. KeyHandoff keeps custody of the key from the provider's page onward, so people and their tools handle it as little as possible." },
                { kind: "p", text: "Cards and keys travel as ordinary files over whatever channel you already use. KeyHandoff has no way to show a key on screen." },
                {
                  kind: "table",
                  head: ["Thing", "What it is", "Secret inside?"],
                  rows: [
                    ["Card (`KeyHandoff card - Ann.handoff`)", "Ann's public identity and {fact:cardLocks} one-time locks. Whoever has it can send Ann up to {fact:cardLocks} keys.", "No"],
                    ["Sealed key file (`Key for Ann.handoff`)", "The key, locked so only Ann's computer can open it, and only once.", "Yes, encrypted"],
                    ["Team key file (`Key for Platform.handoff`)", "One key, sealed separately for each member of a team.", "Yes, encrypted"],
                    ["Kept key", "An entry in your vault: its name and source, never its value.", "Stored encrypted"],
                  ],
                },
                { kind: "callout", tone: "warn", text: "KeyHandoff is {fact:status}. Use it with test keys for now." },
              ],
            },
            {
              id: "install",
              title: "Install",
              question: "How do I install KeyHandoff?",
              answer: "KeyHandoff is built from source for now. Install Rust, Node.js and the Tauri prerequisites, clone the repository, then run npm install and npm run tauri build.",
              keywords: ["install", "build", "download"],
              blocks: [
                { kind: "p", text: "There are no signed downloads yet. KeyHandoff runs on {fact:platforms}." },
                {
                  kind: "steps",
                  items: [
                    { title: "Get the tools", text: "Install [Rust](https://rustup.rs), [Node.js](https://nodejs.org) and the [Tauri prerequisites](https://tauri.app/start/prerequisites/) for your system." },
                    { title: "Get the code", text: "Clone [the repository](" + GITHUB + ")." },
                    { title: "Build it", text: "Run the commands below. The installer lands in `src-tauri/target/release/bundle`." },
                  ],
                },
                { kind: "code", language: "sh", code: "git clone " + GITHUB + "\ncd keyhandoff\nnpm install\nnpm run tauri build                   # installer\nnpm run tauri build -- --no-bundle    # just the binary\n" },
                { kind: "p", text: "The Windows installer registers `.handoff` files, so double-clicking one opens it in KeyHandoff. On Linux, `scripts/register-linux.sh` does the same for a locally built binary." },
              ],
            },
            {
              id: "first-start",
              title: "First start",
              question: "What happens the first time I open KeyHandoff?",
              answer: "The first time it opens, KeyHandoff asks you to choose a passphrase, or take five suggested words, and shows a recovery code once to write down. The passphrase opens your keys from then on.",
              blocks: [
                {
                  kind: "steps",
                  items: [
                    { title: "Choose a passphrase", text: "At least {fact:minPassphrase}. **Suggest five** gives you {fact:suggested}, shown whole so you can learn or write them down. **Type my own** goes back to typing." },
                    { title: "Write down the recovery code", text: "Eight groups of four characters. It's the only way back in if you forget the passphrase, and it's shown this once. Keep it on paper, not on the computer." },
                    { title: "Start using it", text: "Your keys screen opens. From now on KeyHandoff starts locked and asks for the passphrase." },
                  ],
                },
                { kind: "p", text: "If you used KeyHandoff before it had a passphrase, the first start says **Lock your keys** instead: your existing keys move behind the new passphrase, and the old copy of the key that any program could read is deleted." },
                { kind: "callout", tone: "note", text: "Forget both the passphrase and the recovery code, and the keys can't be opened by anyone, you included. Replace them with the provider." },
              ],
            },
          ],
        },
        {
          id: "manual-sharing",
          title: "Sharing keys",
          children: [
            {
              id: "how-it-works",
              title: "How a handoff works",
              question: "How does KeyHandoff hand off a key?",
              answer: "The receiver sends their card once. After that, the sender picks them in Send a key, catches the key from the provider's page and seals it, and the receiver opens the sealed file once to keep the key in their vault.",
              blocks: [
                {
                  kind: "steps",
                  items: [
                    { title: "Share your card", text: "Click **Your card** and send the file to whoever will send you keys. It holds no secret, and each copy is good for {fact:cardLocks} keys." },
                    { title: "Catch", text: "They click **Send a key**, pick you, and copy the key from the provider's page. KeyHandoff catches that one copy and wipes the clipboard." },
                    { title: "Seal", text: "They name the key and it's encrypted so only your computer can open it, and only once. They send you the file and keep nothing." },
                    { title: "Keep", text: "You open the file. The key goes into your vault, and every copy of the sealed file becomes permanently unreadable." },
                  ],
                },
                { kind: "p", text: "Slack, Teams, email: anything that carries a file works, and the file only ever holds ciphertext. When a channel only takes text, paste the file's text instead; it repairs itself if the chat mangles it." },
              ],
            },
            {
              id: "your-card",
              title: "Your card",
              question: "What is a KeyHandoff card?",
              answer: "A KeyHandoff card is a file holding your computer's public identity and {fact:cardLocks} one-time locks. Send it once to someone who'll send you keys; each key they send uses one lock. It holds no secret.",
              blocks: [
                {
                  kind: "list",
                  items: [
                    "Click **Your card** at the bottom of the home screen. The file `KeyHandoff card - <your name>.handoff` goes on the clipboard; paste it into the chat.",
                    "Each copy is for one person. Click **Copy again** for the next person, so each gets fresh locks.",
                    "Your name on the card comes from **You are …** in the footer; click it to change it.",
                    "The card shows your computer's fingerprint, a short code. If it matters, read it to the other person so they can check the card is really yours.",
                    "Unused locks stop working after {fact:cardLife}.",
                  ],
                },
                { kind: "p", text: "When someone opens your card, you become one of their contacts and they see how many keys they can still send you. When the locks run out, Send a key says to ask you for a new card." },
              ],
            },
            {
              id: "send-a-key",
              title: "Send a key",
              question: "How do I send someone a key with KeyHandoff?",
              answer: "Open their card once so they're a contact. Then click Send a key, pick them, and copy the key on the provider's page. KeyHandoff catches the copy, you name the key, and you send the sealed file it makes.",
              blocks: [
                {
                  kind: "steps",
                  items: [
                    { title: "Add their card", text: "Open the card file they sent you: double-click it, drop it on the window, or press Ctrl V in KeyHandoff. **Card added** shows their name and computer fingerprint." },
                    { title: "Pick them", text: "Click **Send a key** and choose the person (or a team). Safe Copy starts listening." },
                    { title: "Copy the key", text: "On the provider's page, copy the key the usual way. KeyHandoff catches that one copy and wipes the clipboard. See [Safe Copy](#safe-copy)." },
                    { title: "Name it and seal it", text: "Type a name, like Dialpad, and seal. The window shows **Sealed** and the file `Key for <name>.handoff`." },
                    { title: "Send the file", text: "Click **Copy** and paste it into the chat. When you close the window, the sealed file is deleted: you keep nothing." },
                  ],
                },
                { kind: "callout", tone: "note", text: "Opening a card trusts the channel it came through. If someone else can post as that person, check the computer fingerprint with them before you send anything." },
              ],
            },
            {
              id: "receive",
              title: "Receive and keep a key",
              question: "How do I open a key someone sent me in KeyHandoff?",
              answer: "Open the sealed file: double-click it, drop it on the KeyHandoff window, or press Ctrl V there. Choose how long to keep the key. The file can never be opened again, by anyone.",
              blocks: [
                { kind: "p", text: "The window shows who sent it and the key's name, then asks how long to keep it: **Use once**, **Keep 30 days** or **Keep forever**. Your last choice is the default next time. See [Keep times](#keep-times)." },
                { kind: "p", text: "Opening the file destroys the lock it was sealed to, so the copy still sitting in the chat is useless. Opening it a second time says it was already accepted." },
                { kind: "p", text: "If KeyHandoff is locked when you open a file, it opens right after you unlock." },
              ],
            },
            {
              id: "teams",
              title: "Teams",
              question: "How do teams work in KeyHandoff?",
              answer: "A KeyHandoff team is a named group of your contacts. Sending to a team catches the key once and seals a separate copy for each member in one file; you keep a copy too. Welcome sends a newcomer every key shared with the team.",
              blocks: [
                {
                  kind: "list",
                  items: [
                    "In **Send a key**, click **New team**, name it and tick the people. Everyone on it must have sent you their card.",
                    "Pick the team in Send a key to share a key with everyone at once. Each member opens their own copy, once.",
                    "**Edit** a team to change its people. **Welcome** next to a member sends them every key you hold that was shared with the team, in one file.",
                    "Keys from a team are filed under its name in your keys.",
                  ],
                },
                { kind: "p", text: "There's no server, so a team lives on each person's computer. Anyone can set up the same team to share with it. Taking someone off a team stops new keys, but keys they already have stay with them: replace those with the provider." },
              ],
            },
          ],
        },
        {
          id: "manual-keys",
          title: "Your keys",
          children: [
            {
              id: "add-a-key",
              title: "Add a key of your own",
              question: "How do I add my own key to KeyHandoff?",
              answer: "Click Add a key, copy the key on the provider's page, then name it and choose how long to keep it. No files are involved, and KeyHandoff never shows the key.",
              blocks: [
                { kind: "p", text: "Use this for keys you create yourself. It's the same catch as sending a key, except the key goes straight into your vault." },
                { kind: "p", text: "Each key's name is unique, and that name is what a `.env` pointer uses. See [Pointers in .env](#env-pointers)." },
              ],
            },
            {
              id: "safe-copy",
              title: "Safe Copy",
              question: "When does KeyHandoff read my clipboard?",
              answer: "KeyHandoff's Safe Copy reads the clipboard only after you ask it to. It catches one item, then switches off, or times out after {fact:safeCopyTime}.",
              blocks: [
                { kind: "p", text: "Nothing watches the clipboard the rest of the time. After a catch, KeyHandoff wipes the clipboard and removes the copy from clipboard history where the system allows it. While Safe Copy is on, the window stays above other apps so you can copy from them." },
                {
                  kind: "list",
                  items: [
                    "**Clipboard apps.** When it turns on, Safe Copy names known clipboard managers that are running, such as Ditto or CopyQ, since they can store the key as you copy it. Close them first.",
                    "**Something read your clipboard.** On Linux, KeyHandoff puts harmless bait on the clipboard while it listens. If another program reads it, you're warned before you copy.",
                    "**Time ran out.** Nothing was copied within {fact:safeCopyTime}. Click **Try again**.",
                  ],
                },
                { kind: "callout", tone: "warn", text: "From your copy until the wipe, the key is on the system clipboard. A program watching the clipboard can read it in that moment. On Linux (X11) it always can. See [What it protects](#limits)." },
              ],
            },
            {
              id: "keep-times",
              title: "Keep times and your list",
              question: "How long does KeyHandoff keep a key?",
              answer: "KeyHandoff keeps each key Once (destroyed after a program first uses it), for 30 days, or until you remove it. You choose when you accept or add it, and can change it on the key's row.",
              blocks: [
                {
                  kind: "table",
                  head: ["Choice", "What happens"],
                  rows: [
                    ["Use once", "Destroyed after a program first gets it."],
                    ["Keep 30 days", "Destroyed automatically after 30 days."],
                    ["Keep forever", "Kept until you remove it."],
                  ],
                },
                { kind: "p", text: "Once you have six or more keys, the list gets a search box (type anywhere, or press `/` or Ctrl F), filters for **Shared**, **Yours** and **Expiring**, and a sort order. To remove a key, click the × on its row, then **Forget for good?** to confirm." },
              ],
            },
          ],
        },
        {
          id: "manual-code",
          title: "Keys in your code",
          children: [
            {
              id: "env-pointers",
              title: "Pointers in .env",
              question: "How do I use a KeyHandoff key in my .env file?",
              answer: "Put a pointer in .env instead of the key, like DIALPAD_API_KEY=handoff:Dialpad. A one-line loader in your program asks KeyHandoff for the key at startup. A search of your files then finds a pointer, not a key.",
              blocks: [
                { kind: "compare", before: { title: ".env today", text: "`DIALPAD_API_KEY=dp_7Qm2vX9kLw4Tz`" }, after: { title: ".env with KeyHandoff", text: "`DIALPAD_API_KEY=handoff:Dialpad`" } },
                { kind: "p", text: "**Copy .env line** on a key's row puts that line on the clipboard, with a variable name made from the key's name; rename the variable if your code expects another. The name after `handoff:` matches a kept key, ignoring case." },
              ],
            },
            {
              id: "loaders",
              title: "Python and Node.js loaders",
              question: "How does my program get a key from KeyHandoff?",
              answer: "Copy KeyHandoff's loader file next to your script and call it once at startup: keyhandoff_env.load() in Python, await load() in Node.js. It fills every handoff: pointer in the environment; your code reads os.environ or process.env as before.",
              blocks: [
                {
                  kind: "tabs",
                  label: "Language",
                  group: "language",
                  panes: [
                    { key: "python", label: "Python", blocks: [
                      { kind: "p", text: "Copy [keyhandoff_env.py](" + GITHUB + "/blob/main/loaders/keyhandoff_env.py) next to your script. Standard library only." },
                      { kind: "code", language: "python", code: "import os\nimport keyhandoff_env\n\nkeyhandoff_env.load()          # after load_dotenv(), if you use it\napi_key = os.environ[\"DIALPAD_API_KEY\"]\n" },
                    ] },
                    { key: "node", label: "Node.js", blocks: [
                      { kind: "p", text: "Copy [keyhandoff_env.mjs](" + GITHUB + "/blob/main/loaders/keyhandoff_env.mjs) next to your script. No dependencies." },
                      { kind: "code", language: "js", code: "import { load } from \"./keyhandoff_env.mjs\";\n\nawait load();                  // after dotenv, if you use it\nconst apiKey = process.env.DIALPAD_API_KEY;\n" },
                    ] },
                  ],
                },
                { kind: "p", text: "The loader asks the running app for every pointed-to key at once, over a local socket (a named pipe on Windows), and sets the variables inside your program. Nothing is written to disk. It runs inside your program rather than launching it with the key, because a program's starting environment can be read by other programs running as you." },
                { kind: "p", text: "For another profile, pass it or set `KEYHANDOFF_PROFILE`. If KeyHandoff is locked, its window comes forward and the loader waits up to {fact:unlockWait} for you to unlock." },
                { kind: "callout", tone: "note", text: "Python and JavaScript strings can't be wiped, so the value stays in your program's memory until it exits." },
              ],
            },
            {
              id: "allowing-programs",
              title: "Allowing a program",
              question: "Why does KeyHandoff ask me to allow a program?",
              answer: "The first time a program asks KeyHandoff for a key, the window names the program and the folder it runs in. Always allow covers that program, in that folder, for those keys. Just this once and Deny are the other choices.",
              blocks: [
                { kind: "p", text: "KeyHandoff asks the operating system which program is asking and where it runs, and shows the command line so you can recognise the script. The same program in another folder asks again, because allowing `python` everywhere would allow every script." },
                { kind: "p", text: "A program waits up to {fact:programWait} for your answer, then is refused. Forgetting a key removes its approvals." },
                { kind: "callout", tone: "warn", text: "The prompt is a decision, not a lock: anything running as you could start the same program in the same folder. And a program that has a key can do anything with it." },
              ],
            },
          ],
        },
        {
          id: "manual-lock",
          title: "Locking",
          children: [
            {
              id: "locking",
              title: "How locking works",
              question: "How is the KeyHandoff vault locked?",
              answer: "KeyHandoff starts locked, and your passphrase opens it. It locks again when the computer sleeps or the screen locks, and from the lock button. A recovery code you wrote down at setup always gets you back in.",
              blocks: [
                { kind: "p", text: "While it's locked, a program running as you can read every KeyHandoff file and still can't open a key. Locking drops the key that opens the vault from memory." },
                {
                  kind: "list",
                  items: [
                    "**Lock now:** the lock icon in the title bar.",
                    "**Automatically:** when the computer sleeps or the screen locks.",
                    "**After a quiet spell:** off by default. Set `lock_after_minutes` in the profile's `settings.json` to turn it on.",
                    "**Programs while locked:** a program asking for a key brings the window forward and waits up to {fact:unlockWait}.",
                  ],
                },
                { kind: "p", text: "**Security**, at the bottom of the home screen, offers a [PIN instead of the passphrase](#tpm-pin) and [Remember on this computer](#remember)." },
              ],
            },
            {
              id: "tpm-pin",
              title: "A PIN instead of the passphrase",
              question: "Can I unlock KeyHandoff with a PIN?",
              answer: "Yes, where the computer has a TPM KeyHandoff can use: Linux for now. In Security, Add a PIN seals the vault key inside the TPM, which checks the PIN and stops anyone guessing it, so the PIN can be short.",
              blocks: [
                { kind: "steps", items: [
                  { title: "Open Security", text: "Click **Security** at the bottom of the home screen." },
                  { title: "Add a PIN", text: "Type a PIN of at least {fact:minPin}, twice, and click **Add**." },
                  { title: "Unlock with it", text: "The lock screen now asks for the PIN first. **Use your passphrase** and the recovery code still work." },
                ] },
                { kind: "p", text: "The sealed key only opens on this computer's TPM, and the TPM refuses tries for a while after several wrong PINs. **Turn off** in Security removes it." },
                { kind: "details", summary: "Security says the TPM isn't allowed", text: "On Linux the TPM device belongs to the tss group. Run `sudo usermod -aG tss $USER`, then sign out and back in." },
                { kind: "details", summary: "Security says there's no TPM", text: "Many computers have a TPM that's switched off in the firmware settings, often called AMD fTPM or Intel PTT. Your passphrase works without one. KeyHandoff only uses hardware your computer already has." },
                { kind: "p", text: "On Windows, Windows Hello is planned instead: your face, fingerprint or PIN." },
              ],
            },
            {
              id: "remember",
              title: "Remember on this computer",
              question: "Can KeyHandoff open without asking for a passphrase?",
              answer: "Yes, if you turn on Remember on this computer in Security. KeyHandoff then opens without asking and stays open through sleep and screen lock, but any program running as you could open your keys. It's off unless you turn it on.",
              blocks: [
                { kind: "p", text: "Remembering keeps a copy of the vault key with the operating system: Windows' DPAPI, or on Linux the system keyring (or a private file without one). Your computer's sign-in is then the only lock left." },
                { kind: "p", text: "Pick **Ask to unlock** in Security to turn it off; the copy is deleted. Your passphrase and recovery code work either way." },
              ],
            },
            {
              id: "recovery",
              title: "Forgot your passphrase",
              question: "What do I do if I forget my KeyHandoff passphrase?",
              answer: "On the lock screen, click Forgot it? Use your recovery code, type the code you wrote down at setup, and choose a new passphrase. Without the code, the keys can't be opened.",
              blocks: [
                { kind: "p", text: "The code forgives case, spaces and dashes, and the letters people write for 0 and 1. Using it replaces the passphrase; the code itself keeps working." },
                { kind: "p", text: "There's no reset by email and nobody to call: KeyHandoff has no servers. If both are lost, replace the keys with the provider and set KeyHandoff up again." },
              ],
            },
          ],
        },
        {
          id: "manual-help",
          title: "Help",
          children: [
            {
              id: "troubleshooting",
              title: "Troubleshooting",
              question: "What do KeyHandoff's error messages mean?",
              answer: "Most KeyHandoff problems are a locked or closed app, a used-up card, a file that was already opened, or a program that wasn't allowed. Each is listed here with what to do.",
              blocks: [
                { kind: "details", summary: "KeyHandoff isn't running. Open it and try again.", text: "The loader couldn't reach the app. Start KeyHandoff with the same profile, then run your program again." },
                { kind: "details", summary: "This key was already accepted and can't be opened.", text: "Each sealed file opens once. If you didn't open it, someone else may have had the same card copy: ask the sender to send it again." },
                { kind: "details", summary: "You've used up Ann's card. Ask Ann for a new one.", text: "Every key sent uses one of the card's locks. Ask them for a new card copy and open it." },
                { kind: "details", summary: "Not allowed in KeyHandoff.", text: "You chose Deny, or nobody answered within {fact:programWait}. Run the program again and choose Always allow or Just this once." },
                { kind: "details", summary: "No kept key is named …", text: "The name after handoff: doesn't match a key. Check the spelling on the key's row, or use Copy .env line." },
                { kind: "details", summary: "Too many wrong PINs", text: "The TPM is refusing tries for a while. Use your passphrase, then try the PIN later." },
                { kind: "details", summary: "Nothing was caught", text: "Safe Copy saw no copy within {fact:safeCopyTime}. Click Try again and copy the key with the page's copy button or Ctrl C." },
              ],
            },
            {
              id: "limits",
              title: "What it protects, and what it doesn't",
              question: "What does KeyHandoff not protect against?",
              answer: "KeyHandoff can't stop a program watching the clipboard from reading a key at the moment you copy it, or stop malware running as you from reaching the vault while it's unlocked. A program you allow can do anything with the key it gets.",
              blocks: [
                { kind: "compare", before: { title: "What holds", text: "A sealed file opens once, on one computer. The sender keeps nothing. Keys are encrypted on disk and never shown. While locked, no program can open them. No servers or accounts." },
                  after: { title: "What doesn't", text: "The copy moment on the clipboard. Anything running as you while unlocked. What an allowed program does with a key. An administrator, or a computer an attacker controls." } },
                { kind: "p", text: "The full list, with the test or design rule behind each claim, is in [What KeyHandoff protects, and what it doesn't](" + GITHUB + "/blob/main/docs/threats.md)." },
              ],
            },
            {
              id: "platforms",
              title: "Platforms",
              question: "Which platforms does KeyHandoff support?",
              answer: "KeyHandoff supports {fact:platforms}. macOS and Wayland are not supported.",
              blocks: [
                { kind: "table", head: ["Feature", "Windows", "Linux (X11)"], rows: [
                  ["Cards, sealing, teams", "Yes", "Yes"],
                  ["Safe Copy history cleanup", "Removes the Win+V entry", "No shared history to clean"],
                  ["Clipboard bait warning", "Not yet", "Yes"],
                  ["Passphrase lock", "Yes", "Yes"],
                  ["TPM PIN", "Not yet (Windows Hello planned)", "Yes"],
                ] },
              ],
            },
            {
              id: "strange-systems",
              title: "Who makes this",
              question: "Who makes KeyHandoff?",
              answer: "KeyHandoff is made by Strange Systems, a one-person indie developer studio in Eugene, Oregon, run by Spencer Nunamaker.",
              blocks: [
                {
                  kind: "facts",
                  title: "Strange Systems",
                  mark: STRANGE_SYSTEMS,
                  href: "https://strangesystems.dev",
                  rows: [
                    ["Based in", "Eugene, Oregon"],
                    ["Size", "One person, for now"],
                    ["That person", "Spencer Nunamaker - UI/UX designer, musician, artist"],
                    ["Also made", "No Ceremony, and Tip Top Brushes for Procreate"],
                  ],
                },
              ],
            },
          ],
        },
      ],
    },
    // -------------------------------------------------------------- developers
    {
      id: "developers",
      title: "Developer documentation",
      landing: {
        id: "developer-docs",
        title: "Developer documentation",
        question: "How is KeyHandoff built?",
        answer: "KeyHandoff is a Tauri app: React and TypeScript screens that never see a secret, and a Rust core that holds every key. The sealing library is a standalone Rust crate. These pages cover building, the architecture, the file formats and the key socket.",
        blocks: [
          { kind: "cards", items: [
            { title: "Build and run", text: "Toolchain, profiles and the software TPM.", href: "#dev-build", label: "Build" },
            { title: "Architecture", text: "Where everything lives and why.", href: "#dev-architecture", label: "Read" },
            { title: "File formats", text: "Requests, sealed keys, cards, bundles, armored text.", href: "#dev-formats", label: "Read" },
            { title: "Key storage", text: "Key slots, the TPM PIN and remembering.", href: "#dev-key-storage", label: "Read" },
            { title: "Key socket", text: "The one-line protocol loaders speak.", href: "#dev-key-socket", label: "Read" },
            { title: "Tests and rules", text: "How it's checked, and the design rules.", href: "#dev-tests", label: "Read" },
          ] },
          { kind: "p", text: "The source is on [GitHub](" + GITHUB + "). The app is under the PolyForm Shield License 1.0.0; the sealing library, `crates/seal`, is Apache 2.0. Outside contributions aren't being accepted right now." },
        ],
      },
      children: [
        {
          id: "dev-build",
          title: "Build and run",
          question: "How do I build and run KeyHandoff from source?",
          answer: "Install Rust, Node.js and the Tauri prerequisites, then run npm install and npm run tauri dev for a hot-reloading build. Use --profile or KEYHANDOFF_PROFILE to run two identities side by side.",
          blocks: [
            { kind: "code", language: "sh", code: "npm install\nnpm run tauri dev                    # run with hot reload\nnpm run tauri build                  # installer in src-tauri/target/release/bundle\nnpm run tauri build -- --no-bundle   # just the binary\n" },
            { kind: "p", text: "**Two identities on one computer:** run each with its own profile, `keyhandoff --profile ben`, or set `KEYHANDOFF_PROFILE=ben`. Each profile has its own data folder, vault, passphrase and running instance; their windows open on top of each other, so move them apart." },
            { kind: "p", text: "**A TPM without one:** debug builds talk to the [swtpm](https://github.com/stefanberger/swtpm) software TPM when `KEYHANDOFF_SWTPM` names its port, so the PIN flow can be tried on any Linux machine." },
            { kind: "code", language: "sh", code: "swtpm socket --tpm2 --tpmstate dir=/tmp/tpm --flags not-need-init,startup-clear \\\n  --server type=tcp,port=2321 --ctrl type=tcp,port=2322 &\nKEYHANDOFF_SWTPM=2321 npm run tauri dev\n" },
            { kind: "p", text: "**Screens without the app:** `node scripts/site-shots/board.mjs` renders every screen with sample data to `docs/screens.png`, and `shoot.mjs` makes the website's screenshots." },
          ],
        },
        {
          id: "dev-architecture",
          title: "Architecture",
          question: "How is KeyHandoff's code organised?",
          answer: "KeyHandoff has three parts: crates/seal (identity, sealing and the file format, with no UI or OS code), the Rust core in src-tauri (flows, vault, locking, per-OS backends) and the React screens in src, which only ever get names, states and ids.",
          blocks: [
            { kind: "code", language: "text", code: "crates/seal/       identity, requests, seal and open, one-time keys,\n                   the .handoff format and armored text\nsrc/               React + TypeScript screens; never receive a secret value\nsrc-tauri/src/\n  lib.rs           window and Tauri commands (names, states and ids only)\n  core.rs          every flow: cards, Safe Copy, seal, accept, add, teams,\n                   giving keys to allowed programs; holds the secrets\n  session.rs       locked or open: unlock, lock on sleep, screen lock or idle\n  store.rs         profile files sealed under the master key; the locked profile\n  slots.rs         keyslots.bin: passphrase and recovery code; the TPM PIN slot\n  tpm.rs           a small TPM 2.0 client: seal and unseal behind a PIN\n  vault.rs         kept keys, keep times, expiry\n  pointer.rs       what programs asking for keys send and get back\n  os/              per-OS backends (linux/, windows/): clipboard, drag-out,\n                   one instance per profile, the key socket or pipe\nloaders/           the Python and Node.js loaders\n" },
            { kind: "p", text: "**Secrets live only in Rust.** A screen asks to \"seal what you caught for this contact\"; it never passes or receives the value. The passphrase and the recovery code are the one exception: they're typed or shown in the window and go straight to Rust." },
            { kind: "p", text: "**Safe Copy, Windows:** a message-only window with `AddClipboardFormatListener` reads the first text change, empties the clipboard, then deletes the matching Win+V entry on a single-threaded WinRT thread. **Linux (X11):** KeyHandoff owns the clipboard with bait text while listening, XFixes reports the next owner change, and the text is read once. Wayland isn't supported." },
            { kind: "p", text: "**Transport:** `.handoff` files hold armored text. Copy puts the file on the clipboard with the text as a fallback. A second launch hands its files to the running instance over a local pipe or socket. There are no links and no website in the flow: an https link would need a domain whose hijacking could swap a card's locks." },
          ],
        },
        {
          id: "dev-formats",
          title: "File formats",
          question: "What is inside a .handoff file?",
          answer: "A .handoff file holds armored text around a binary object: HOF, version 0 and a kind. Kind 1 is a request (one lock), 2 a sealed key, 3 a card (a batch of locks), 4 a bundle of sealed keys for a team. Sealing uses X25519, HKDF-SHA256, ChaCha20-Poly1305 and Ed25519.",
          blocks: [
            { kind: "table", head: ["Kind", "Object", "Holds"], rows: [
              ["1", "Request (one lock)", "Request id, created and expiry times, the requester's Ed25519 identity, a fresh X25519 one-time public key, a name; signed. No secret."],
              ["2", "Sealed key", "Request id, ephemeral X25519 key, sender identity, nonce, ciphertext of label, note and secret; signed by the sender."],
              ["3", "Card", "1 to 32 complete requests from one identity under one name."],
              ["4", "Bundle", "A team name (a label, unsigned) and up to 200 sealed keys."],
            ] },
            { kind: "steps", items: [
              { title: "Agree", text: "The sender makes an ephemeral X25519 key pair and computes the shared secret with the lock's one-time public key, rejecting an all-zero result." },
              { title: "Derive", text: "HKDF-SHA256, salted with the SHA-256 of the whole signed request, so the key is bound to that exact lock, identity and name." },
              { title: "Encrypt", text: "ChaCha20-Poly1305 under a random nonce; every header byte before the nonce is associated data. The label is inside the encryption." },
              { title: "Sign", text: "The sender's Ed25519 identity signs the whole object, which lets a receiver warn when a known sender's key changes." },
            ] },
            { kind: "p", text: "**Opening** finds the pending one-time key by request id, checks expiry and signature, decrypts, then destroys the pending key before the secret reaches the vault. Every copy of the file is then unopenable." },
            { kind: "p", text: "**Armored text** is Crockford base32 in groups of four with Reed-Solomon parity over GF(2^8), interleaved, and an integrity check that refuses anything repaired wrongly. Rewrapped lines, quote markers and curly quotes are repaired." },
            { kind: "p", text: "The full layout, byte by byte, is in [sealing-format.md](" + GITHUB + "/blob/main/docs/sealing-format.md)." },
          ],
        },
        {
          id: "dev-key-storage",
          title: "Key storage",
          question: "How does KeyHandoff protect its vault on disk?",
          answer: "Each KeyHandoff profile has a random 32-byte master key that seals every profile file with ChaCha20-Poly1305. The master key is stored only wrapped: by an Argon2id passphrase, a 160-bit recovery code, and optionally a TPM-sealed PIN.",
          blocks: [
            { kind: "table", head: ["File", "Holds"], rows: [
              ["keyslots.bin", "`HOFK`, version, then per slot: kind (1 passphrase, 2 recovery code), Argon2id memory, passes and lanes, salt, nonce, and the sealed master key. The header is associated data."],
              ["keyslot-tpm.bin", "`HOFT`, version, the PIN's Argon2id cost and salt, then the TPM sealed object's private and public blobs."],
              ["identity.bin, pending.bin, vault.bin, approvals.bin, contacts.bin, teams.bin", "Sealed under the master key, with the file name as associated data. Each kept secret is sealed again with its item id."],
              ["settings.json", "Your name, the last keep time, what protects the master key, `remembered`, `lock_after_minutes`. No secret."],
            ] },
            { kind: "p", text: "**Passphrase:** Argon2id at 256 MiB, 3 passes, 4 lanes. **Recovery code:** 20 random bytes as eight groups of four Crockford base32 characters." },
            { kind: "p", text: "**TPM PIN:** `tpm.rs` sends five TPM 2.0 commands directly over `/dev/tpmrm0`, so no TPM libraries are needed. CreatePrimary rebuilds a P-256 storage key from the owner seed; Create seals the master key in a keyed-hash object whose password is the PIN's Argon2id hash, with dictionary-attack protection on; Load and Unseal open it; FlushContext cleans up." },
            { kind: "p", text: "**Remember on this computer** puts a copy of the master key back with the OS (DPAPI, the Secret Service, or a `0600` file) and records the backend in `remembered`. Turning it off deletes that copy." },
            { kind: "p", text: "**Locking** (`session.rs`) drops the `Handoff` core, and with it the master key, identity and loaded vault, and returns to the locked profile. The key socket stays up so programs can wait for the unlock." },
          ],
        },
        {
          id: "dev-key-socket",
          title: "Key socket protocol",
          question: "How do programs ask KeyHandoff for keys?",
          answer: "A program sends KeyHandoff one line of JSON, {\"names\": [\"Dialpad\"]}, over a per-user local socket or named pipe, and gets one line back: {\"values\": {\"Dialpad\": \"…\"}} or {\"error\": \"…\"}. The loaders do this for you.",
          blocks: [
            { kind: "table", head: ["", "Linux", "Windows"], rows: [
              ["Endpoint", "`$XDG_RUNTIME_DIR/handoff-<profile>-keys.sock`, mode 0600", "`\\\\.\\pipe\\handoff-<user>-<profile>-keys`, remote clients rejected"],
              ["Who is asking", "`SO_PEERCRED`, then `/proc` for the executable and working folder", "`GetNamedPipeClientProcessId`, the image path, and the working folder from the PEB"],
            ] },
            { kind: "code", language: "text", code: "→ {\"names\": [\"Dialpad\", \"OpenAI\"]}\n← {\"values\": {\"Dialpad\": \"dp_…\", \"OpenAI\": \"sk-…\"}}\n← {\"error\": \"Not allowed in KeyHandoff.\"}\n" },
            { kind: "list", items: [
              "Names may be bare or the whole pointer (`handoff:Dialpad`); up to 64 per request.",
              "A key not yet allowed for that program and folder waits up to {fact:programWait} for an answer in the window; one program waits at a time.",
              "While KeyHandoff is locked, the request waits up to {fact:unlockWait} for an unlock.",
              "The reply is built in a buffer that never reallocates and is zeroed after sending.",
            ] },
            { kind: "p", text: "To support another language, port one of [the loaders](" + GITHUB + "/tree/main/loaders): read `.env`, collect `handoff:` values, send one request, set the variables in-process." },
          ],
        },
        {
          id: "dev-tests",
          title: "Tests",
          question: "How is KeyHandoff tested?",
          answer: "KeyHandoff's tests run with cargo test in crates/seal and src-tauri, using synthetic keys only. They cover sealing, the vault, whole flows between two identities with a fake clipboard, locking, and the TPM PIN against the swtpm software TPM.",
          blocks: [
            { kind: "code", language: "sh", code: "cd crates/seal && cargo test\ncd src-tauri && cargo test\ncd src-tauri && cargo test -- --ignored --test-threads=1   # real clipboard and keyring checks\n" },
            { kind: "list", items: [
              "Never use real keys in tests or fixtures.",
              "The real clipboard checks need `DISPLAY` and `xclip` on Linux, and an unlocked session on Windows.",
              "TPM tests start a fresh swtpm for each run and skip when it isn't installed.",
              "Each claim in [threats.md](" + GITHUB + "/blob/main/docs/threats.md) links to the test that checks it.",
            ] },
          ],
        },
        {
          id: "dev-rules",
          title: "Design rules",
          question: "What rules does KeyHandoff's design follow?",
          answer: "KeyHandoff's rules: every feature shortens the time a plaintext key is held; encryption is the protection; keys travel sealed to one-time locks; keys are never shown; secrets live only in Rust; a check that can miss is a warning, never a promise.",
          blocks: [
            { kind: "list", ordered: true, items: [
              "Every feature must shorten the time a person or their tools hold a plaintext key.",
              "One flow: card, Safe Copy, sealed file, accept, use.",
              "Encryption is the protection. Sealed files and the vault are ordinary, clearly named files.",
              "Prefer designs that remove a whole class of problem. A check that can miss is a warning, never a promise.",
              "Every key travels sealed to a one-time lock, destroyed when its key is opened, so a sealed file opens once.",
              "No way to reveal a key on screen. The sender keeps nothing. No servers, accounts or telemetry.",
              "Safe Copy listens only when armed, catches one item, times out, and wipes the clipboard and its history.",
              "Secret values live only in Rust; the passphrase and recovery code are the exception that passes through the screens.",
              "No browser extension. Never claim a key is never on the clipboard.",
              "One window, one size, nothing jumping between screens; calm and plain.",
            ] },
            { kind: "p", text: "The full list is [docs/rules/keyhandoff.md](" + GITHUB + "/blob/main/docs/rules/keyhandoff.md)." },
          ],
        },
        {
          id: "dev-security",
          title: "Reporting a security problem",
          question: "How do I report a security problem in KeyHandoff?",
          answer: "Email junovhs@gmail.com rather than opening a public issue, and don't include real keys or other live secrets in the report. KeyHandoff is pre-release and has not had a security audit.",
          blocks: [
            { kind: "p", text: "See [SECURITY.md](" + GITHUB + "/blob/main/SECURITY.md) and the limits already documented in [threats.md](" + GITHUB + "/blob/main/docs/threats.md)." },
          ],
        },
      ],
    },
    // ---------------------------------------------------------------- articles
    {
      id: "articles",
      title: "Articles",
      landing: {
        id: "articles-index",
        title: "Articles",
        question: "Why does KeyHandoff work the way it does?",
        answer: "KeyHandoff's articles explain the thinking behind it: why .env files leak, how a sealed file can open only once, what the vault lock does and doesn't protect, and why it only uses hardware your computer already has.",
        blocks: [
          { kind: "cards", items: [
            { title: "Your .env file is announcing itself", text: "Why plain-text keys are found, and what to do instead.", href: "#why-env-files-leak", label: "Read" },
            { title: "A file that opens once", text: "How one-time locks make the copy in the chat useless.", href: "#one-time-sealed-files", label: "Read" },
            { title: "What a lock can and can't do", text: "The honest version of local encryption.", href: "#what-the-lock-protects", label: "Read" },
            { title: "No hardware to buy", text: "TPM, Windows Hello, and why a passphrase still matters.", href: "#built-in-hardware", label: "Read" },
          ] },
        ],
      },
      children: [
        {
          id: "why-env-files-leak",
          title: "Your .env file is announcing itself",
          question: "Why are API keys in .env files risky?",
          answer: "A .env file is plain text with a predictable name, so anything running as you can find it with one search: a poisoned package, a rogue extension, an infostealer, an AI tool. Ignore rules only keep it out of git. Taking the key out of the file is what helps.",
          keywords: ["dotenv", "supply chain", "infostealer", "secrets"],
          blocks: [
            { kind: "p", text: "Everyone guards keys from being committed. Far fewer guard them from everything else that runs on the machine. A `.env` file sits in every project folder under the same name, which makes it the easiest thing on a developer's computer to find." },
            { kind: "p", text: "Real campaigns did exactly this. In August 2025, poisoned releases of the Nx build system searched for `.env` files, using local AI coding tools as the search party. The Shai-Hulud npm worm, in September 2025, scanned infected machines for secrets and used stolen tokens to spread. In May 2026, a compromised node-ipc release listed `.env.local` and `.env.production` among its targets." },
            { kind: "quote", text: "Ignore rules stop one of these. Take the key out of the file, and a search finds nothing." },
            { kind: "p", text: "One key is never just one key. A Stripe secret reaches customer records and payouts; an AWS key, servers on your bill and whatever is in your buckets; a database URL, every row you have." },
            { kind: "p", text: "KeyHandoff replaces the value with a pointer, `handoff:Dialpad`, and your program asks the app for the key when it starts. The file stays, your code stays, and the secret leaves. See [Pointers in .env](#env-pointers)." },
            { kind: "callout", tone: "note", text: "A pointer doesn't make a program trustworthy: once your code has the key, it can do anything with it. It removes the copy that sits on disk for anything to find." },
          ],
        },
        {
          id: "one-time-sealed-files",
          title: "A file that opens once",
          question: "How can a KeyHandoff sealed file open only once?",
          answer: "Every KeyHandoff key is sealed to a one-time lock whose private half exists only on the receiver's computer. Opening the file destroys that private half, so every other copy of the file, in the chat or a backup, can never be decrypted again.",
          blocks: [
            { kind: "p", text: "Pasting a key into a chat leaves it there: in the scrollback, the search index, the export, every device that synced. Encrypting it with a shared password just moves the problem to the password." },
            { kind: "p", text: "A card fixes this with one-time locks. Each lock is a fresh X25519 public key; its private half stays on the receiver's computer. The sender seals the key to one lock. The receiver opens it, and KeyHandoff destroys that private half before the key even reaches the vault." },
            { kind: "compare", before: { title: "A pasted key", text: "Readable by anyone who can read the chat, for as long as the chat exists." }, after: { title: "A sealed file", text: "Readable once, by one computer. After that, every copy is noise." } },
            { kind: "p", text: "The seal is bound to the exact lock, identity and name it was made for, signed by the sender, and fails if any byte changes. Armored text repairs the damage chat apps do to pasted text, and refuses anything it can't repair, rather than misreading it." },
            { kind: "p", text: "The honest limits: the first time you open a card, you're trusting the channel it came through. Check the computer fingerprint if it matters. And a card copy is for one person: if two people use the same copy, they can collide on a lock, and the second key won't open." },
          ],
        },
        {
          id: "what-the-lock-protects",
          title: "What a lock can and can't do",
          question: "What does locking the KeyHandoff vault protect against?",
          answer: "While KeyHandoff is locked, a program running as you can read every file and still can't open a key without your passphrase. While it's unlocked, that program could reach the vault. Locking on sleep and screen lock keeps the open window short.",
          blocks: [
            { kind: "p", text: "Many apps encrypt their data with a key the operating system hands to any program running as you: Windows' DPAPI, a desktop keyring. That stops someone who steals the disk. It doesn't stop malware already running as you, which is what infostealers are." },
            { kind: "p", text: "So KeyHandoff's vault key is wrapped by something only you can give: a passphrase, hashed with Argon2id so each guess is slow. A copy of the files is useless without it; five random words are out of reach for any guesser, though a short, common passphrase isn't." },
            { kind: "table", head: ["", "Locked or closed", "Unlocked"], rows: [
              ["Reads every KeyHandoff file", "Gets ciphertext", "Gets ciphertext"],
              ["Asks for a key like an allowed program", "Waits for you", "May get it"],
              ["Reads KeyHandoff's memory", "Nothing to find", "Could find the vault key"],
              ["Logs your keyboard", "Could catch the passphrase", "Could catch the passphrase"],
            ] },
            { kind: "p", text: "That's why KeyHandoff locks again whenever you step away: on sleep, on screen lock, from the lock button. **Remember on this computer** gives this up on purpose, for people who'd rather not type anything, and says so before you turn it on." },
            { kind: "p", text: "No app can protect a secret on a computer an attacker controls as an administrator. The lock narrows what an ordinary infostealer can take; the rest is in [What it protects](#limits)." },
          ],
        },
        {
          id: "built-in-hardware",
          title: "No hardware to buy",
          question: "Does KeyHandoff need a TPM or a security key?",
          answer: "No. KeyHandoff works on any computer with a passphrase. Where the computer already has hardware, it can use it: a TPM PIN on Linux now, Windows Hello planned. It never asks anyone to buy a security key.",
          blocks: [
            { kind: "p", text: "A TPM is often described as real security and software encryption as fake. The difference that matters is whether a prompt you answer stands between the vault and other programs. A TPM key that any program can use without asking is no better than DPAPI. A TPM key behind a PIN is better, because the TPM counts wrong guesses and the key can't be copied off the machine." },
            { kind: "p", text: "But many computers have no usable TPM. Firmware TPMs ship switched off on lots of desktop boards built before Windows 11, and on Linux a normal user can't open the device until they join the tss group. So KeyHandoff's baseline is the passphrase, which works everywhere, and hardware adds a faster way in where it exists." },
            { kind: "metrics", items: [
              { value: "About 3 in 4", label: "Windows PCs with a usable TPM", detail: "Estimate, from Windows 11 adoption." },
              { value: "5 to 20 in 100", label: "Linux desktops a normal app can use it on", detail: "Estimate: the TPM is often off, or limited to the tss group." },
            ] },
            { kind: "p", text: "Buying a USB security key would add protection for the few who own one, and a reason to give up for everyone else. KeyHandoff only uses what's already there: the passphrase, the TPM, and later Windows Hello and the Mac's Secure Enclave." },
          ],
        },
      ],
    },
  ],
});
