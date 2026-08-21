# kebacc-antigravity

Saves the Antigravity login currently in use, seals the saved ones on disk,
reads each account's quota, and switches to one that still has room. The binary
lives in `crates/kebacc-antigravity/`. The slash commands it carries live in
`plugins/kebacc-antigravity/`.

## Branch layout

`master` is the Claude Code half, published as `kebacc-switch`. The `Codex`
branch is the Codex half, published as `kebacc-codex`. This tree is the
Antigravity half on `antigravity-port`, published as `kebacc-antigravity`. Each
half has its own binary, its own pool and its own slash commands. They install
side by side into `~/.claude-tools`. Releases use a tag prefix per half
(`kebacc-antigravity-v*` here). A download has to pick the file by name.
GitHub's Latest label can only point at one of them.

## Shared source

The three halves share most of their source. A fix in `seal.rs`, `pool.rs`,
`lock.rs` or `jsonio.rs` has to be copied onto each branch by hand.

## Version

The version number is one number written in three places, kept in step by CI:
`crates/kebacc-antigravity/Cargo.toml`, `plugins/kebacc-antigravity/VERSION`,
and the string the binary prints for `--version`.

## Checks

```
cargo fmt --all -- --check
cargo clippy --release --all-targets -p kebacc-antigravity -- -D warnings
cargo test --release -p kebacc-antigravity
```
