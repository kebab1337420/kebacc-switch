---
description: Per-pool settings: the order auto picks from, its caps, and what runs after a switch
allowed-tools: Bash(~/.claude-tools/kebacc:*), Bash(~/.claude-tools/kebacc.exe:*)
argument-hint: "-claude|-codex|-ag [-Rank <n>] [-Reserve|-NoReserve] [-FiveHour <pct>] [-SevenDay <pct>] [-OnSwitch <cmd>]"
---

Run `~/.claude-tools/kebacc`, or `~/.claude-tools/kebacc.exe` on Windows. This one needs a pool named: `-claude`, `-codex` or `-ag`.

```
~/.claude-tools/kebacc set $ARGUMENTS
```

- `-Rank <n>` sets where an account sits in the order `auto` picks from, highest first.
- `-Reserve` holds an account back until every other one is capped; `-NoReserve` puts it back in the normal rotation.
- Both act on the account named by `-Email`, or on the live one when no `-Email` is given.
- `-FiveHour <pct>` and `-SevenDay <pct>` set the switch thresholds for that pool alone, ahead of `CLAUDE_AUTOSWITCH_THRESHOLD` and `CLAUDE_AUTOSWITCH_WEEKLY_THRESHOLD`. `off` puts one back to the default.
- `-OnSwitch <cmd>` runs a command after every switch in that pool, with `KEBACC_POOL`, `KEBACC_CLI`, `KEBACC_FROM` and `KEBACC_TO` in its environment. `-OnSwitch ""` clears it. `KEBACC_SWITCH_ON_SWITCH` overrides it for one run.

The settings live in `.pool.json` beside the saved logins. Report what the command answers; it changes nothing else.
