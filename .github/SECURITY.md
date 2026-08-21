# Security policy

## What this tool holds

The pool stores OAuth refresh tokens for Antigravity logins. They are sealed
before they touch disk. Windows uses DPAPI. Everywhere else the seal is
AES-256-GCM under a key kept in the OS keychain (macOS Keychain or libsecret).
When no OS secret store is available, that key is stored in plaintext next to
the pool.

## Supported versions

Only the latest release receives security fixes.

## Reporting a vulnerability

Use GitHub private vulnerability reporting on this repository. Do not open a
public issue for a security problem.
