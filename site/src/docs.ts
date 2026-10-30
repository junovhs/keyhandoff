import { defineDocs, defineFacts } from "dopedocs";

export const docs = defineDocs({
  entity: {
    name: "KeyHandoff",
    url: "https://keyhandoff.dev",
    notToBeConfusedWith: ["Other products and services named KeyHandoff or Key Handoff"],
  },
  title: "KeyHandoff",
  lead: "Hand off API keys without exposing them.",
  facts: defineFacts({
    platforms: { value: "Windows and Linux (X11)", reviewed: "2026-10-30" },
    status: { value: "pre-release and has not had a security audit", reviewed: "2026-10-30" },
    network: { value: "no servers, accounts, telemetry or network code", reviewed: "2026-10-30" },
  }),
  sections: [
    {
      id: "what-is-it",
      title: "What is KeyHandoff?",
      question: "What is KeyHandoff?",
      answer: "KeyHandoff is a small desktop app for handing someone an API key, or any other secret, without pasting it into a chat or leaving it in a plain file. It has {fact:network}.",
      blocks: [
        { kind: "p", text: "Requests and keys travel as ordinary files over whatever channel you already use. The key is never displayed on screen." },
        { kind: "callout", tone: "warn", text: "KeyHandoff is {fact:status}. Use it with test keys for now." },
      ],
    },
    {
      id: "how-it-works",
      title: "How it works",
      question: "How does KeyHandoff hand off a key?",
      answer: "The receiver sends a request file, the sender catches the key from the provider's page and seals it, and the receiver opens the sealed file once to keep the key in a vault.",
      blocks: [
        {
          kind: "steps",
          items: [
            { title: "Ask", text: "Click Request a key and send the request file to whoever has the key." },
            { title: "Catch", text: "They copy the key from the provider's page. KeyHandoff catches that one copy and wipes the clipboard." },
            { title: "Seal", text: "They name the key and it's encrypted so only your computer can open it, and only once. They send you the file and keep nothing." },
            { title: "Keep", text: "You open the file. The key goes into your vault, and every copy of the sealed file becomes permanently unreadable." },
          ],
        },
        { kind: "p", text: "You can also catch a key for yourself with Add your own, which involves no files." },
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
      id: "limits",
      title: "What it protects, and what it doesn't",
      question: "What does KeyHandoff not protect against?",
      answer: "KeyHandoff cannot protect against a compromised computer, malware running as you, or another program reading the clipboard in the moment before it is wiped.",
      blocks: [
        {
          kind: "list",
          items: [
            "The copy moment: a copied key is on the clipboard, unencrypted, until the wipe.",
            "Malware running as you can ask the operating system to unlock the vault.",
            "Lost computers: keys are tied to one computer and there is no recovery. Replace the key with the provider.",
          ],
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
  ],
});
