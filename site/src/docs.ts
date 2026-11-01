import { defineDocs, defineFacts } from "dopedocs";
import { reference } from "./reference-docs.ts";

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
  title: "User manual",
  lead: "Hand off API keys without exposing them. This is the user manual; the developer reference and the articles are a click away.",
  facts: defineFacts({
    platforms: { value: "Windows and Linux (X11)", reviewed: REVIEWED },
    status: { value: "pre-release and has not had a security audit", reviewed: REVIEWED },
    network: { value: "no servers, accounts or telemetry; its only network request checks for a signed update", reviewed: REVIEWED },
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
              { title: "Sharing keys", text: "Sending a key, keeping one, teammates and teams.", href: "#how-it-works", label: "Share" },
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
              answer: "Download KeyHandoff from keyhandoff.dev: a Windows installer, or a .deb or AppImage for Linux (X11). Installed copies update themselves with signed updates.",
              keywords: ["install", "build", "download", "update"],
              blocks: [
                { kind: "p", text: "Get the latest version from the [download section](https://keyhandoff.dev/#download) on the home page or from [GitHub releases](" + GITHUB + "/releases). KeyHandoff runs on {fact:platforms}." },
                {
                  kind: "steps",
                  items: [
                    { title: "Windows 10 or 11", text: "Run `keyhandoff-…-windows-x64-setup.exe`. It installs for you only, without asking for admin rights. It isn't code-signed yet, so SmartScreen may ask: choose **More info**, then **Run anyway**." },
                    { title: "Ubuntu, Debian and friends", text: "Run `sudo apt install ./keyhandoff-…-linux-amd64.deb`. It needs WebKitGTK 4.1, which apt installs with it." },
                    { title: "Other Linux", text: "Make `keyhandoff-…-linux-x86_64.AppImage` executable (`chmod +x`) and run it. KeyHandoff needs X11; Wayland isn't supported yet." },
                  ],
                },
                { kind: "p", text: "Double-clicking a `.handoff` file opens it in KeyHandoff with the Windows installer and the .deb. Check a download against the release's `SHA256SUMS` if you like." },
                { kind: "p", text: "**Updates:** KeyHandoff checks for a new version when it starts and every six hours, and installs only updates signed with its release key. While it's locked, it installs and restarts on its own. While it's open, **Restart to update** appears in the title bar. A .deb never installs on its own, because installing it asks for your password: use Restart to update. The check is KeyHandoff's only network request." },
                { kind: "p", text: "**From source:** install [Rust](https://rustup.rs), [Node.js](https://nodejs.org) and the [Tauri prerequisites](https://tauri.app/start/prerequisites/), then run the commands below. A copy built from source never checks for updates." },
                { kind: "code", language: "sh", code: "git clone " + GITHUB + "\ncd keyhandoff\nnpm install\nnpm run tauri dev                     # run it\nnpm run tauri build -- --no-bundle    # just the binary\n" },
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
              answer: "In KeyHandoff, the sender picks the receiver from their teammates, picks a key already in their vault, and sends the sealed file KeyHandoff makes. The receiver opens it once to keep the key in their own vault.",
              blocks: [
                {
                  kind: "steps",
                  items: [
                    { title: "Pick a teammate", text: "Ben clicks **Send a key** and picks Ann from his list." },
                    { title: "Seal the key", text: "He picks the key from his saved keys. KeyHandoff seals it so only Ann's computer can open it, once, and he sends her the file." },
                    { title: "Keep", text: "Ann opens the file. The key goes into her vault, and every copy of the sealed file becomes permanently unreadable." },
                  ],
                },
                { kind: "p", text: "To be on Ben's list, Ann sends him her card once. That's [adding a teammate](#your-card), a separate one-time step." },
                { kind: "p", text: "Slack, Teams, email: anything that carries a file works, and the file only ever holds ciphertext. When a channel only takes text, paste the file's text instead; it repairs itself if the chat mangles it." },
              ],
            },
            {
              id: "your-card",
              title: "Add a teammate",
              question: "How do I add a teammate in KeyHandoff?",
              answer: "To be on someone's KeyHandoff list, send them your card once: a file with your computer's public identity and {fact:cardLocks} one-time locks. They open it, and from then on they can send you keys. It holds no secret.",
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
                { kind: "p", text: "**In the same room?** Both of you click **Add someone nearby** in Send a key, on the same network. Pick each other, and both screens show a six-digit code. If the codes match, click **Yes, add**: you're on each other's lists, and you can send keys straight across with Nearby. If they don't match, choose **No**: something on the network got in the middle." },
                { kind: "p", text: "**When someone sends you their card,** open it: double-click it, drop it on the window, or press Ctrl V in KeyHandoff. **Card added** shows their name and computer fingerprint, and they're on your Send a key list." },
                { kind: "p", text: "When someone opens your card, you become one of their contacts and they see how many keys they can still send you. When two or fewer are left, Send a key marks you in amber, on your row and on any team you're in, and the send that uses the last one says to ask you for a new card." },
                { kind: "p", text: "**To forget a contact,** point at them in Send a key, click the cross and then **Forget for good?**. Their locks go and they come off your teams; a team left with nobody goes too. Keys you already sent them stay with them." },
              ],
            },
            {
              id: "send-a-key",
              title: "Send a key",
              question: "How do I send someone a key with KeyHandoff?",
              answer: "In KeyHandoff, click Send a key, pick the teammate, then pick one of your saved keys. KeyHandoff seals it for them and gives you the file to send. Nothing is copied from anywhere.",
              blocks: [
                {
                  kind: "steps",
                  items: [
                    { title: "Pick them", text: "Click **Send a key** and choose the person, or a team. Not on your list yet? [Add them as a teammate](#your-card) first." },
                    { title: "Pick the key", text: "Choose one of your saved keys. The window shows **Sealed** and the file `Key for <name>.handoff`." },
                    { title: "Send the file", text: "Click **Copy** and paste it into the chat. When you close the window, the sealed file is deleted; your own saved key stays in your keys." },
                  ],
                },
                { kind: "p", text: "Only keys you've saved can be sent. To save one, use [Add a key](#add-a-key)." },
                { kind: "p", text: "**On the same network?** Click **Find people on this network** in Send a key. Contacts whose KeyHandoff is open with Nearby on show **Nearby**, and the Sealed screen offers **Send to Ann's computer**: the key goes straight there, with no file in the chat. Only people on your list can send you keys this way. Nearby stops when KeyHandoff locks." },
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
              answer: "A KeyHandoff team is a named group of your contacts. Sending a saved key to a team seals a separate copy for each member in one file, and files your key under the team. Welcome sends a newcomer every key shared with the team.",
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
            {
              id: "team-folders",
              title: "Team folders",
              question: "How do I keep a team's keys in sync with KeyHandoff?",
              answer: "Keep the team in a folder everyone already syncs, like Dropbox, Google Drive, OneDrive, a repo or a network share. Members join from the invite in the folder, team keys reach them through it, and when you replace a key, everyone gets the new one and their programs keep working.",
              blocks: [
                {
                  kind: "steps",
                  items: [
                    { title: "Make the team", text: "In **Send a key**, click **New team**, then **Keep it in a shared folder** and pick the folder. KeyHandoff puts `invite.handoff` in it." },
                    { title: "People join", text: "Each person opens `invite.handoff` in their copy of the folder and clicks **Join**. You see **Ann wants to join** in Send a key, with her computer's fingerprint; click **Add**." },
                    { title: "Share keys", text: "Pick the team in Send a key, then a saved key. Everyone on the team gets it the next time their KeyHandoff sees the folder." },
                    { title: "Rotate", text: "Made a new key with the provider? Click **Replace** on the old one and add the new value [the way you add any key](#add-a-key). Everyone's copy updates under the same name, so `.env` pointers and allowed programs keep working." },
                  ],
                },
                { kind: "p", text: "**Taking someone off:** in Send a key, click the cross next to them, then **Take off**. They get nothing new from the team, and you see the team keys they had. Replace each with the provider, then click **Replace** on it; it leaves the list and everyone still on the team gets the new one. Their KeyHandoff says they were taken off." },
                { kind: "p", text: "KeyHandoff never syncs anything itself; the folder's own app does. Everything in the folder is sealed: who's on the team and when it changed is visible, never the keys or their names. Only the person who made the team can add people or change its keys, and nobody joins until they're added." },
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
                { kind: "p", text: "This is how a key gets into KeyHandoff in the first place. Once it's saved, you can [send it](#send-a-key) to anyone on your list without copying it again." },
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
                { kind: "p", text: "Once the keys are loaded, a key your program prints shows as `[KeyHandoff: Dialpad hidden]`, whether it's printed plain, base64, hex, URL-encoded or backwards. In Python that covers everything the process and the programs it starts write to the terminal; in Node.js, what goes through `process.stdout` and `process.stderr`. Pass `mask=False` (Python) or `{ mask: false }` (Node.js) to turn it off." },
                { kind: "callout", tone: "note", text: "Python and JavaScript strings can't be wiped, so the value stays in your program's memory until it exits." },
                { kind: "p", text: "Every parameter, return value and error is in the developer reference: [Python](#py-load), [Node.js](#js-load), and [the socket protocol](#socket-endpoint) for other languages." },
              ],
            },
            {
              id: "keyhandoff-run",
              title: "keyhandoff run",
              question: "How do I give a key to a program without changing its code?",
              answer: "Start it with keyhandoff run, as in keyhandoff run -- python report.py. KeyHandoff fills the handoff: pointers in .env and the environment, starts the command with the keys, and hides any key it prints.",
              keywords: ["run", "cli", "command", "any language", "op run", "doppler run"],
              blocks: [
                { kind: "code", language: "sh", code: "keyhandoff run -- python report.py\nkeyhandoff run -- npm test\nkeyhandoff run --profile work -- go run ./cmd/server\n" },
                { kind: "p", text: "It works for any language and any tool: nothing in your project needs to change beyond the `.env` pointers. The approval is for that exact command: the program, its arguments, and the contents of the files it names. Edit the script, or run something else with the same interpreter, and KeyHandoff asks again." },
                { kind: "p", text: "Everything the command and the programs it starts print to the terminal passes through KeyHandoff on the way, so a key printed plain, base64, hex, URL-encoded or backwards shows as `[KeyHandoff: Stripe hidden]`." },
                { kind: "p", text: "`--env` picks another file than `.env`, and `--profile` another profile. The command's exit code is passed back." },
              ],
            },
            {
              id: "allowing-programs",
              title: "Allowing a program",
              question: "Why does KeyHandoff ask me to allow a program?",
              answer: "The first time a command asks KeyHandoff for a key, the window shows the command and the folder it runs in. Always allow covers that command, in that folder, for those keys, until the script changes. Just this once and Deny are the other choices.",
              blocks: [
                { kind: "p", text: "KeyHandoff asks the operating system which program is asking and where it runs, and shows the command line so you can recognise the script. The approval is bound to the command and the files it names, because allowing `python` everywhere would allow every script, including a one-off `python -c` an AI assistant writes." },
                { kind: "p", text: "**AI coding agents.** When Claude Code, Cursor, Codex, GitHub Copilot, Aider, Windsurf or Gemini CLI started the command, KeyHandoff always asks, names the agent, and offers **Allow for 30 minutes** instead of Always. Read the command before you allow it: that's exactly what gets the key." },
                { kind: "p", text: "A program waits up to {fact:programWait} for your answer, then is refused. Forgetting a key removes its approvals." },
                { kind: "callout", tone: "warn", text: "The prompt is a decision, not a lock: anything running as you could run the same command in the same folder. And a program that has a key can do anything with it." },
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
                    "**After a quiet spell:** off by default. Under Security, pick **Lock after**: 15 minutes or 1 hour. It applies straight away.",
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
                { kind: "compare", before: { title: "What holds", text: "A sealed file opens once, on one computer. Sending leaves nothing new behind. Keys are encrypted on disk and never shown. While locked, no program can open them. No servers or accounts." },
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
  ],
  reference,
  articles: [
        {
          id: "why-env-files-leak",
          title: "Your .env file is announcing itself",
          published: "2026-10-31",
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
          published: "2026-10-31",
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
          published: "2026-10-31",
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
          published: "2026-10-31",
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
        }
  ],
});
