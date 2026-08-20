---
description: Turn the auto-switch on or off, any time, no conditions
allowed-tools: Bash(~/.claude-tools/kebacc-antigravity:*), Bash(~/.claude-tools/kebacc-antigravity.exe:*)
argument-hint: "[on|off]"
---

Flip the auto-switch. Argument: `$ARGUMENTS`

Run `~/.claude-tools/kebacc-antigravity`, or `~/.claude-tools/kebacc-antigravity.exe` on Windows:

```
~/.claude-tools/kebacc-antigravity arm -Provider <scope>
```

`<scope>` comes from the argument:

- `off` — `off`
- `antigravity` — `antigravity`
- `on`, empty, or no argument — `antigravity` when nothing is armed, `off` when
  something is. `~/.claude-tools/kebacc-antigravity doctor -Provider antigravity` says
  which it is; the status line's `auto …` segment says the same.

The command writes both of its hooks in `~/.claude/settings.json` itself — the
`SessionStart` one and the `PreToolUse` one that lets it act mid-task — leaving
every other hook alone. It never changes the account in use on its own; only
`/kebacc-switch-antigravity` does that on the spot.

Print the one line it prints and stop. Nothing else — no explanation of what the
switch does, no suggestion to add accounts, no question.
