---
description: Install the newest release of the Antigravity switcher
allowed-tools: Bash(~/.claude-tools/kebacc-antigravity:*), Bash(~/.claude-tools/kebacc-antigravity.exe:*)
---

Check first, and only install if there is something newer:

```
~/.claude-tools/kebacc-antigravity update -Check
```

If it reports a newer version, run:

```
~/.claude-tools/kebacc-antigravity update
```

Report the version it moved to. The saved Antigravity logins are not touched by an
update, and the Claude half is a separate program this leaves alone.
