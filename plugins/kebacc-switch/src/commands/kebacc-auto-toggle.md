---
description: Turn the auto-switch on or off, any time, no conditions
allowed-tools: Bash(~/.claude-tools/kebacc-switch:*), Read, Edit, Write
argument-hint: "[on|off|claude|codex|all]"
---

Flip the auto-switch. Argument: `$ARGUMENTS`

The switch is armed by a `SessionStart` hook in `~/.claude/settings.json`. Arming
it means the hook is present; disarming means it is gone. Nothing else gates
this — no quota check, no account count, no confirmation. Do what the argument
says and stop.

Read `~/.claude/settings.json` first, then:

- `off` — remove every `hooks.SessionStart` hook whose command matches
  `kebacc-switch … auto`. Leave any other SessionStart hook untouched. If the list
  ends up empty, remove the `SessionStart` key.
- `on`, empty, or no argument — toggle: if such a hook exists, remove it as
  above; if none exists, add the `all` hook below.
- `claude`, `codex`, or `all` — arm for that provider. If a `kebacc-switch … auto`
  hook already exists, edit its `-Provider` in place. Otherwise add:

  ```json
  {
    "type": "command",
    "command": "<absolute path to kebacc-switch> auto -Provider all -Hook",
    "timeout": 25
  }
  ```

  with `all` replaced by the requested provider. The path is the one in
  `~/.claude-tools`, written out in full — `kebacc-switch.exe` on Windows,
  `kebacc-switch` elsewhere — and quoted if it contains a space.

Keep the file valid JSON and keep the existing indentation.

Then print one line: `auto <provider>` or `auto off`. Nothing else — no
explanation of what the switch does, no suggestion to add accounts, no question.
