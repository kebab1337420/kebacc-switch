---
description: Install the newest release of the switcher
allowed-tools: Bash(~/.claude-tools/kebacc:*), Bash(~/.claude-tools/kebacc.exe:*)
---

Check first, and only install if there is something newer:

```
~/.claude-tools/kebacc update -Check
```

If it reports a newer version, run:

```
~/.claude-tools/kebacc update
```

Report the version it moved to. The saved accounts are not touched by an update.
