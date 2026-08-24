---
description: Arm or disarm the auto-switch
allowed-tools: Bash(~/.claude-tools/kebacc:*), Bash(~/.claude-tools/kebacc.exe:*)
argument-hint: "[-ag|-claude|-codex|-opencode|-all|off]"
---

Run `~/.claude-tools/kebacc`, or `~/.claude-tools/kebacc.exe` on Windows. Pass the argument through (`-ag`, `-claude`, `-codex`, `-opencode`, `-all`, `off`). No argument arms every pool.

```
~/.claude-tools/kebacc arm $ARGUMENTS
```

This only writes the hooks. It never changes the account in use. Print the one line it prints and stop.

OpenCode publishes no usage, so there is nothing for the switch to decide there. Arming it is allowed and harmless — the check passes over that pool — but its accounts are moved by hand, with `/kebacc-switch -opencode`.
