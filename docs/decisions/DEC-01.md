+++
date = "2026-11-13"
evidence = ["SITE-22", "base:0fd767dbd0c3b86a29f9d0ca01fd069f7ccc383c"]
id = "DEC-01"
revised_at = "2026-11-13T19:24:18.6776075Z"
scope = ["LICENSE", "NOTICE", "TRADEMARKS.md", "README.md", "site/"]
status = "accepted"
superseded_by = []
supersedes = []
title = "KeyHandoff is open source under Apache 2.0; credit is protected by the NOTICE and the trademark"

[author]
agent = "agent-30288"
person = "Spencer Nunamaker"
+++

# DEC-01: KeyHandoff is open source under Apache 2.0; credit is protected by the NOTICE and the trademark

## Context

KeyHandoff was under PolyForm Shield 1.0.0, chosen out of fear that someone would fork it, put money behind a lookalike name and take the credit. The owner doesn't intend to sell it: the goal is to be free, everywhere and the standard recommendation, like VLC, including at big companies. Those companies' legal teams usually pre-approve only OSI licenses, and PolyForm Shield isn't one, so an engineer who just wants to install the tool hits a legal review and gives up.

## Decision

The app and crates/seal are under Apache 2.0. NOTICE says Spencer Nunamaker created KeyHandoff, and Apache 2.0 requires every fork to keep it. TRADEMARKS.md says 'KeyHandoff' and the logo are trademarks: forks must use a different, not confusingly similar name and logo, and can't claim to be official. Register the trademark once adoption makes a lookalike plausible (SITE-21).

## Why

A license doesn't protect credit; the name does. Shield doesn't stop a free fork called 'KeyHand0ff' any better than Apache does, while trademark law covers confusingly similar names under any license. Apache 2.0 removes the legal-review barrier for companies and legally requires attribution through NOTICE. Famous hostile forks (OpenTofu, Valkey) happened because originals moved to restrictive licenses. The owner's real moat is being the obvious source: the domain, the repo, signed releases, the paper.

## Alternatives rejected

Keep PolyForm Shield: blocks big-company adoption and buys little, since nothing is sold. MIT: also OSI and simple, but has no NOTICE mechanism and no explicit patent grant. GPL: OSI, but copyleft worries some company legal teams more than Apache.

## Consequences

Anyone may use, change and ship KeyHandoff, commercially too. Forks must carry NOTICE and rename. KeyHandoff can now apply for funded audits (SITE-18). Changing back later wouldn't affect copies already released under Apache 2.0.
