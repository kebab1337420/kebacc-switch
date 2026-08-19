# Installs kebacc-codex on a machine that has no clone of this repository and
# no Rust toolchain.
#
# It fetches the newest release, unpacks the plugin from the source archive of
# that same tag, and hands the published binary to install.ps1, which does the
# actual work. Everything lands in a temporary directory that is removed on the
# way out; the only lasting writes are the ones install.ps1 makes.
#
# install-codex.bat is a launcher for this file. Running this directly works
# too:
#
#   pwsh -NoProfile -File bootstrap.ps1 -StatusLine -AutoSwitch
#
# Any parameter this script does not recognise is passed straight through.
[CmdletBinding()]
param(
    # A specific release to install, as its tag. The newest one is used when
    # this is not given, prereleases included.
    [string]$Tag,
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$InstallerArgs
)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$repo = 'kebab1337420/kebacc-switch'
$asset = 'kebacc-codex-x86_64-pc-windows-msvc.exe'
# Releases of the Claude half share this repository, under kebacc-switch-v*.
$tagPrefix = 'kebacc-codex-v'
# The plugin's directory in the source archive, which is not the pool's name.
$pluginDir = 'kebacc-codex'

function Say { param([string]$Text, [string]$Color) if ($Color) { Write-Host $Text -ForegroundColor $Color } else { Write-Host $Text } }

if (-not ($IsWindows -or $env:OS -eq 'Windows_NT')) {
    Say 'This is the Windows bootstrap. On macOS or Linux, run bootstrap.sh instead:' Red
    Say '  curl -fsSL https://github.com/kebab1337420/kebacc-switch/releases/download/kebacc-codex-v0.2.6/bootstrap.sh | sh' Yellow
    exit 1
}

# TLS 1.2 is not the default on the PowerShell that ships with Windows 10, and
# GitHub answers nothing else.
try { [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12 } catch {}

$headers = @{
    'User-Agent' = 'kebacc-codex-bootstrap'
    'Accept'     = 'application/vnd.github+json'
}

# GitHub answers 403 rather than a rate-limit message once an address has asked
# too often, which on a shared address can be the first request of the day. A
# token in GITHUB_TOKEN or GH_TOKEN raises that limit; without one this asks
# anonymously, which is enough for a machine installing this once.
$token = if ($env:GITHUB_TOKEN) { $env:GITHUB_TOKEN } else { $env:GH_TOKEN }
if ($token) { $headers['Authorization'] = "Bearer $token" }

# /releases/latest skips prereleases, and this project has been one so far, so
# the whole list is read and the newest published entry taken from it.
try {
    $answer = Invoke-RestMethod -Uri "https://api.github.com/repos/$repo/releases" -Headers $headers
} catch {
    Say "GitHub did not answer: $($_.Exception.Message)" Red
    exit 1
}
# Invoke-RestMethod writes a JSON array as one object rather than as a stream of
# them, so @(Invoke-RestMethod ...) is an array holding an array, and filtering
# it matches nothing. Assigning first and wrapping after unrolls it properly.
# With a single release the difference does not show; from the second one on,
# every lookup comes back empty.
$releases = @($answer)
# The repository publishes both halves of the switcher, under a tag prefix each.
# Without the prefix filter the newest release could be the Claude one, whose
# assets this script would then look for a Codex binary in.
$published = @($releases | Where-Object { -not $_.draft -and $_.tag_name -like "$tagPrefix*" })
if ($Tag) {
    $release = @($published | Where-Object { $_.tag_name -eq $Tag }) | Select-Object -First 1
    if (-not $release) { Say "$repo has no published release tagged $Tag." Red; exit 1 }
} else {
    Say 'Looking up the newest release...' Cyan
    $release = $published | Select-Object -First 1
    if (-not $release) { Say "No $tagPrefix* release published at $repo yet." Red; exit 1 }
    $Tag = $release.tag_name
}

# Asked for by name rather than by a URL built from the tag, so a release that
# was published without its binary says so instead of handing back GitHub's
# 404 page.
$download = @(@($release.assets) | Where-Object { $_.name -eq $asset }) | Select-Object -First 1
if (-not $download) {
    Say "$Tag has no $asset attached to it, so there is nothing to install." Red
    Say "Releases: https://github.com/$repo/releases" Yellow
    exit 1
}
Say "Installing $Tag" Cyan

$work = Join-Path ([IO.Path]::GetTempPath()) ('kebacc-codex-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $work -Force | Out-Null

try {
    $zip = Join-Path $work 'source.zip'
    Invoke-WebRequest -Uri "https://github.com/$repo/archive/refs/tags/$Tag.zip" -OutFile $zip -Headers @{ 'User-Agent' = 'kebacc-codex-bootstrap' }
    Expand-Archive -LiteralPath $zip -DestinationPath $work -Force

    $installer = Get-ChildItem -LiteralPath $work -Recurse -Filter 'install.ps1' -File |
        Where-Object { $_.DirectoryName -like "*$pluginDir" } |
        Select-Object -First 1
    if (-not $installer) { throw "The $Tag source archive has no plugins/$pluginDir/install.ps1." }

    # Fetched through the API rather than through browser_download_url, which is
    # served by a cache that keeps handing out the previous file for a while
    # after an asset is replaced. The API URL answers with the current one.
    $exe = Join-Path $work 'kebacc-codex.exe'
    $binaryHeaders = @{ 'User-Agent' = 'kebacc-codex-bootstrap'; 'Accept' = 'application/octet-stream' }
    if ($token) { $binaryHeaders['Authorization'] = "Bearer $token" }
    Invoke-WebRequest -Uri $download.url -OutFile $exe -Headers $binaryHeaders
    if ((Get-Item -LiteralPath $exe).Length -lt 100KB) { throw "$asset came back too small to be the binary." }

    # ValueFromRemainingArguments hands back a single empty string when there is
    # nothing left over, and splatting that fills install.ps1's first positional
    # parameter with it.
    #
    # The leftovers are rebuilt into a hashtable rather than splatted as an
    # array: an array splat binds every element by position, so a `-Name value`
    # pair typed on the command line arrives as two positional arguments and
    # lands on whichever parameters happen to sit at those positions. A name
    # whose next leftover is another name is a switch and is passed as true.
    $passthru = @{}
    $left = @(@($InstallerArgs) | Where-Object { $_ })
    for ($i = 0; $i -lt $left.Count; $i++) {
        $item = $left[$i]
        if ($item -notlike '-*') { throw "Unexpected argument '$item'. Pass installer options by name, as -StatusLine or -AutoSwitch all." }
        $name = $item.TrimStart('-')
        if ($i + 1 -lt $left.Count -and $left[$i + 1] -notlike '-*') {
            $passthru[$name] = $left[$i + 1]
            $i++
        } else {
            $passthru[$name] = $true
        }
    }
    & $installer.FullName -Binary $exe @passthru
    if ($LASTEXITCODE) { exit $LASTEXITCODE }
    exit 0
} finally {
    Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue
}
