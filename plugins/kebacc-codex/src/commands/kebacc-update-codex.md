---
description: Install the newest release of the Codex switcher
allowed-tools: Bash(~/.claude-tools/kebacc-codex:*), Bash(~/.claude-tools/kebacc-codex.exe:*)
---

Check first, and only install if there is something newer:

```
~/.claude-tools/kebacc-codex update -Check
```

If it reports a newer version, run:

```
~/.claude-tools/kebacc-codex update
```

Report the version it moved to. The saved Codex logins are not touched by an
update, and the Claude half is a separate program this leaves alone.
