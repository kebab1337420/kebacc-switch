#!/bin/sh
# Puts the switcher on this machine. The macOS and Linux counterpart of
# install.ps1, and it does the same things in the same order.
#
# This step downloads nothing. The switcher is a binary, built from
# `crates/kebacc-switch` by `cargo build --release`, and this copies the one
# that came out of that build into place. Run it again to update: it overwrites
# what it owns and never touches the pools.
#
# Options, all optional:
#   --tools-dir DIR    where the binary goes (default ~/.claude-tools)
#   --binary PATH      the binary to install, instead of the built one
#   --status-line      point Claude Code's status line at the switcher
#   --auto-switch      run `auto` at session start and mid-task
#   --no-auto-update   turn the daily self-update off on this machine
#   --no-profile-edit  do not add the shell function to the shell rc file
set -eu

tools_dir="${HOME}/.claude-tools"
binary=""
status_line=no
auto_switch=no
no_auto_update=no
profile_edit=yes

while [ $# -gt 0 ]; do
    case "$1" in
        --tools-dir) tools_dir="${2:?--tools-dir needs a directory}"; shift 2 ;;
        --binary) binary="${2:?--binary needs a path}"; shift 2 ;;
        --status-line|--statusline) status_line=yes; shift ;;
        --auto-switch|--autoswitch) auto_switch=yes; shift
            # `--auto-switch claude` is accepted so the two installers take the
            # same words; claude is the only pool this plugin has.
            case "${1:-}" in claude|all) shift ;; esac ;;
        --no-auto-update|--noautoupdate) no_auto_update=yes; shift ;;
        --no-profile-edit|--noprofileedit) profile_edit=no; shift ;;
        -h|--help) sed -n '2,17p' "$0"; exit 0 ;;
        *) printf 'Unknown option %s\n' "$1" >&2; exit 64 ;;
    esac
done

here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
version=$(tr -d ' \t\r\n' < "$here/VERSION")
claude="${CLAUDE_CONFIG_DIR:-$HOME/.claude}"

red() { printf '\033[31m%s\033[0m\n' "$1"; }
green() { printf '\033[32m%s\033[0m\n' "$1"; }
yellow() { printf '\033[33m%s\033[0m\n' "$1"; }
dim() { printf '\033[90m%s\033[0m\n' "$1"; }

# The workspace root is two directories up from the plugin, and cargo puts the
# binary under it. A release build is preferred over a debug one.
if [ -z "$binary" ]; then
    root=$(dirname -- "$(dirname -- "$here")")
    for profile in release debug; do
        if [ -x "$root/target/$profile/kebacc-switch" ]; then
            binary="$root/target/$profile/kebacc-switch"
            break
        fi
    done
fi
if [ -z "$binary" ] || [ ! -f "$binary" ]; then
    red 'No kebacc-switch binary found. Build it first: cargo build --release -p kebacc-switch'
    exit 1
fi

mkdir -p "$tools_dir"

# Versions before this one were a set of PowerShell scripts and a node status
# line. Named one by one on purpose: $tools_dir is a shared directory and a
# wildcard sweep here would delete files this installer never wrote.
for stale in claude-cc.ps1 claude-cc-core.ps1 claude-cc-usage.ps1 \
    claude-cc-pool.ps1 claude-cc-statusline.ps1 claude-cc-providers.ps1 \
    kebacc-switch.ps1 statusline.js claude-cc.js package.json; do
    rm -f "$tools_dir/$stale"
done

entry="$tools_dir/kebacc-switch"
# Written to a neighbouring name and moved into place, because a running
# switcher, a status line most of all, cannot be overwritten byte by byte
# without the running copy seeing a half-written file.
cp -f -- "$binary" "$entry.new"
chmod 700 "$entry.new"
mv -f -- "$entry.new" "$entry"
green "Installed kebacc-switch into $tools_dir"

# The slash commands, which is how the switcher is used from inside Claude Code.
if [ -d "$here/src/commands" ]; then
    mkdir -p "$claude/commands"
    # Names from earlier versions, so an update does not leave two of each. The
    # codex ones are left alone: they belong to kebacc-codex, which installs
    # into this same directory.
    for old in "$claude"/commands/claude-account-*.md "$claude"/commands/account-*.md "$claude"/commands/kebacc-*.md; do
        [ -e "$old" ] || continue
        case "$old" in *codex*) continue ;; esac
        rm -f "$old"
    done
    rm -f "$claude/commands/refresh-a.md" "$claude/commands/refresh-t.md"
    cp -f -- "$here"/src/commands/*.md "$claude/commands/"
    green "Installed the slash commands into $claude/commands"
fi

printf '%s' "$version" > "$tools_dir/.version"

# The binary is asked what it is rather than taken on trust: a --binary pointing
# at an older build, and a truncated download, fail here where the message can
# say so.
reported=$("$entry" --version 2>/dev/null | head -n 1 || true)
if [ -z "$reported" ]; then
    red "Copied the binary, but $entry would not run."
    yellow 'The slash commands are in place; the settings were left untouched.'
    exit 1
fi
reported_version=${reported##* }
if [ "$reported_version" != "$version" ]; then
    yellow "The binary reports $reported_version and the plugin here is $version."
    yellow 'The status line will show the plugin version with a ! until the two match.'
fi

# `kebacc-switch` as a shell function rather than a directory on the PATH: it is
# one line to add, and it keeps the name working in a shell where ~/.claude-tools
# is not on the PATH.
if [ "$profile_edit" = yes ]; then
    marker='# kebacc-switch account switcher'
    case "${SHELL:-}" in
        */zsh) rc="$HOME/.zshrc" ;;
        */bash) rc="$HOME/.bashrc" ;;
        *) rc="$HOME/.profile" ;;
    esac
    if [ -f "$rc" ] && grep -qF "$marker" "$rc"; then
        dim "kebacc-switch is already in $rc."
    else
        {
            printf '\n%s\n' "$marker"
            printf 'kebacc-switch() { "%s" "$@"; }\n' "$entry"
        } >> "$rc"
        green "Added kebacc-switch to $rc"
        dim 'Open a new shell for it to exist there.'
    fi
fi

# settings.json belongs to the user, so the switcher edits it itself: one
# implementation of that read-amend-write, shared by both installers, rather
# than a second one here in sed.
if [ "$status_line" = yes ] || [ "$no_auto_update" = yes ]; then
    set --
    if [ "$status_line" = yes ]; then set -- "$@" -StatusLine; fi
    if [ "$no_auto_update" = yes ]; then set -- "$@" -NoAutoUpdate; fi
    "$entry" wire "$@" > /dev/null
    if [ "$status_line" = yes ]; then
        green "Pointed the Claude Code status line at the switcher ($claude/settings.json)"
    fi
    if [ "$no_auto_update" = yes ]; then
        yellow "The switcher will not update itself: KEBACC_SWITCH_UPDATE=off ($claude/settings.json)"
    fi
fi

if [ "$auto_switch" = yes ]; then
    # -Merge, not a plain arm: a hook already armed on a scope that covers the
    # other half too keeps covering it. Installing this plugin must not narrow
    # what somebody else is running.
    armed=$("$entry" arm -Provider claude -Merge)
    green "Session start and every tool call now check the quota: ${armed:-auto claude}"
fi

printf '\n'
green "kebacc-switch $version is installed."
dim '  kebacc-switch add            save the login you are on'
dim '  kebacc-switch list           what is saved, and its quota'
dim '  kebacc-switch doctor         check everything'
