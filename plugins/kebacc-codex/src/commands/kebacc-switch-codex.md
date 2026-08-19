---
description: Switch Codex to another saved account
allowed-tools: Bash(~/.claude-tools/kebacc-codex:*), Bash(~/.claude-tools/kebacc-codex.exe:*)
---

Run `~/.claude-tools/kebacc-codex`, or `~/.claude-tools/kebacc-codex.exe` on Windows:

```
~/.claude-tools/kebacc-codex switch -Provider codex -Email <email>
```

Ask which account first if the user did not name one — run `/kebacc-list-codex` to show the choices. Tell them to restart the CLI afterwards. If it answers that the account is not trusted it is waiting for a yes or no, so ask the user before answering.
