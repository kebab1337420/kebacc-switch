# Takes back what the Codex installer put down. The saved logins are left
# alone: they are the expensive thing to rebuild, and a reinstall finds them
# again. So is everything kebacc-switch owns - it is a separate program, with a
# binary, hooks, a status line and a marker of its own, none of them read here.
[CmdletBinding()]
param(
    [string]$ToolsDir = (Join-Path $HOME '.claude-tools'),
    # Leave the shell profile alone. The profile is not under $ToolsDir and not
    # under CLAUDE_CONFIG_DIR, so it is the one thing a run against a sandbox
    # would still reach out and change.
    [switch]$NoProfileEdit,
    [switch]$Yes
)

$ErrorActionPreference = 'Stop'

# The half kebacc-codex shares: which commands are this plugin's, and how to
# take one pool out of a session hook that covers both.
. (Join-Path $PSScriptRoot 'shared.ps1')

$pluginId = 'codex'
$claude = if ($env:CLAUDE_CONFIG_DIR) { $env:CLAUDE_CONFIG_DIR } else { Join-Path $HOME '.claude' }
$exeName = if ($IsWindows -or $env:OS -eq 'Windows_NT') { 'kebacc-codex.exe' } else { 'kebacc-codex' }
# The binary is shared, so it goes only when this is the last half standing.
$alone = @(Get-KebaccInstalled -ToolsDir $ToolsDir | Where-Object { $_ -ne $pluginId }).Count -eq 0

function Say { param([string]$Text, [string]$Color) if ($Color) { Write-Host $Text -ForegroundColor $Color } else { Write-Host $Text } }

if (-not $Yes) {
    Say "This removes the Codex slash commands, and its half of the session hook."
    Say "The kebacc-codex binary in $ToolsDir goes with them."
    if (-not $alone) { Say 'kebacc-switch is installed too, and keeps everything of its own.' DarkGray }
    Say 'Saved logins are not touched.' DarkGray
    if ((Read-Host 'Continue? [y/N]') -notmatch '^(y|yes)$') { Say 'Nothing removed.'; exit 0 }
}

# Named one by one rather than removing the whole directory: kebacc-switch
# installs into this same $ToolsDir, and taking the directory would uninstall a
# plugin nobody asked about. Every name here is this half's own, so it goes
# whether or not the other half is there.
$ours = @($script:KebaccMarkers[$pluginId], $exeName, "$exeName.old",
          '.update-codex.json', 'update-codex.stamp')
if (Test-Path -LiteralPath $ToolsDir) {
    $removed = 0
    foreach ($name in $ours) {
        $file = Join-Path $ToolsDir $name
        if (Test-Path -LiteralPath $file -PathType Leaf) {
            Remove-Item -LiteralPath $file -Force
            $removed++
        }
    }
    # Half-finished updates: kebacc-codex.<pid>.new.
    Get-ChildItem -LiteralPath $ToolsDir -Filter 'kebacc-codex.*.new' -File -ErrorAction Ignore |
        ForEach-Object { Remove-Item -LiteralPath $_.FullName -Force; $removed++ }
    if ($removed) { Say "Removed $removed file(s) from $ToolsDir" Green }
    # Gone entirely when nothing else lives there.
    if (-not (Get-ChildItem -LiteralPath $ToolsDir -Force -ErrorAction Ignore)) {
        Remove-Item -LiteralPath $ToolsDir -Force
        Say "Removed $ToolsDir" Green
    }
}

$commands = Join-Path $claude 'commands'
if (Test-Path -LiteralPath $commands) {
    $gone = @()
    foreach ($own in ($script:KebaccOwnCommands[$pluginId] + $script:KebaccStaleCommands[$pluginId])) {
        $file = Join-Path $commands $own
        if (Test-Path -LiteralPath $file -PathType Leaf) { Remove-Item -LiteralPath $file -Force; $gone += $own }
    }
    if ($gone.Count) { Say "Removed $($gone.Count) slash command(s)" Green }
    # With one half gone the -all pair has nothing to span, so it goes with it.
    $null = Sync-KebaccAllCommands -CommandTarget $commands -AllSource '' -Installed @(Get-KebaccInstalled -ToolsDir $ToolsDir)
}

# The `kebacc-codex` shell function carries this binary's name, so it goes with
# this binary. kebacc-switch writes a function of its own, which this leaves.
$profilePath = $PROFILE.CurrentUserAllHosts
if (-not $NoProfileEdit -and (Test-Path -LiteralPath $profilePath)) {
    $lines = Get-Content -LiteralPath $profilePath
    $kept  = @()
    for ($i = 0; $i -lt $lines.Count; $i++) {
        if ($lines[$i] -match '^\s*#\s*kebacc-codex account switcher') { $i++; continue }
        $kept += $lines[$i]
    }
    if ($kept.Count -ne $lines.Count) {
        Set-Content -LiteralPath $profilePath -Value $kept -Encoding utf8
        Say "Removed kebacc-codex from $profilePath" Green
    }
}

# The session hook. It is one hook for both pools, so this narrows its scope
# when kebacc-codex is still armed and only removes it when nothing is left.
# A hook left pointing at a binary that is gone is worse than a stale setting:
# it fails at the start of every session the user opens from here on.
$settingsPath = Join-Path $claude 'settings.json'
if (Test-Path -LiteralPath $settingsPath) {
    try {
        $settings = Get-Content -LiteralPath $settingsPath -Raw | ConvertFrom-Json
        $touched = $false

        $hooks = $settings.PSObject.Properties['hooks']
        if ($hooks) {
            # Only the hooks running this binary are narrowed, and only their
            # scope: whatever the Claude switcher armed under its own name is
            # left alone. Forward slashes: the path ends up in JSON, where a
            # backslash is an escape.
            $entryPath = ((Join-Path $ToolsDir $exeName) -replace '\\', '/')
            $before = ($hooks.Value | ConvertTo-Json -Depth 100 -Compress)
            $narrowed = Remove-KebaccAutoPool -Hooks $hooks.Value -Pool $pluginId -Entry $entryPath
            if (($hooks.Value | ConvertTo-Json -Depth 100 -Compress) -ne $before) {
                if ($narrowed) { Say "Narrowed the auto hooks to -Provider $narrowed" Green }
                else { Say 'Removed the auto hooks from settings.json' Green }
                $touched = $true
            }
            if (-not $hooks.Value.PSObject.Properties.Name.Count) {
                $settings.PSObject.Properties.Remove('hooks')
            }
        }

        # Only a status line running this binary: one pointing at kebacc-switch
        # belongs to the other half and is left where it is.
        $line = $settings.PSObject.Properties['statusLine']
        if ($line -and "$($line.Value.command)" -like '*kebacc-codex*statusline*') {
            $settings.PSObject.Properties.Remove('statusLine')
            Say 'Removed the status line from settings.json' Green
            $touched = $true
        }

        if ($touched) {
            if (-not (Test-Path -LiteralPath "$settingsPath.cc-backup")) {
                Copy-Item -LiteralPath $settingsPath -Destination "$settingsPath.cc-backup" -Force
            }
            Copy-Item -LiteralPath $settingsPath -Destination "$settingsPath.cc-backup.prev" -Force
            $json = $settings | ConvertTo-Json -Depth 100
            try { $null = $json | ConvertFrom-Json } catch {
                Say "Refusing to rewrite ${settingsPath}: the result would not parse. Nothing changed." Red
                exit 1
            }
            [IO.File]::WriteAllText($settingsPath, $json, [Text.UTF8Encoding]::new($false))
        }
    } catch { }
}

Say ''
Say 'Uninstalled. The saved logins are still in ~/.kebacc-switch-codex-accounts.' DarkGray
exit 0
