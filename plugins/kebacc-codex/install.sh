#!/bin/sh
# Puts the Codex half of the switcher on this machine. The macOS and Linux
# counterpart of install.ps1, and it does the same things in the same order.
#
# The binary is kebacc-codex, and it carries the Codex pool only. kebacc-switch,
# the Claude half, is a separate program published from master; it may live in
# this same directory under its own name, and the two do not read or rewrite
# each other's binary, marker, hooks or status line.
#
# Options, all optional:
#   --tools-dir DIR    where the binary goes (default ~/.claude-tools)
#   --binary PATH      the binary to install, instead of the built one
#   --auto-switch      run `auto` at session start and during a task
#   --status-line      point the Claude Code status line at the switcher
#   --no-auto-update   do not let the switcher update itself
#   --force            install this build even over the same version
#   --no-profile-edit  do not add the shell function to the shell rc file
set -eu

tools_dir="${HOME}/.claude-tools"
binary=""
auto_switch=no
status_line=no
no_auto_update=no
profile_edit=yes
force=no

while [ $# -gt 0 ]; do
    case "$1" in
        --tools-dir) tools_dir="${2:?--tools-dir needs a directory}"; shift 2 ;;
        --binary) binary="${2:?--binary needs a path}"; shift 2 ;;
        --auto-switch|--autoswitch) auto_switch=yes; shift
            # `--auto-switch codex` is accepted so the two installers take the
            # same words; codex is the pool this half speaks for.
            case "${1:-}" in codex|all) shift ;; esac ;;
        --force) force=yes; shift ;;
        --no-profile-edit|--noprofileedit) profile_edit=no; shift ;;
        --status-line|--statusline) status_line=yes; shift ;;
        --no-auto-update|--noautoupdate) no_auto_update=yes; shift ;;
        -h|--help) sed -n '2,19p' "$0"; exit 0 ;;
        *) printf 'Unknown option %s\n' "$1" >&2; exit 64 ;;
    esac
done

here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
version=$(tr -d ' \t\r\n' < "$here/VERSION")
claude="${CLAUDE_CONFIG_DIR:-$HOME/.claude}"

# The parts install.sh and uninstall.sh both need.
. "$here/shared.sh"

plugin=codex

red() { printf '\033[31m%s\033[0m\n' "$1"; }
green() { printf '\033[32m%s\033[0m\n' "$1"; }
yellow() { printf '\033[33m%s\033[0m\n' "$1"; }
dim() { printf '\033[90m%s\033[0m\n' "$1"; }

# The workspace root is two directories up from the plugin, and cargo puts the
# binary under it. A release build is preferred over a debug one.
if [ -z "$binary" ]; then
    root=$(dirname -- "$(dirname -- "$here")")
    for profile in release debug; do
        if [ -x "$root/target/$profile/kebacc-codex" ]; then
            binary="$root/target/$profile/kebacc-codex"
            break
        fi
    done
fi

mkdir -p "$tools_dir"
entry="$tools_dir/kebacc-codex"

# Beside kebacc-codex there may already be a binary, and a newer one than this
# plugin ships. Only when there is not does a build have to be found here.
if [ "$force" = yes ] || kebacc_should_replace_binary "$entry" "$version"; then
    if [ -z "$binary" ] || [ ! -f "$binary" ]; then
        red 'No kebacc binary found. Build it first: cargo build --release -p kebacc-codex'
        exit 1
    fi
    # Written to a neighbouring name and moved into place, because a running
    # switcher, a status line most of all, cannot be overwritten byte by byte
    # without the running copy seeing a half-written file.
    cp -f -- "$binary" "$entry.new"
    chmod 700 "$entry.new"
    mv -f -- "$entry.new" "$entry"
    green "Installed the switcher into $tools_dir"
else
    dim 'kebacc-codex already put a binary here - using it. --force replaces it anyway.'
fi

# A binary that does not know this pool is a kebacc-codex left here from before
# Codex came back into the crate. Saying so now beats five slash commands that
# answer with an unknown provider.
if ! "$entry" list -Provider codex 2>&1 | grep -qi codex; then
    red "The binary at $entry does not know the codex pool."
    yellow 'Update kebacc-codex, or pass --binary pointing at a build that does.'
    exit 1
fi

# The slash commands, which is how the switcher is used from inside Claude Code.
command_target="$claude/commands"
if [ -d "$here/src/commands" ]; then
    mkdir -p "$command_target"
    # This plugin's own commands, by name. kebacc-codex's commands live in this
    # same directory and are not ours to remove.
    for own in $(kebacc_own_commands "$plugin"); do
        rm -f -- "$command_target/$own"
    done
    # Names this half used under an earlier release, so an update does not leave
    # two of each. Per plugin, for the same reason as above.
    for gone_name in $(kebacc_stale_commands "$plugin"); do
        rm -f -- "$command_target/$gone_name"
    done
    cp -f -- "$here"/src/commands/*.md "$command_target/"
    green "Installed the Codex slash commands into $command_target"
fi

# The version file is what says this half is installed, and it is what another
# switcher sharing the machine reads to know it is here.
printf '%s' "$version" > "$tools_dir/$(kebacc_marker "$plugin")"

# The commands that only mean something with both pools present. Neither plugin
# owns them, so whichever installer finds the other half already there puts them
# in - and this runs after the marker above, or it would not count itself.
if [ -d "$here/src/commands-all" ]; then
    synced=$(kebacc_sync_all_commands "$command_target" "$here/src/commands-all" "$tools_dir")
    if [ "$synced" -gt 0 ] && kebacc_both_installed "$tools_dir"; then
        green "Installed the $synced command(s) that span both pools"
    fi
fi

# `kebacc-codex` as a shell function rather than a directory on the PATH. Same
# name and same body as the other plugin writes, so whichever runs second finds
# the line already there and leaves it alone.
if [ "$profile_edit" = yes ]; then
    marker='# kebacc-codex account switcher'
    case "${SHELL:-}" in
        */zsh) rc="$HOME/.zshrc" ;;
        */bash) rc="$HOME/.bashrc" ;;
        *) rc="$HOME/.profile" ;;
    esac
    if [ -f "$rc" ] && grep -qF "$marker" "$rc"; then
        dim "kebacc-codex is already in $rc."
    else
        {
            printf '\n%s\n' "$marker"
            printf 'kebacc-codex() { "%s" "$@"; }\n' "$entry"
        } >> "$rc"
        green "Added kebacc-codex to $rc"
        dim 'Open a new shell for it to exist there.'
    fi
fi

# settings.json belongs to the user, so the switcher edits it itself rather than
# a second implementation of that read-amend-write living here in sed.
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

# -Merge, not a plain arm: the hooks this reads and rewrites are the ones running
# kebacc-codex, and a scope already there is added to rather than replaced. The
# Claude switcher's own hooks run its own binary and are left exactly as they
# are, so installing here never disarms it.
if [ "$auto_switch" = yes ]; then
    armed=$("$entry" arm -Provider codex -Merge)
    green "Session start and every tool call now check the quota: $armed"
fi

printf '\n'
green "kebacc-codex $version is installed."
dim '  /kebacc-add-codex       save the Codex login you are on'
dim '  /kebacc-list-codex      what is saved, and its quota'
dim '  /kebacc-switch-codex    move to another saved login'
