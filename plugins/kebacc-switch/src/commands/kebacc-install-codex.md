---
description: Install the Codex switcher, which lives on its own branch
allowed-tools: Bash(pwsh:*), Bash(~/.claude-tools/kebacc-codex:*), Bash(~/.claude-tools/kebacc-codex.exe:*)
---

Run:

```
pwsh -NoProfile -Command "$ErrorActionPreference='Stop'; $s=Join-Path ([IO.Path]::GetTempPath()) 'kebacc-install-codex.ps1'; Invoke-WebRequest 'https://api.github.com/repos/kebab1337420/kebacc-switch/contents/plugins/kebacc-switch/install-codex.ps1?ref=master' -Headers @{'User-Agent'='kebacc-switch';'Accept'='application/vnd.github.raw'} -OutFile $s; & $s; $code=$LASTEXITCODE; Remove-Item $s -Force -ErrorAction Ignore; exit $code"
```

The installer is taken from the repository each time rather than from disk, so
this works on an install that predates it and never runs a stale copy.

It clones the `Codex` branch, builds `kebacc-codex` with cargo and installs it
beside kebacc-switch: its own binary, its own `*-codex` slash commands, the same
saved logins. It needs `git` and `cargo`, and takes a minute the first time.
Nothing on the Claude side is touched, and running it again is how it updates.

To arm the session-start auto-switch for the Codex pool at the same time, add
`-AutoSwitch` to the `& $s` call. A checkout on this machine can be used instead
of the clone with `-Source <path>`.

Report the version it installed. The new slash commands appear once Claude Code
is restarted.
