# Security policy

## What this tool holds

One binary, three pools. Claude Code, Codex and Antigravity OAuth refresh
tokens live in separate directories. Windows seals them with DPAPI. macOS and
Linux seal them with AES-256-GCM under a key held by the login keychain
(`security`) or by libsecret (`secret-tool`). When none of those is
available, `add` writes the credentials as plain JSON inside the account
snapshot, skips the HMAC stamp that detects an edited pool file, and prints
a warning once.

The keychain account names are load-bearing. Claude and Codex store the seal
key under `kebacc-switch`. Antigravity stores it under `kebacc-antigravity`.
Renaming either unlocks nothing already sealed.

## Supported versions

Only the latest `kebacc-v*` release receives security fixes. GitHub's Latest
label is not used.

## Reporting a vulnerability

Use GitHub private vulnerability reporting on this repository. Do not open a
public issue for a security problem.
