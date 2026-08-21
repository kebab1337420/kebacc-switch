---
description: Switch Antigravity to another saved account
allowed-tools: Bash(~/.claude-tools/kebacc-antigravity:*), Bash(~/.claude-tools/kebacc-antigravity.exe:*)
---

Run `~/.claude-tools/kebacc-antigravity`, or `~/.claude-tools/kebacc-antigravity.exe` on Windows:

```
~/.claude-tools/kebacc-antigravity switch -Provider antigravity -Email <email>
```

Ask which account first if the user did not name one — run `/kebacc-list-antigravity` to show the choices. Tell them to restart the CLI afterwards. If it answers that the account is not trusted it is waiting for a yes or no, so ask the user before answering.
