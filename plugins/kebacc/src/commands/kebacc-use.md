---
description: Give a terminal a session directory of its own, on one saved account
allowed-tools: Bash(~/.claude-tools/kebacc:*), Bash(~/.claude-tools/kebacc.exe:*)
argument-hint: "-claude|-codex|-ag|-grok [-Email you@example.com] [-Dir <path>]"
---

Run `~/.claude-tools/kebacc`, or `~/.claude-tools/kebacc.exe` on Windows. This one needs a pool named: `-claude`, `-codex`, `-ag` or `-grok`.

```
~/.claude-tools/kebacc use $ARGUMENTS
```

It writes the saved login into a directory of its own and prints the line that points a shell at it. A CLI started from that shell runs on that account, and a CLI started anywhere else is untouched — so several projects can hold several accounts at the same time instead of taking turns.

The directory defaults to `~/.kebacc-sessions/<cli>/<address>`; `-Dir` puts it somewhere else. The variable to set is the one that CLI reads: `CLAUDE_CONFIG_DIR` for Claude Code, `CODEX_HOME` for Codex, `GROK_HOME` for Grok, and so on; the command prints the right line for the pool you named. The saved logins stay shared, so an account set up this way is still the same one `list` and `switch` know about.

Two things to say when you report this: the variable has to be set before the CLI starts, since nothing repoints a session already running; and on a machine that keeps the login in the keychain, every session reads the same keychain, so the directory cannot hold an account of its own there.
