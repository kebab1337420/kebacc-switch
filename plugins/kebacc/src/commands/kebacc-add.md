---
description: Save the login you are on into a pool
allowed-tools: Bash(~/.claude-tools/kebacc:*), Bash(~/.claude-tools/kebacc.exe:*)
argument-hint: "-ag|-claude|-codex|-grok|-opencode"
---

Run `~/.claude-tools/kebacc`, or `~/.claude-tools/kebacc.exe` on Windows. The argument names the pool (`-ag`, `-antigravity`, `-claude`, `-cc`, `-codex`, `-cx`, `-grok`, `-opencode`). One pool is required.

```
~/.claude-tools/kebacc add $ARGUMENTS
```

Report the account it saved. An API key has no email attached, so if it asks for one, ask the user for it and pass `-Email`.
