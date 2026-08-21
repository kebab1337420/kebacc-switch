---
description: Switch Antigravity to another saved account
allowed-tools: Bash(~/.claude-tools/kebacc:*), Bash(~/.claude-tools/kebacc.exe:*)
---

Run `~/.claude-tools/kebacc`, or `~/.claude-tools/kebacc.exe` on Windows:

```
~/.claude-tools/kebacc switch -Provider antigravity -Email <email>
```

Ask which account first if the user did not name one. Run `/kebacc-list-antigravity` to show the choices. Tell them to restart the CLI afterwards. If it answers that the account is not trusted it is waiting for a yes or no, so ask the user before answering.
