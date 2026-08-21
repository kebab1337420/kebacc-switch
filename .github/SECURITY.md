# Security policy

## What this tool holds

Each half stores OAuth refresh tokens for the pool it owns: Claude Code,
Codex, and Antigravity. Windows seals them with DPAPI. macOS and Linux seal
them with AES-256-GCM under a key held by the login keychain (`security`) or
by libsecret (`secret-tool`). When none of those is available, `add` writes
the credentials as plain JSON inside the account snapshot, skips the HMAC
stamp that detects an edited pool file, and prints a warning once.

The keychain account names are load-bearing. Claude and Codex store the seal
key under `kebacc-switch`. Antigravity stores it under `kebacc-antigravity`.
Renaming either unlocks nothing already sealed.

## Supported versions

Only the latest release of each half receives security fixes. A half is
identified by its tag prefix (`kebacc-v*`, `kebacc-codex-v*`,
`kebacc-antigravity-v*`), never by GitHub's Latest label.

## Reporting a vulnerability

Use GitHub private vulnerability reporting on this repository. Do not open a
public issue for a security problem.
