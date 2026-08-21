---
description: Turn the auto-switch on or off, any time, no conditions
allowed-tools: Bash(~/.claude-tools/kebacc:*), Bash(~/.claude-tools/kebacc.exe:*)
argument-hint: "[on|off]"
---

Flip the auto-switch. Argument: `$ARGUMENTS`

Run `~/.claude-tools/kebacc`, or `~/.claude-tools/kebacc.exe` on Windows:

```
~/.claude-tools/kebacc arm -Provider <scope>
```

`<scope>` comes from the argument:

- `off` — `off`
- `claude` — `claude`
- `on`, empty, or no argument — `claude` when nothing is armed, `off` when
  something is. `~/.claude-tools/kebacc doctor -Provider claude` says
  which it is; the status line's `auto …` segment says the same.

The command writes both of its hooks in `~/.claude/settings.json` itself — the
`SessionStart` one and the `PreToolUse` one that lets it act mid-task — leaving
every other hook alone. It never changes the account in use on its own; only
`/kebacc-switch-claude` does that on the spot.

Print the one line it prints and stop. Nothing else — no explanation of what the
switch does, no suggestion to add accounts, no question.
