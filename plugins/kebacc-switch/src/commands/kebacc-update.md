---
description: Install the newest release of the switcher
allowed-tools: Bash(~/.claude-tools/kebacc-switch:*), Bash(~/.claude-tools/kebacc-switch.exe:*)
---

Check first, and only install if there is something newer:

```
~/.claude-tools/kebacc-switch update -Check
```

If it reports a newer version, run:

```
~/.claude-tools/kebacc-switch update
```

Report the version it moved to. The saved accounts are not touched by an update.
