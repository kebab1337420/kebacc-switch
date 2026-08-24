---
description: Forget a saved account
allowed-tools: Bash(~/.claude-tools/kebacc:*), Bash(~/.claude-tools/kebacc.exe:*)
argument-hint: "-ag|-claude|-codex|-opencode [email]"
---

Run `~/.claude-tools/kebacc`, or `~/.claude-tools/kebacc.exe` on Windows. The argument names the pool (`-ag`, `-claude`, `-codex`, `-opencode`) and the email to forget.

```
~/.claude-tools/kebacc remove $ARGUMENTS -Yes
```

Report which account it forgot. The live session is untouched.
