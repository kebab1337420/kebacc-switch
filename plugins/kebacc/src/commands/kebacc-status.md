---
description: What is live, what it has left, what is armed
allowed-tools: Bash(~/.claude-tools/kebacc:*), Bash(~/.claude-tools/kebacc.exe:*)
argument-hint: "[-ag|-claude|-codex|-grok|-all]"
---

Run `~/.claude-tools/kebacc`, or `~/.claude-tools/kebacc.exe` on Windows. Pass the argument through as pool flags (`-ag`, `-claude`, `-codex`, `-grok`, `-all`). No argument covers every pool.

```
~/.claude-tools/kebacc status $ARGUMENTS
```

One line per pool: the login in use and what is left of its quota, from the cache. Then whether the auto-switch is armed, whether the background watcher is up, and when the last switch happened. Add `-Refresh` to ask the API instead of reading the cache.

The Grok line names the login and stops there, since that provider publishes no usage.

Repeat the lines as they are. This only reports: it never switches.
