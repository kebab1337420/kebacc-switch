---
description: Per-pool settings: the order auto picks from, and its caps
allowed-tools: Bash(~/.claude-tools/kebacc:*), Bash(~/.claude-tools/kebacc.exe:*)
argument-hint: "-claude|-codex|-ag [-Rank <n>] [-FiveHour <pct>] [-SevenDay <pct>]"
---

Run `~/.claude-tools/kebacc`, or `~/.claude-tools/kebacc.exe` on Windows. This one needs a pool named: `-claude`, `-codex` or `-ag`.

```
~/.claude-tools/kebacc set $ARGUMENTS
```

- `-Rank <n>` sets where an account sits in the order `auto` picks from, highest first. Name the account with `-Email`; without it the command asks which one, so pass `-Email` when nobody is there to answer.
- `-FiveHour <pct>` and `-SevenDay <pct>` set the switch thresholds for that pool alone, ahead of `CLAUDE_AUTOSWITCH_THRESHOLD` and `CLAUDE_AUTOSWITCH_WEEKLY_THRESHOLD`. `off` puts one back to the default.

The settings live in `.pool.json` beside the saved logins. Report what the command answers; it changes nothing else.
