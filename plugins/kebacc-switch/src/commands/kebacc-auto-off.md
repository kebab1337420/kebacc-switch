---
description: Turn the auto-switch off
allowed-tools: Read, Edit, Write
---

Disarm the auto-switch. No conditions, no confirmation.

Read `~/.claude/settings.json`, then under `hooks.SessionStart` remove every hook
whose command matches `kebacc-switch … auto` (any `-Provider`, any extension, and the
legacy `claude-code-auto.ps1` form too). Leave any other SessionStart hook alone.
If a group's `hooks` list ends up empty, drop the group; if `SessionStart` ends up
empty, drop the key.

Keep the file valid JSON and keep the existing indentation.

Then print one line: `auto off`, or `auto already off` if there was nothing to
remove. Nothing else — no explanation, no suggestion, no question.
