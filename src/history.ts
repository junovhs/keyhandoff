/**
 * What changed, newest first, in the words someone using KeyHandoff would
 * use. Shown by clicking the logo in the title bar. Add to the top when a
 * change someone would notice lands.
 */
export type Release = { date: string; title: string; notes: string[] };

export const HISTORY: Release[] = [
  {
    date: "2026-11-22",
    title: "On the Mac",
    notes: [
      "KeyHandoff runs on Macs, Intel and Apple silicon, with the same vault, cards, teams and Safe Copy.",
      "Programs and AI agents asking for a key are named from macOS itself; cron jobs and launchd agents always ask.",
      "On a Mac, Type works once you allow KeyHandoff under Privacy & Security, Accessibility.",
      "Safe Copy lists what it checked while it listens.",
      "A sealed key shows as a card you can drag into a chat, an email or a folder.",
    ],
  },
  {
    date: "2026-11-19",
    title: "Moves with you",
    notes: [
      "Your card is holographic, card-shaped, and tips toward the pointer.",
      "Pearl buttons and picked options light up where the pointer is.",
      "Wide windows show keys as tiles, two or three across, without jumping between sizes.",
      "Every screen, sheet and dialog eases in and out.",
      "Settings links to keyhandoff.dev and Strange Systems. This history opens from the logo.",
    ],
  },
  {
    date: "2026-11-18",
    title: "Black and pearl",
    notes: [
      "A slick black look with a pearl accent on every screen.",
      "On Linux (X11), a panel types a saved key for you.",
      "The clipboard listener is harder to fool.",
    ],
  },
  {
    date: "2026-11-17",
    title: "A vault built around your keys",
    notes: [
      "One line per key, with clear actions to use it or hand it off.",
      "Receiving cards: share a .handoff file so people can send you keys.",
      "An agent can reuse one approval for a key in one folder; see its recent commands and revoke it.",
    ],
  },
  {
    date: "2026-11-13",
    title: "Open source",
    notes: ["KeyHandoff is open source under Apache 2.0."],
  },
  {
    date: "2026-11-12",
    title: "Import a .env",
    notes: [
      "Import a .env file: its keys are sealed in the vault and pointers are left behind.",
      "On Windows, Node.js programs get stand-in keys, never the real one.",
      "The keyhandoff command is on PATH and works from any Windows terminal.",
    ],
  },
  {
    date: "2026-11-02",
    title: "Tighter by default",
    notes: [
      "Server restrictions you set stay in force when a key is handed to a run.",
      "Local connections are bounded and incomplete requests are refused.",
    ],
  },
];
