# Installs the Codex switcher, which lives on its own branch.
#
# kebacc-switch handles Claude and nothing else. Codex has a plugin of its own,
# built from the `Codex` branch of the same repository. It installs into
# ~/.claude-tools beside this one, under its own name and its own version
# marker: the two share the directory and nothing else, and each uninstaller
# names its own files rather than sweeping the directory.
#
# There is no published release for it, so this clones the branch, builds it
# with cargo and hands the binary to the branch's own installer. Run it again to
# update. The saved logins are never touched.
[CmdletBinding()]
param(
    # Where the binary goes. Left empty on purpose: the branch's own installer
    # picks ~/.claude-tools, and only an explicit value overrides it.
    [string]$ToolsDir,
    # Where the branch comes from. A local checkout works as well as the URL,
    # which is what to pass when the branch has not been pushed yet.
    [string]$Source = 'https://github.com/kebab1337420/kebacc-switch.git',
    [string]$Branch = 'Codex',
    # Run `auto` once as each session starts, for the Codex pool. Off by
    # default: it changes which login the next session answers as.
    [switch]$AutoSwitch,
    # Leave the checkout on disk instead of deleting it. For looking at what was
    # built, or for building again without cloning again.
    [switch]$KeepCheckout
)

$ErrorActionPreference = 'Stop'
$onWindows = $IsWindows -or $env:OS -eq 'Windows_NT'
$exeName = if ($onWindows) { 'kebacc-codex.exe' } else { 'kebacc-codex' }

function Say { param([string]$Text, [string]$Color) if ($Color) { Write-Host $Text -ForegroundColor $Color } else { Write-Host $Text } }

foreach ($needed in @('git', 'cargo')) {
    if (-not (Get-Command $needed -ErrorAction Ignore)) {
        Say "$needed is not on the PATH, and this builds from source. Install it and run this again." Red
        exit 1
    }
}

$checkout = Join-Path ([IO.Path]::GetTempPath()) ("kebacc-codex-" + [Guid]::NewGuid().ToString('N').Substring(0, 8))

# --depth 1 on one branch: the history is not wanted, and cloning the whole
# repository to build one crate is a minute nobody asked for.
Say "Cloning $Branch from $Source" DarkGray
& git clone --quiet --depth 1 --branch $Branch -- $Source $checkout
if ($LASTEXITCODE -ne 0) {
    Say "Could not clone the $Branch branch from $Source." Red
    Say "If the branch only exists locally, point at that checkout: -Source <path>" Yellow
    exit 1
}

try {
    $manifest = Join-Path $checkout 'Cargo.toml'
    Say 'Building kebacc-codex, which takes a minute the first time.' DarkGray
    & cargo build --release --manifest-path $manifest -p kebacc-codex
    if ($LASTEXITCODE -ne 0) {
        Say 'The build failed. Nothing was installed.' Red
        exit 1
    }

    $binary = Join-Path $checkout (Join-Path 'target' (Join-Path 'release' $exeName))
    if (-not (Test-Path -LiteralPath $binary)) {
        Say "The build reported success but $binary is not there." Red
        exit 1
    }

    # The branch ships its own installer, and it is the one that knows which
    # slash commands are the codex ones and where the version marker goes.
    $installer = Join-Path $checkout (Join-Path 'plugins' (Join-Path 'kebacc-codex' 'install.ps1'))
    if (-not (Test-Path -LiteralPath $installer)) {
        Say "The $Branch branch has no plugins/kebacc-codex/install.ps1." Red
        exit 1
    }

    $arguments = @{ Binary = $binary }
    if ($ToolsDir) { $arguments['ToolsDir'] = $ToolsDir }
    if ($AutoSwitch) { $arguments['AutoSwitch'] = $true }
    & $installer @arguments
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
} finally {
    if ($KeepCheckout) {
        Say "The checkout is at $checkout" DarkGray
    } else {
        Remove-Item -LiteralPath $checkout -Recurse -Force -ErrorAction Ignore
    }
}
