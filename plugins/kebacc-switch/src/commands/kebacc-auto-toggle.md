---
description: Turn the auto-switch on or off, any time, no conditions
allowed-tools: Bash(~/.claude-tools/kebacc-switch:*), Bash(~/.claude-tools/kebacc-switch.exe:*)
argument-hint: "[on|off]"
---

Flip the auto-switch. Argument: `$ARGUMENTS`

Run `~/.claude-tools/kebacc-switch`, or `~/.claude-tools/kebacc-switch.exe` on Windows:

```
~/.claude-tools/kebacc-switch arm -Provider <scope>
```

`<scope>` comes from the argument:

- `off` — `off`
- `claude` — `claude`
- `on`, empty, or no argument — `claude` when nothing is armed, `off` when
  something is. `~/.claude-tools/kebacc-switch doctor -Provider claude` says
  which it is; the status line's `auto …` segment says the same.

The command writes the `SessionStart` hook in `~/.claude/settings.json` itself,
leaving every other hook alone. It never changes the account in use — only
`/kebacc-switch-claude` does that.

Print the one line it prints and stop. Nothing else — no explanation of what the
switch does, no suggestion to add accounts, no question.
