import { defineDocs, defineFacts } from "dopedocs";

/** The Strange Systems glyph, for the maker card: the same vectors No Ceremony's uses. */
const STRANGE_SYSTEMS = `<svg viewBox="0 0 505 474" xmlns="http://www.w3.org/2000/svg" fill="currentColor">
  <path d="M0,0 L0,411.878 L216.914,411.878 L216.914,350.713 L62.937,350.713 L62.937,61.163 L216.914,61.163 L216.914,0 Z" />
  <path d="M216.914,153.657 L288.081,122.522 L288.081,319.256 L216.914,350.391 Z" />
  <path d="M505,61.503 L505,473.381 L288.081,473.381 L288.081,412.216 L442.058,412.216 L442.058,122.666 L288.081,122.666 L288.081,61.503 Z" />
</svg>`;

export const docs = defineDocs({
  entity: {
    name: "KeyHandoff",
    url: "https://keyhandoff.dev",
    legalName: "Strange Systems",
    notToBeConfusedWith: ["Other products and services named KeyHandoff or Key Handoff"],
  },
  title: "KeyHandoff",
  lead: "Hand off API keys without exposing them.",
  facts: defineFacts({
    platforms: { value: "Windows and Linux (X11)", reviewed: "2026-10-31" },
    status: { value: "pre-release and has not had a security audit", reviewed: "2026-10-31" },
    network: { value: "no servers, accounts, telemetry or network code", reviewed: "2026-10-31" },
  }),
  sections: [
    {
      id: "what-is-it",
      title: "What is KeyHandoff?",
      question: "What is KeyHandoff?",
      answer: "KeyHandoff is a small desktop app for handing someone an API key, or any other secret, without pasting it into a chat or leaving it in a plain file. It has {fact:network}.",
      blocks: [
        { kind: "p", text: "Cards and keys travel as ordinary files over whatever channel you already use. The key is never displayed on screen." },
        { kind: "callout", tone: "warn", text: "KeyHandoff is {fact:status}. Use it with test keys for now." },
      ],
    },
    {
      id: "how-it-works",
      title: "How it works",
      question: "How does KeyHandoff hand off a key?",
      answer: "The receiver sends their card once. After that, the sender picks them in Send a key, catches the key from the provider's page and seals it, and the receiver opens the sealed file once to keep the key in their vault.",
      blocks: [
        {
          kind: "steps",
          items: [
            { title: "Share your card", text: "Click Your card and send the file to whoever will send you keys. It holds no secret, and each copy is good for 12 keys." },
            { title: "Catch", text: "They click Send a key, pick you, and copy the key from the provider's page. KeyHandoff catches that one copy and wipes the clipboard." },
            { title: "Seal", text: "They name the key and it's encrypted so only your computer can open it, and only once. They send you the file and keep nothing." },
            { title: "Keep", text: "You open the file. The key goes into your vault, and every copy of the sealed file becomes permanently unreadable." },
          ],
        },
        { kind: "p", text: "Sending to a team catches the key once and seals a copy for every member, in one file. You can also catch a key for yourself with Add a key, which involves no files." },
      ],
    },
    {
      id: "safe-copy",
      title: "Safe Copy",
      question: "When does KeyHandoff read my clipboard?",
      answer: "Safe Copy reads the clipboard only after you ask it to. It catches one item, then switches off, or times out after 60 seconds.",
      blocks: [{ kind: "p", text: "Nothing watches the clipboard the rest of the time." }],
    },
    {
      id: "locking",
      title: "Locking",
      question: "How is the vault locked?",
      answer: "KeyHandoff starts locked, and your passphrase opens it. It locks again when the computer sleeps or the screen locks. A recovery code you write down at setup always gets you back in.",
      blocks: [
        { kind: "p", text: "While it's locked, a program running as you can read every KeyHandoff file and still can't open a key. A program that asks for a key while it's locked brings the window forward and waits for you." },
        { kind: "p", text: "In Security, at the bottom of the home screen, you can unlock with a short PIN instead, where the computer has a TPM KeyHandoff can use (Linux for now). The TPM checks the PIN and stops anyone guessing it. KeyHandoff only uses hardware your computer already has." },
        { kind: "callout", tone: "warn", text: "Remember on this computer opens KeyHandoff without asking. It's off unless you turn it on, because any program running as you could then open your keys." },
      ],
    },
    {
      id: "limits",
      title: "What it protects, and what it doesn't",
      question: "What does KeyHandoff not protect against?",
      answer: "KeyHandoff can't stop a program watching the clipboard from reading a key at the moment you copy it, or stop malware running as you from reaching the vault while it's unlocked. A program you allow can do anything with the key it gets.",
      blocks: [
        {
          kind: "p",
          text: "The full list, with the test or design rule behind each claim, is in [What KeyHandoff protects, and what it doesn't](https://github.com/junovhs/keyhandoff/blob/main/docs/threats.md).",
        },
      ],
    },
    {
      id: "platforms",
      title: "Platforms",
      question: "Which platforms does KeyHandoff support?",
      answer: "KeyHandoff supports {fact:platforms}. macOS and Wayland are not supported.",
      blocks: [],
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
});
