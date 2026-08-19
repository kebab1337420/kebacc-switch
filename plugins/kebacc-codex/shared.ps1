# What the two kebacc plugins have to agree on to share one machine.
#
# kebacc-switch, published from master, carries the Claude Code pool;
# kebacc-codex carries the Codex one. Two binaries, two names, one machine.
# They ship separately and neither can disturb the other: each has its own
# binary name, its own version marker, its own release tags and its own pair of
# hooks. What is left to agree on is the commands directory they both write into
# and the shape of the hooks they both read. This file is what keeps that true.
# It is copied verbatim into both plugins: neither can reach for the other's
# copy, because the other may not be there.

# A plugin says it is installed by leaving its version file in the tools
# directory. That is the only registry there is, and it is one both halves can
# read without either owning it.
$script:KebaccMarkers = [ordered]@{ claude = '.version'; codex = '.codex-version' }

# The slash commands each plugin owns. An installer sweeps its own names before
# copying, so an update leaves no stale command behind — and leaves the other
# half's commands exactly where they are.
$script:KebaccOwnCommands = @{
    claude = @('kebacc-add-claude.md', 'kebacc-list-claude.md', 'kebacc-switch-claude.md',
               'kebacc-remove-claude.md', 'kebacc-auto-claude.md', 'kebacc-auto-toggle.md',
               'kebacc-doctor.md', 'kebacc-update.md',
               'kebacc-install-codex.md')
    codex  = @('kebacc-add-codex.md', 'kebacc-list-codex.md', 'kebacc-switch-codex.md',
               'kebacc-remove-codex.md', 'kebacc-auto-codex.md', 'kebacc-doctor-codex.md')
}

# Names each plugin shipped under an earlier release and no longer does. Swept
# on install and on uninstall, so an update does not leave two of each in the
# commands directory. Per plugin, for the same reason the list above is: the
# other half's leftovers are not ours to remove.
$script:KebaccStaleCommands = @{
    claude = @('kebacc-auto-claude-off.md', 'refresh-a.md', 'refresh-t.md')
    codex  = @('kebacc-auto-codex-off.md', 'kebacc-codex-add.md',
               'kebacc-codex-list.md', 'kebacc-codex-switch.md',
               'kebacc-install-claude.md')
}

# The commands that only mean something with both pools present. Neither plugin
# owns them: whichever installer finds the other half already there puts them
# in, and whichever uninstaller leaves last takes them out.
$script:KebaccAllCommands = @('kebacc-list-all.md', 'kebacc-auto-all.md')

function Get-KebaccInstalled {
    param([Parameter(Mandatory)][string]$ToolsDir)
    $found = @()
    foreach ($id in $script:KebaccMarkers.Keys) {
        $marker = Join-Path $ToolsDir $script:KebaccMarkers[$id]
        if (Test-Path -LiteralPath $marker -PathType Leaf) { $found += $id }
    }
    return $found
}

function Get-KebaccPluginVersion {
    param([Parameter(Mandatory)][string]$ToolsDir, [Parameter(Mandatory)][string]$Id)
    $marker = Join-Path $ToolsDir $script:KebaccMarkers[$Id]
    if (-not (Test-Path -LiteralPath $marker -PathType Leaf)) { return $null }
    $text = (Get-Content -LiteralPath $marker -Raw -ErrorAction Ignore)
    if ($null -eq $text) { return $null }
    $text = $text.Trim()
    if (-not $text) { return $null }
    return $text
}

# Semver enough for two version strings this project produces. Anything it
# cannot read compares as equal, which makes the caller keep what it has.
function Test-KebaccNewer {
    param([string]$Candidate, [string]$Current)
    if (-not $Current) { return $true }
    if (-not $Candidate) { return $false }
    $a = @(); $b = @()
    foreach ($part in ($Candidate -split '[.\-+]')) { if ($part -match '^\d+$') { $a += [int]$part } }
    foreach ($part in ($Current   -split '[.\-+]')) { if ($part -match '^\d+$') { $b += [int]$part } }
    if (-not $a.Count -or -not $b.Count) { return $false }
    for ($i = 0; $i -lt [Math]::Max($a.Count, $b.Count); $i++) {
        $x = if ($i -lt $a.Count) { $a[$i] } else { 0 }
        $y = if ($i -lt $b.Count) { $b[$i] } else { 0 }
        if ($x -gt $y) { return $true }
        if ($x -lt $y) { return $false }
    }
    return $false
}

# Both plugins install the same executable, so the second one to arrive must not
# put an older build over a newer one. Same version, or a version neither side
# can read, means the file already there is good enough.
function Test-KebaccShouldReplaceBinary {
    param([Parameter(Mandatory)][string]$Entry, [Parameter(Mandatory)][string]$Version)
    if (-not (Test-Path -LiteralPath $Entry -PathType Leaf)) { return $true }
    $reported = $null
    try { $reported = (& $Entry --version 2>$null | Select-Object -First 1) } catch {}
    if (-not $reported) { return $true }
    $current = ($reported -split '\s+' | Where-Object { $_ })[-1]
    if ($current -eq $Version) { return $false }
    return (Test-KebaccNewer -Candidate $Version -Current $current)
}

# The -all commands exist exactly when both halves do.
function Sync-KebaccAllCommands {
    param(
        [Parameter(Mandatory)][string]$CommandTarget,
        [Parameter(Mandatory)][AllowEmptyString()][string]$AllSource,
        [Parameter(Mandatory)][AllowEmptyCollection()][string[]]$Installed
    )
    $both = ($Installed -contains 'claude') -and ($Installed -contains 'codex')
    $touched = 0
    foreach ($name in $script:KebaccAllCommands) {
        $target = Join-Path $CommandTarget $name
        if ($both) {
            $from = if ($AllSource) { Join-Path $AllSource $name } else { $null }
            if ($from -and (Test-Path -LiteralPath $from -PathType Leaf)) {
                Copy-Item -LiteralPath $from -Destination $target -Force
                $touched++
            }
        } elseif (Test-Path -LiteralPath $target -PathType Leaf) {
            Remove-Item -LiteralPath $target -Force
            $touched++
        }
    }
    return [pscustomobject]@{ Both = $both; Touched = $touched }
}

# One session hook for both pools. Arming the second one widens the scope of the
# hook that is already there rather than adding a second hook beside it, which
# would run the switcher twice and let the two disagree about which login the
# session opens on.
#
# The binary does the same thing in `arm -Merge`, and the two have to answer the
# same for every input: an installer widens the scope through here, and the
# `/kebacc-auto-*` commands widen it through the binary. Kept line for line with
# `widen()` in crates/kebacc-codex/src/cmd/arm.rs, including what it does with
# a scope neither side knows.
function Merge-KebaccAutoScope {
    param([string]$Existing, [Parameter(Mandatory)][string]$Adding)
    if ($null -eq $Existing) { return $Adding }
    $had = $Existing.Trim().ToLower()
    if (-not $had -or $had -eq $Adding) { return $Adding }
    $known = @('claude', 'codex', 'all')
    if (($known -contains $had) -and ($known -contains $Adding)) { return 'all' }
    # A scope this version has never heard of is not something to widen, so the
    # pool asked for takes its place.
    return $Adding
}

# Taking one pool out of the scope of a hook that covers both. Returns $null
# when nothing is left to arm, and the caller drops the hook entirely.
function Split-KebaccAutoScope {
    param([string]$Existing, [Parameter(Mandatory)][string]$Removing)
    if (-not $Existing) { return $null }
    $pools = if ($Existing -eq 'all') { @('claude', 'codex') } else { @($Existing -split '\+') }
    $left = @()
    foreach ($pool in $pools) {
        $pool = $pool.Trim().ToLower()
        if ($pool -and $pool -ne $Removing.ToLower()) { $left += $pool }
    }
    if (-not $left.Count) { return $null }
    if (($left -contains 'claude') -and ($left -contains 'codex')) { return 'all' }
    # In the order they were read, as `narrow()` in the binary leaves them.
    return $left -join '+'
}

# The events auto is armed on, and the flag each one's command carries.
# SessionStart catches an account that was already out of quota. PreToolUse
# catches one that runs out halfway through a task, which is where a long job
# would otherwise sit on a capped account until the user noticed. PreToolUse is
# the one that needs a matcher, and it has to be every tool: a task made of
# nothing but edits would never check the quota otherwise.
$script:KebaccAutoEvents = [ordered]@{
    SessionStart = @{ Flag = '';         Timeout = 25; Matcher = $null }
    PreToolUse   = @{ Flag = ' -Midtask'; Timeout = 10; Matcher = '*' }
}

# Reads one event out of a settings hooks object and separates this half's hooks
# from everything else in it. Everything else means the user's own hooks and the
# Claude half's: that one runs a binary of its own, under its own name, so its
# hooks are none of this uninstaller's business. Returns the foreign groups
# untouched, and the scope our hooks were armed on.
function Read-KebaccArmedScope {
    param([Parameter(Mandatory)][psobject]$Hooks, [Parameter(Mandatory)][string]$Event)
    $others = @()
    $armed = $null
    if ($Hooks.PSObject.Properties[$Event]) {
        foreach ($group in @($Hooks.$Event)) {
            $mine = @($group.hooks | Where-Object { "$($_.command)" -like '*kebacc-codex*auto*' })
            if (-not $mine.Count) { $others += $group; continue }
            foreach ($hook in $mine) {
                if ("$($hook.command)" -match '(?i)-{1,2}provider\s+"?([a-z+]+)"?') {
                    $armed = Merge-KebaccAutoScope -Existing $armed -Adding $Matches[1]
                }
            }
        }
    }
    return [pscustomobject]@{ Others = $others; Armed = $armed }
}

# Arms every event on one scope. `-Hook` is what makes this safe to run this
# often: it prints nothing, where stdout would be fed to the model, and it exits
# 0, where a non-zero exit would be shown to the user - which `auto` returns for
# a pool too small to switch in, a normal state.
function Set-KebaccAutoHooks {
    param(
        [Parameter(Mandatory)][psobject]$Hooks,
        [Parameter(Mandatory)][string]$Entry,
        [Parameter(Mandatory)][string]$Scope
    )
    foreach ($event in $script:KebaccAutoEvents.Keys) {
        $shape = $script:KebaccAutoEvents[$event]
        $others = (Read-KebaccArmedScope -Hooks $Hooks -Event $event).Others
        $hook = [pscustomobject]@{
            type    = 'command'
            command = "`"$Entry`" auto -Provider $Scope -Hook$($shape.Flag)"
            timeout = $shape.Timeout
        }
        $group = if ($shape.Matcher) {
            [pscustomobject]@{ matcher = $shape.Matcher; hooks = @($hook) }
        } else {
            [pscustomobject]@{ hooks = @($hook) }
        }
        $Hooks | Add-Member -NotePropertyName $event -NotePropertyValue ($others + $group) -Force
    }
}

# Takes this plugin's pool out of every armed event. Returns the scope that is
# left, or $null when the hooks are gone entirely.
function Remove-KebaccAutoPool {
    param(
        [Parameter(Mandatory)][psobject]$Hooks,
        [Parameter(Mandatory)][string]$Pool,
        [Parameter(Mandatory)][string]$Entry
    )
    $narrowed = $null
    foreach ($event in $script:KebaccAutoEvents.Keys) {
        $read = Read-KebaccArmedScope -Hooks $Hooks -Event $event
        if (-not $read.Armed) { continue }
        $left = Split-KebaccAutoScope -Existing $read.Armed -Removing $Pool
        if ($left) { $narrowed = $left }
    }
    foreach ($event in $script:KebaccAutoEvents.Keys) {
        $read = Read-KebaccArmedScope -Hooks $Hooks -Event $event
        if (-not $Hooks.PSObject.Properties[$event]) { continue }
        if ($read.Others.Count) { $Hooks | Add-Member -NotePropertyName $event -NotePropertyValue $read.Others -Force }
        else { $Hooks.PSObject.Properties.Remove($event) }
    }
    if ($narrowed) { Set-KebaccAutoHooks -Hooks $Hooks -Entry $Entry -Scope $narrowed }
    return $narrowed
}
