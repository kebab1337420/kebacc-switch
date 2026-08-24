---
description: Saved accounts, fresh numbers and time to reset
allowed-tools: Bash(~/.claude-tools/kebacc:*), Bash(~/.claude-tools/kebacc.exe:*)
argument-hint: "[-ag|-claude|-codex|-opencode|-all]"
---

Run `~/.claude-tools/kebacc`, or `~/.claude-tools/kebacc.exe` on Windows. Pass the argument through as pool flags (`-ag`, `-claude`, `-codex`, `-opencode`, `-all`). No argument lists every pool.

```
~/.claude-tools/kebacc list -Refresh -Countdown $ARGUMENTS
```

The numbers come from the API, not from the cache. One block per pool. Repeat the lines as they are; the `*` marks the account in use. This only lists: it never switches, whatever the numbers say. The OpenCode block carries no numbers: that provider publishes none.
