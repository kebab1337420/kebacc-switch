---
description: Check the install, the pools and the session hook
allowed-tools: Bash(~/.claude-tools/kebacc:*), Bash(~/.claude-tools/kebacc.exe:*)
argument-hint: "[-ag|-claude|-codex|-opencode|-all]"
---

Run `~/.claude-tools/kebacc`, or `~/.claude-tools/kebacc.exe` on Windows. Pass the argument through as pool flags. No argument checks every pool.

```
~/.claude-tools/kebacc doctor $ARGUMENTS
```

Report the `!` and `~` lines. Plain-text snapshots are fixed with `doctor -ag -Protect` (or `-claude`, `-codex`), unstamped ones with `-Adopt`. A login whose token has run out takes `-Renew`. The credentials from before the last switch come back with `-Rollback`.

A switch that ends in a login prompt leaves its reason in `~/.kebacc-switch/kebacc.log`: read the last lines of that file before saying anything else about it.
