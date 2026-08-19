#!/bin/sh
# Takes kebacc-codex back off this machine. The macOS and Linux counterpart of
# uninstall.ps1.
#
# The pool is left alone by default: the saved logins are the point of the tool,
# and removing the plugin is not a reason to lose them. Pass --pool to delete it.
#
# kebacc-switch, the Claude half, may be installed into this same directory. It
# has a binary, hooks, a status line and a shell function of its own, all under
# its own name, and none of them are touched here.
#
# Options:
#   --tools-dir DIR   where the binary was installed (default ~/.claude-tools)
#   --pool            also delete the saved Codex logins
set -eu

tools_dir="${HOME}/.claude-tools"
pool=no

while [ $# -gt 0 ]; do
    case "$1" in
        --tools-dir) tools_dir="${2:?--tools-dir needs a directory}"; shift 2 ;;
        --pool) pool=yes; shift ;;
        -h|--help) sed -n '2,14p' "$0"; exit 0 ;;
        *) printf 'Unknown option %s\n' "$1" >&2; exit 64 ;;
    esac
done

here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
claude="${CLAUDE_CONFIG_DIR:-$HOME/.claude}"
entry="$tools_dir/kebacc-codex"

. "$here/shared.sh"

plugin=codex

green() { printf '\033[32m%s\033[0m\n' "$1"; }
yellow() { printf '\033[33m%s\033[0m\n' "$1"; }
dim() { printf '\033[90m%s\033[0m\n' "$1"; }

# Whether kebacc-codex is still here decides almost everything below: the
# binary, the shell function and the status line are shared, and are only taken
# out by whichever half leaves last.
installed=$(kebacc_installed "$tools_dir")
alone=yes
kebacc_has claude $installed && alone=no

# Disarmed before the binary goes, or the hooks are left pointing at a file that
# is no longer there. -Drop takes this pool out of the scope our own hooks carry
# and disarms them when nothing is left; hooks running the Claude switcher are
# not read here at all. The status line comes out the same way: `wire` only
# removes one that points at this binary.
if [ -x "$entry" ]; then
    "$entry" arm -Provider "$plugin" -Drop -Quiet > /dev/null 2>&1 || true
    "$entry" wire -NoStatusLine > /dev/null 2>&1 || true
fi

# The version marker goes first, so anything reading it after this point sees
# this half as gone.
rm -f -- "$tools_dir/$(kebacc_marker "$plugin")"

# Named one by one: ~/.claude-tools is shared with kebacc-switch, and a sweep
# here would take its files too. Everything listed is this half's own, so it
# goes whether or not the other half is installed - the two have not shared a
# binary since they stopped being one program.
removed=0
for name in kebacc-codex kebacc-codex.old .update-codex.json update-codex.stamp; do
    if [ -e "$tools_dir/$name" ]; then
        rm -f -- "$tools_dir/$name"
        removed=$((removed + 1))
    fi
done
for half in "$tools_dir"/kebacc-codex.*.new; do
    [ -e "$half" ] || continue
    rm -f -- "$half"
    removed=$((removed + 1))
done
if [ "$removed" -gt 0 ]; then
    green "Removed $removed file(s) from $tools_dir"
fi

if [ -d "$tools_dir" ]; then
    if [ -z "$(ls -A "$tools_dir" 2>/dev/null)" ]; then
        rmdir "$tools_dir"
        green "Removed $tools_dir"
    else
        dim "$tools_dir kept: something else has files there."
    fi
fi

# This plugin's own names. kebacc-codex's commands belong to it and stay until
# its own uninstaller runs.
commands="$claude/commands"
if [ -d "$commands" ]; then
    gone=0
    for own in $(kebacc_own_commands "$plugin"); do
        if [ -e "$commands/$own" ]; then
            rm -f -- "$commands/$own"
            gone=$((gone + 1))
        fi
    done
    for old in $(kebacc_stale_commands "$plugin"); do
        [ -e "$commands/$old" ] || continue
        rm -f -- "$commands/$old"
        gone=$((gone + 1))
    done
    if [ "$gone" -gt 0 ]; then
        green "Removed $gone slash command(s)"
    fi
    # With one half gone the -all pair has nothing to span, so it goes with it.
    kebacc_sync_all_commands "$commands" '' "$tools_dir" > /dev/null
fi

# The shell function carries this binary's name, so it goes with this binary
# whatever else is installed. kebacc-switch writes a line of its own, under its
# own name, which these patterns do not match.
marker='# kebacc-codex account switcher'
for rc in "$HOME/.zshrc" "$HOME/.bashrc" "$HOME/.profile"; do
    [ -f "$rc" ] || continue
    grep -qF "$marker" "$rc" || continue
    tmp="$rc.kebacc-tmp"
    grep -vF -e "$marker" -e 'kebacc-codex() {' "$rc" > "$tmp"
    mv -f -- "$tmp" "$rc"
    green "Took kebacc-codex out of $rc"
done

pool_dir="${KEBACC_SWITCH_CODEX_ACCOUNTS:-$HOME/.kebacc-switch-codex-accounts}"
if [ "$pool" = yes ]; then
    if [ -d "$pool_dir" ]; then
        rm -rf -- "$pool_dir"
        yellow "Deleted the Codex pool $pool_dir"
    fi
elif [ -d "$pool_dir" ]; then
    dim "The saved Codex accounts are still in $pool_dir. Delete them with --pool."
fi

printf '\n'
if [ "$alone" = yes ]; then
    dim 'The status line and the auto hooks were taken out of the Claude Code settings.'
else
    dim 'This half is out of the Claude Code settings; kebacc-switch keeps its own.'
fi
