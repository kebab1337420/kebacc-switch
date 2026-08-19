#!/bin/sh
# Takes kebacc-switch back off this machine. The macOS and Linux counterpart of
# uninstall.ps1.
#
# The pool is left alone by default: the saved logins are the point of the tool,
# and removing the plugin is not a reason to lose them. Pass --pool to delete it.
#
# Options:
#   --tools-dir DIR   where the binary was installed (default ~/.claude-tools)
#   --pool            also delete the saved logins
set -eu

tools_dir="${HOME}/.claude-tools"
pool=no

while [ $# -gt 0 ]; do
    case "$1" in
        --tools-dir) tools_dir="${2:?--tools-dir needs a directory}"; shift 2 ;;
        --pool) pool=yes; shift ;;
        -h|--help) sed -n '2,11p' "$0"; exit 0 ;;
        *) printf 'Unknown option %s\n' "$1" >&2; exit 64 ;;
    esac
done

claude="${CLAUDE_CONFIG_DIR:-$HOME/.claude}"
entry="$tools_dir/kebacc-switch"

green() { printf '\033[32m%s\033[0m\n' "$1"; }
yellow() { printf '\033[33m%s\033[0m\n' "$1"; }
dim() { printf '\033[90m%s\033[0m\n' "$1"; }

# Disarmed and unwired before the binary goes, or the session hook and the
# status line are left pointing at a file that is no longer there.
if [ -x "$entry" ]; then
    # -Drop rather than off: it takes this pool out and leaves anything else
    # armed, where off disarms whatever it finds.
    "$entry" arm -Provider claude -Drop -Quiet > /dev/null 2>&1 || true
    "$entry" wire -NoStatusLine -Quiet > /dev/null 2>&1 || true
    # KEBACC_SWITCH_UPDATE is a setting about a binary that is about to be gone,
    # and left there it would silence the next install. kebacc-codex reads the
    # same name, so it stays while that half is installed: turning its updates
    # back on is not this uninstaller's call.
    if [ ! -e "$tools_dir/.codex-version" ]; then
        "$entry" wire -AutoUpdate -Quiet > /dev/null 2>&1 || true
    fi
fi

# Named one by one: ~/.claude-tools is shared with kebacc-codex, and a sweep
# here would take its files too.
removed=0
for name in kebacc-switch kebacc-switch.old .version .update.json update.stamp \
    install-codex.ps1 \
    claude-cc.ps1 claude-cc-core.ps1 claude-cc-usage.ps1 \
    claude-cc-pool.ps1 claude-cc-statusline.ps1 claude-cc-providers.ps1 \
    kebacc-switch.ps1 statusline.js claude-cc.js package.json; do
    if [ -e "$tools_dir/$name" ]; then
        rm -f "$tools_dir/$name"
        removed=$((removed + 1))
    fi
done
for half in "$tools_dir"/kebacc-switch.*.new; do
    [ -e "$half" ] || continue
    rm -f "$half"
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
        dim "$tools_dir kept: another plugin has files there."
    fi
fi

# The codex commands belong to kebacc-codex, which has its own uninstaller.
# kebacc-install-codex.md is the exception: this plugin ships it, so this plugin
# takes it away, or it is left behind pointing at a script that went with the
# binary.
gone=0
for old in "$claude"/commands/kebacc-*.md "$claude"/commands/account-*.md "$claude"/commands/claude-account-*.md; do
    [ -e "$old" ] || continue
    case "$old" in
        */kebacc-install-codex.md) ;;
        *codex*) continue ;;
    esac
    rm -f "$old"
    gone=$((gone + 1))
done
if [ "$gone" -gt 0 ]; then
    green "Removed $gone slash command(s)"
fi

marker='# kebacc-switch account switcher'
for rc in "$HOME/.zshrc" "$HOME/.bashrc" "$HOME/.profile"; do
    [ -f "$rc" ] || continue
    grep -qF "$marker" "$rc" || continue
    # The block is the marker and the line after it, and nothing else in the
    # file is touched.
    tmp="$rc.kebacc-tmp"
    grep -vF -e "$marker" -e 'kebacc-switch() {' "$rc" > "$tmp"
    mv -f -- "$tmp" "$rc"
    green "Took kebacc-switch out of $rc"
done

pool_dir="${KEBACC_SWITCH_ACCOUNTS:-$HOME/.kebacc-switch-accounts}"
if [ "$pool" = yes ]; then
    if [ -d "$pool_dir" ]; then
        rm -rf -- "$pool_dir"
        yellow "Deleted the pool $pool_dir"
    fi
elif [ -d "$pool_dir" ]; then
    dim "The saved accounts are still in $pool_dir. Delete them with --pool."
fi

printf '\n'
dim 'The status line and the session hook were taken out of the Claude Code settings.'
