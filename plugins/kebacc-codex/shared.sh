#!/bin/sh
# What the two kebacc plugins have to agree on to share one machine, for the
# macOS and Linux installers. The shell half of shared.ps1, and it answers the
# same questions in the same words.
#
# kebacc-switch, published from master, carries the Claude Code pool;
# kebacc-codex carries the Codex one. Two binaries, two names, one machine.
# They ship separately and neither can disturb the other: each has its own
# binary name, its own version marker, its own release tags and its own pair of
# hooks. What is left to agree on is the commands directory they both write into
# and the shape of the hooks they both read. This file is what keeps that true.
# It is copied verbatim into both plugins: neither can reach for the other's
# copy, because the other may not be there.
#
# Sourced, never run: `. "$here/shared.sh"`.

# A plugin says it is installed by leaving its version file in the tools
# directory. That is the only registry there is, and it is one both halves can
# read without either owning it.
kebacc_marker() {
    case "$1" in
        codex) printf '%s\n' .codex-version ;;
        *) printf '%s\n' .version ;;
    esac
}

# The slash commands each plugin owns. An installer sweeps its own names before
# copying, so an update leaves no stale command behind — and leaves the other
# half's commands exactly where they are.
kebacc_own_commands() {
    case "$1" in
        codex)
            printf '%s\n' kebacc-add-codex.md kebacc-list-codex.md \
                kebacc-switch-codex.md kebacc-remove-codex.md \
                kebacc-auto-codex.md kebacc-doctor-codex.md \
                kebacc-update-codex.md
            ;;
        *)
            printf '%s\n' kebacc-add-claude.md kebacc-list-claude.md \
                kebacc-switch-claude.md kebacc-remove-claude.md \
                kebacc-auto-claude.md kebacc-auto-toggle.md \
                kebacc-doctor.md kebacc-update.md kebacc-install-codex.md
            ;;
    esac
}

# Names earlier releases of each plugin installed and this one does not. They
# are swept on install and on uninstall: a command left behind with nothing to
# run is worse than no command, and the plugin that put it there is the only one
# that may remove it.
kebacc_stale_commands() {
    case "$1" in
        codex)
            printf '%s\n' kebacc-auto-codex-off.md kebacc-codex-add.md \
                kebacc-codex-list.md kebacc-codex-switch.md \
                kebacc-install-claude.md
            ;;
        *)
            printf '%s\n' kebacc-auto-claude-off.md refresh-a.md refresh-t.md
            ;;
    esac
}

# The commands that only mean something with both pools present. Neither plugin
# owns them: whichever installer finds the other half already there puts them
# in, and whichever uninstaller leaves last takes them out.
kebacc_all_commands() {
    printf '%s\n' kebacc-list-all.md kebacc-auto-all.md
}

# Which halves are on this machine, by their markers.
#
# `sh` has no portable `local`, so everything set inside these functions is set
# in the caller too. They are prefixed with _kb_ for that reason: an installer
# keeps its own $installed and $target whatever it calls here.
kebacc_installed() {
    _kb_tools=$1
    for _kb_id in claude codex; do
        [ -f "$_kb_tools/$(kebacc_marker "$_kb_id")" ] && printf '%s\n' "$_kb_id"
    done
    return 0
}

kebacc_has() {
    _kb_wanted=$1
    shift
    for _kb_one in "$@"; do
        [ "$_kb_one" = "$_kb_wanted" ] && return 0
    done
    return 1
}

# Semver enough for two version strings this project produces. Anything it
# cannot read compares as not newer, which makes the caller keep what it has.
kebacc_newer() {
    awk -v a="$1" -v b="$2" '
        function nums(s, out,   n, i, parts) {
            n = split(s, parts, /[.+-]/)
            i = 0
            for (p = 1; p <= n; p++)
                if (parts[p] ~ /^[0-9]+$/) out[++i] = parts[p] + 0
            return i
        }
        BEGIN {
            if (b == "") exit 0
            if (a == "") exit 1
            na = nums(a, x); nb = nums(b, y)
            if (na == 0 || nb == 0) exit 1
            for (i = 1; i <= (na > nb ? na : nb); i++) {
                p = (i <= na ? x[i] : 0); q = (i <= nb ? y[i] : 0)
                if (p > q) exit 0
                if (p < q) exit 1
            }
            exit 1
        }'
}

# Both plugins install the same executable, so the second one to arrive must not
# put an older build over a newer one. Same version, or a version neither side
# can read, means the file already there is good enough.
kebacc_should_replace_binary() {
    _kb_entry=$1
    _kb_version=$2
    [ -x "$_kb_entry" ] || return 0
    _kb_reported=$("$_kb_entry" --version 2>/dev/null | head -n 1 || true)
    [ -n "$_kb_reported" ] || return 0
    _kb_current=${_kb_reported##* }
    [ "$_kb_current" = "$_kb_version" ] && return 1
    kebacc_newer "$_kb_version" "$_kb_current"
}

# The -all commands exist exactly when both halves do. Prints how many files it
# added or removed, so the caller can stay quiet when it did nothing.
kebacc_sync_all_commands() {
    _kb_commands=$1
    _kb_all_source=$2
    _kb_tools_dir=$3
    _kb_installed=$(kebacc_installed "$_kb_tools_dir")
    _kb_touched=0
    for _kb_name in $(kebacc_all_commands); do
        _kb_target="$_kb_commands/$_kb_name"
        if kebacc_has claude $_kb_installed && kebacc_has codex $_kb_installed; then
            if [ -n "$_kb_all_source" ] && [ -f "$_kb_all_source/$_kb_name" ]; then
                cp -f -- "$_kb_all_source/$_kb_name" "$_kb_target"
                _kb_touched=$((_kb_touched + 1))
            fi
        elif [ -e "$_kb_target" ]; then
            rm -f -- "$_kb_target"
            _kb_touched=$((_kb_touched + 1))
        fi
    done
    printf '%s\n' "$_kb_touched"
}

# Both halves are installed, so the commands that span the two mean something.
kebacc_both_installed() {
    _kb_both=$(kebacc_installed "$1")
    kebacc_has claude $_kb_both && kebacc_has codex $_kb_both
}
