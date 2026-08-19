# Puts the Codex half of the switcher on this machine.
#
# The binary is kebacc-codex, and it carries the Codex pool only. kebacc-switch,
# the Claude half, is a separate program published from master; it may live in
# this same directory under its own name, and the two do not read or rewrite
# each other's binary, marker, hooks or status line.
[CmdletBinding()]
param(
    [string]$ToolsDir = (Join-Path $HOME '.claude-tools'),
    # The binary to install. Found in the workspace's target directory when this
    # is not given.
    [string]$Binary,
    # Run `auto` once as each session starts. Off by default: it changes which
    # login the next session answers as.
    [switch]$AutoSwitch,
    # Point the Claude Code status line at the switcher.
    [switch]$StatusLine,
    # Write KEBACC_SWITCH_UPDATE=off, so the switcher never updates itself.
    [switch]$NoAutoUpdate,
    [switch]$NoProfileEdit,
    # Install this build even when the one already there reports the same
    # version. What you want after rebuilding the same version from source.
    [switch]$Force
)

$ErrorActionPreference = 'Stop'

# The parts install.ps1 and uninstall.ps1 both need.
. (Join-Path $PSScriptRoot 'shared.ps1')

$pluginId = 'codex'
$version  = (Get-Content -LiteralPath (Join-Path $PSScriptRoot 'VERSION') -Raw).Trim()
$source   = Join-Path $PSScriptRoot 'src'
$claude   = if ($env:CLAUDE_CONFIG_DIR) { $env:CLAUDE_CONFIG_DIR } else { Join-Path $HOME '.claude' }
$exeName  = if ($IsWindows -or $env:OS -eq 'Windows_NT') { 'kebacc-codex.exe' } else { 'kebacc-codex' }

function Say { param([string]$Text, [string]$Color) if ($Color) { Write-Host $Text -ForegroundColor $Color } else { Write-Host $Text } }

function Find-SwitcherBinary {
    if ($Binary) {
        if (-not (Test-Path -LiteralPath $Binary)) { throw "No binary at $Binary." }
        return (Resolve-Path -LiteralPath $Binary).Path
    }
    $root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
    foreach ($profileDir in @('release', 'debug')) {
        $candidate = Join-Path $root (Join-Path 'target' (Join-Path $profileDir $exeName))
        if (Test-Path -LiteralPath $candidate) { return (Resolve-Path -LiteralPath $candidate).Path }
    }
    return $null
}

if (-not (Test-Path -LiteralPath $ToolsDir)) { New-Item -ItemType Directory -Path $ToolsDir -Force | Out-Null }
$entry = Join-Path $ToolsDir $exeName

# Beside kebacc-codex there may already be a binary, and a newer one than this
# plugin ships. Only when there is not does a build have to be found here.
$built = Find-SwitcherBinary
if ($Force -or (Test-KebaccShouldReplaceBinary -Entry $entry -Version $version)) {
    if (-not $built) {
        Say "No kebacc binary found. Build it first: cargo build --release -p kebacc-codex" Red
        exit 1
    }
    # A running switcher cannot be overwritten on Windows, and the status line
    # runs it often enough that this happens. Renaming the file out of the way
    # first is allowed while it is running, and the next uninstall or update
    # sweeps the .old up.
    try {
        Copy-Item -LiteralPath $built -Destination $entry -Force
    } catch {
        try {
            Move-Item -LiteralPath $entry -Destination "$entry.old" -Force
            Copy-Item -LiteralPath $built -Destination $entry -Force
        } catch {
            Say "Could not replace $entry - something is running it. Close it and try again." Red
            exit 1
        }
    }
    if (-not ($IsWindows -or $env:OS -eq 'Windows_NT')) { chmod 700 $entry }
    Say "Installed the switcher into $ToolsDir" Green
} else {
    Say "kebacc-codex already put a binary here - using it. -Force replaces it anyway." DarkGray
}

# The binary is asked what it is rather than taken on trust: a truncated copy
# and an executable a security product refuses to start both fail here, where
# the message can say so.
$reported = $null
try { $reported = (& $entry --version 2>$null | Select-Object -First 1) } catch {}
if (-not $reported) {
    Say "The binary at $entry would not run." Red
    Say 'A security product blocking it is the usual reason. Allow it, then run this again.' Yellow
    exit 1
}

# Not every build knows the codex pool: an older kebacc-codex may have put one
# here from before Codex came back into the crate. Saying so now beats five
# slash commands that answer with an unknown provider.
$knows = $false
try { $knows = [bool](& $entry list -Provider codex 2>&1 | Select-String -Quiet -SimpleMatch 'Codex') } catch {}
if (-not $knows) {
    Say "The binary at $entry does not know the codex pool." Red
    Say 'Update kebacc-codex, or pass -Binary pointing at a build that does.' Yellow
    exit 1
}

# The slash commands, which is how the switcher is used from inside Claude Code.
$commandSource = Join-Path $source 'commands'
$commandTarget = Join-Path $claude 'commands'
if (Test-Path -LiteralPath $commandSource) {
    if (-not (Test-Path -LiteralPath $commandTarget)) { New-Item -ItemType Directory -Path $commandTarget -Force | Out-Null }
    # This plugin's own commands, by name. kebacc-codex's commands live in this
    # same directory and are not ours to remove.
    foreach ($own in $script:KebaccOwnCommands[$pluginId]) {
        Remove-Item -LiteralPath (Join-Path $commandTarget $own) -Force -ErrorAction Ignore
    }
    # Names this half shipped under an earlier release, so an update does not
    # leave two of each behind.
    foreach ($dead in $script:KebaccStaleCommands[$pluginId]) {
        Remove-Item -LiteralPath (Join-Path $commandTarget $dead) -Force -ErrorAction Ignore
    }
    Copy-Item -Path (Join-Path $commandSource '*.md') -Destination $commandTarget -Force
    Say "Installed the Codex slash commands into $commandTarget" Green
}

# The version file is what says this half is installed, and it is what another
# switcher sharing the machine reads to know it is here.
Set-Content -LiteralPath (Join-Path $ToolsDir $script:KebaccMarkers[$pluginId]) -Value $version -NoNewline -Encoding utf8

# The commands that only mean something with both pools present. Neither plugin
# owns them, so whichever installer finds the other half already there puts them
# in - and this runs after the marker above, or it would not count itself.
$allSource = Join-Path $source 'commands-all'
if (Test-Path -LiteralPath $allSource) {
    $synced = Sync-KebaccAllCommands -CommandTarget $commandTarget -AllSource $allSource -Installed @(Get-KebaccInstalled -ToolsDir $ToolsDir)
    if ($synced.Both -and $synced.Touched) {
        Say "Installed the $($synced.Touched) command(s) that span both pools" Green
    }
}

# `kebacc-codex` as a shell function rather than a directory on the PATH. Same
# name and same body as the other plugin writes, so whichever runs second finds
# the line already there and leaves it alone.
if (-not $NoProfileEdit) {
    $marker = '# kebacc-codex account switcher'
    $block  = @(
        $marker
        "function kebacc-codex { & `"$entry`" @args }"
    ) -join [Environment]::NewLine

    $profilePath = $PROFILE.CurrentUserAllHosts
    $profileDir  = Split-Path -Parent $profilePath
    if (-not (Test-Path -LiteralPath $profileDir)) { New-Item -ItemType Directory -Path $profileDir -Force | Out-Null }
    $existing = if (Test-Path -LiteralPath $profilePath) { Get-Content -LiteralPath $profilePath -Raw } else { '' }
    if ($existing -notmatch [regex]::Escape($marker)) {
        Add-Content -LiteralPath $profilePath -Value ([Environment]::NewLine + $block)
        Say "Added kebacc-codex to $profilePath" Green
        Say 'Open a new shell for it to exist there.' DarkGray
    }
}

# settings.json belongs to the user, so it is read, amended and written back
# whole rather than rebuilt, and a copy is kept the first time this touches it.
$settingsPath = Join-Path $claude 'settings.json'

function Read-ClaudeSettings {
    if (-not (Test-Path -LiteralPath $settingsPath)) { return [pscustomobject]@{} }
    return Get-Content -LiteralPath $settingsPath -Raw | ConvertFrom-Json
}

function Write-ClaudeSettings {
    param([psobject]$Settings)
    if (Test-Path -LiteralPath $settingsPath) {
        if (-not (Test-Path -LiteralPath "$settingsPath.cc-backup")) {
            Copy-Item -LiteralPath $settingsPath -Destination "$settingsPath.cc-backup" -Force
        }
        Copy-Item -LiteralPath $settingsPath -Destination "$settingsPath.cc-backup.prev" -Force
    }
    $json = $Settings | ConvertTo-Json -Depth 100
    try { $null = $json | ConvertFrom-Json } catch {
        Say "Refusing to rewrite $settingsPath - the result would not parse. Nothing changed." Red
        exit 1
    }
    [IO.File]::WriteAllText($settingsPath, $json, [Text.UTF8Encoding]::new($false))
}

# Forward slashes throughout: these strings end up in JSON, where a backslash is
# an escape.
$command = ($entry -replace '\\', '/')

# settings.json belongs to the user, so the switcher edits it itself rather than
# a second implementation of that read-amend-write living here.
if ($StatusLine -or $NoAutoUpdate) {
    $wireArgs = @()
    if ($StatusLine) { $wireArgs += '-StatusLine' }
    if ($NoAutoUpdate) { $wireArgs += '-NoAutoUpdate' }
    & $entry wire @wireArgs | Out-Null
    if ($LASTEXITCODE -ne 0) { Say "wire exited $LASTEXITCODE - the settings were left alone." Red; exit 1 }
    if ($StatusLine) { Say "Pointed the Claude Code status line at the switcher ($settingsPath)" Green }
    if ($NoAutoUpdate) { Say "The switcher will not update itself: KEBACC_SWITCH_UPDATE=off ($settingsPath)" Yellow }
}

if ($AutoSwitch) {
    $settings = Read-ClaudeSettings
    $hooks = if ($settings.PSObject.Properties['hooks']) { $settings.hooks } else { [pscustomobject]@{} }

    # Only the hooks running kebacc-codex are read and rewritten here, and the
    # scope already on them is widened rather than overwritten, so rearming
    # keeps whatever an earlier version put there. The Claude switcher runs a
    # binary of its own: its hooks are not touched, and it cannot touch these.
    $armed = (Read-KebaccArmedScope -Hooks $hooks -Event 'SessionStart').Armed
    $scope = Merge-KebaccAutoScope -Existing $armed -Adding 'codex'

    Set-KebaccAutoHooks -Hooks $hooks -Entry $command -Scope $scope
    $settings | Add-Member -NotePropertyName hooks -NotePropertyValue $hooks -Force
    Write-ClaudeSettings $settings
    Say "auto -Provider $scope now runs at every session start, and during a task as the quota runs out." Green
    if ($armed -and $scope -ne 'codex') {
        Say "  ($armed was already armed, so the one hook now covers both.)" DarkGray
    }
}

Say ''
Say "kebacc-codex $version is installed." Green
Say '  /kebacc-add-codex       save the Codex login you are on' DarkGray
Say '  /kebacc-list-codex      what is saved, and its quota' DarkGray
Say '  /kebacc-switch-codex    move to another saved login' DarkGray
exit 0
