---
description: Switch Claude Code to another saved account
allowed-tools: Bash(~/.claude-tools/kebacc:*), Bash(~/.claude-tools/kebacc.exe:*)
---

Run `~/.claude-tools/kebacc`, or `~/.claude-tools/kebacc.exe` on Windows:

```
~/.claude-tools/kebacc switch -Provider claude -Email <email>
```

Ask which account first if the user did not name one — run `/kebacc-list-claude` to show the choices. If it answers that the account is not trusted it is waiting for a yes or no, so ask the user before answering.

Once the switch has gone through, read the quotas again so the saved numbers are not the stale ones the switch was decided on:

```
~/.claude-tools/kebacc refresh -Provider claude
```

It prints nothing. Report the account you moved to, and tell them to restart the CLI for it to pick the change up.
