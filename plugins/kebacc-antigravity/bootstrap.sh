#!/bin/sh
# Installs kebacc-antigravity on a macOS or Linux machine that has no clone of this
# repository and no Rust toolchain. The counterpart of bootstrap.ps1.
#
#   curl -fsSL https://github.com/kebab1337420/kebacc-switch/releases/download/kebacc-antigravity-v0.2.6/bootstrap.sh | sh
#
# The url names a tag rather than `latest`, because `/releases/latest/` is a
# 404 while every release of a project is a prerelease, which every release of
# this one is so far. This script itself installs the newest release whatever
# tag it was downloaded from, so an older copy still installs the current
# version.
#
# Options after `sh -s --` are passed straight to install.sh:
#
#   curl -fsSL .../bootstrap.sh | sh -s -- --status-line --auto-switch
#
# It fetches the newest release, unpacks the plugin from the source archive of
# that same tag, and hands the published binary to install.sh, which does the
# actual work. Everything lands in a temporary directory that is removed on the
# way out; the only lasting writes are the ones install.sh makes.
set -eu

repo=kebab1337420/kebacc-switch
tag=""
plugin_dir=kebacc-antigravity

while [ $# -gt 0 ]; do
    case "$1" in
        # A specific release to install, as its tag. The newest one is used when
        # this is not given.
        --tag) tag="${2:?--tag needs a release tag}"; shift 2 ;;
        *) break ;;
    esac
done

red() { printf '\033[31m%s\033[0m\n' "$1" >&2; }
cyan() { printf '\033[36m%s\033[0m\n' "$1"; }

case "$(uname -s)" in
    Darwin) os=apple-darwin ;;
    Linux) os=unknown-linux-gnu ;;
    *) red "No published binary for $(uname -s). Clone the repository and run: cargo build --release, then plugins/$plugin_dir/install.sh"; exit 1 ;;
esac
case "$(uname -m)" in
    x86_64|amd64) arch=x86_64 ;;
    arm64|aarch64) arch=aarch64 ;;
    *) red "No published binary for $(uname -m)."; exit 1 ;;
esac
asset="kebacc-antigravity-$arch-$os"
# Releases of the Claude half share this repository, under kebacc-switch-v*.
tag_prefix=kebacc-antigravity-v

for needed in curl tar; do
    command -v "$needed" > /dev/null 2>&1 || { red "$needed is not on the PATH, and this downloads."; exit 1; }
done

# GitHub answers 403 rather than a rate-limit message once an address has asked
# too often, which on a shared address can be the first request of the day. A
# token in GITHUB_TOKEN or GH_TOKEN raises that limit; without one this asks
# anonymously, which is enough for a machine installing this once.
token="${GITHUB_TOKEN:-${GH_TOKEN:-}}"
api_get() {
    if [ -n "$token" ]; then
        curl -fsSL -H "Authorization: Bearer $token" -H 'User-Agent: kebacc-antigravity-bootstrap' "$@"
    else
        curl -fsSL -H 'User-Agent: kebacc-antigravity-bootstrap' "$@"
    fi
}

api="https://api.github.com/repos/$repo/releases"
if [ -z "$tag" ]; then
    cyan 'Looking up the newest release...'
    # The list rather than /releases/latest, which answers 404 while every
    # release of a project is still a prerelease — as this one's are. The list
    # comes back newest first, so the first release that is not a draft is the
    # one to install, prereleases included. bootstrap.ps1 asks the same way.
    answer=$(api_get -H 'Accept: application/vnd.github+json' "$api?per_page=20") || {
        red "GitHub did not answer, or nothing is published at $repo yet."
        exit 1
    }
    # Every release object carries its tag and its draft flag before its assets,
    # so flattening the answer and starting a line at each release gives one
    # line per release with both on it. Only the tag is taken here; the release
    # itself is asked for by tag below, whole and with its assets.
    # The tag prefix matters: this repository publishes the Claude half too,
    # under kebacc-switch-v*, and its releases carry no Antigravity binary.
    tag=$(printf '%s' "$answer" | tr -d '\r\n' | sed 's/"assets_url"/\
&/g' | grep '"tag_name"' | grep -v '"draft" *: *true' |
        sed -n 's/.*"tag_name" *: *"\('"$tag_prefix"'[^"]*\)".*/\1/p' | head -n 1)
    [ -n "$tag" ] || {
        red "$repo has no published $tag_prefix* release yet, only drafts."
        exit 1
    }
fi
release=$(api_get -H 'Accept: application/vnd.github+json' "$api/tags/$tag") || {
    red "$repo has no published release tagged $tag."
    exit 1
}
[ -n "$tag" ] || { red 'GitHub answered something with no tag in it.'; exit 1; }

# Every asset object starts at a brace, so flattening the answer to one line and
# splitting there gives one chunk per asset, with the name and the API url of
# that asset together in it. The newlines have to go first: GitHub pretty-prints,
# which would otherwise leave the name on a line of its own. Asked for
# by name rather than built from the tag, so a release published without its
# binary says so instead of handing back GitHub's 404 page.
asset_url=$(printf '%s' "$release" | tr -d '\r\n' | tr '{' '\n' | grep "\"name\": *\"$asset\"" | sed -n 's/.*"url" *: *"\(https:\/\/api[^"]*\)".*/\1/p' | head -n 1)
if [ -z "$asset_url" ]; then
    red "$tag has no $asset attached to it, so there is nothing to install."
    red "Releases: https://github.com/$repo/releases"
    exit 1
fi
cyan "Installing $tag"

work=$(mktemp -d 2>/dev/null || mktemp -d -t kebacc-antigravity)
cleanup() { rm -rf -- "$work"; }
trap cleanup EXIT INT TERM

api_get \
    "https://github.com/$repo/archive/refs/tags/$tag.tar.gz" -o "$work/source.tar.gz"
tar -xzf "$work/source.tar.gz" -C "$work"

installer=$(find "$work" -type f -path "*/plugins/$plugin_dir/install.sh" | head -n 1)
[ -n "$installer" ] || { red "The $tag source archive has no plugins/$plugin_dir/install.sh."; exit 1; }

# Fetched through the API url rather than through browser_download_url, which is
# served by a cache that keeps handing out the previous file for a while after
# an asset is replaced. The API url answers with the current one.
binary="$work/kebacc-antigravity"
api_get -H 'Accept: application/octet-stream' \
    "$asset_url" -o "$binary"
size=$(wc -c < "$binary")
[ "$size" -gt 102400 ] || { red "$asset came back too small to be the binary."; exit 1; }
chmod 700 "$binary"

sh "$installer" --binary "$binary" "$@"
