# Installs kebacc-switch on a machine that has no clone of this repository and
# no Rust toolchain.
#
# It fetches the newest release, unpacks the plugin from the source archive of
# that same tag, and hands the published binary to install.ps1, which does the
# actual work. Everything lands in a temporary directory that is removed on the
# way out; the only lasting writes are the ones install.ps1 makes.
#
# install.bat is a launcher for this file. Running this directly works too:
#
#   pwsh -NoProfile -File bootstrap.ps1 -StatusLine -AutoSwitch all
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
$asset = 'kebacc-switch-x86_64-pc-windows-msvc.exe'

function Say { param([string]$Text, [string]$Color) if ($Color) { Write-Host $Text -ForegroundColor $Color } else { Write-Host $Text } }

if (-not ($IsWindows -or $env:OS -eq 'Windows_NT')) {
    Say 'Only Windows has a published binary. On macOS or Linux, clone the repository and run: cargo build --release, then plugins/kebacc-switch/install.ps1' Red
    exit 1
}

# TLS 1.2 is not the default on the PowerShell that ships with Windows 10, and
# GitHub answers nothing else.
try { [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12 } catch {}

$headers = @{
    'User-Agent' = 'kebacc-switch-bootstrap'
    'Accept'     = 'application/vnd.github+json'
}

# /releases/latest skips prereleases, and this project has been one so far, so
# the whole list is read and the newest published entry taken from it.
try {
    $releases = @(Invoke-RestMethod -Uri "https://api.github.com/repos/$repo/releases" -Headers $headers)
} catch {
    Say "GitHub did not answer: $($_.Exception.Message)" Red
    exit 1
}
$published = @($releases | Where-Object { -not $_.draft })
if ($Tag) {
    $release = @($published | Where-Object { $_.tag_name -eq $Tag }) | Select-Object -First 1
    if (-not $release) { Say "$repo has no published release tagged $Tag." Red; exit 1 }
} else {
    Say 'Looking up the newest release...' Cyan
    $release = $published | Select-Object -First 1
    if (-not $release) { Say "No release published at $repo yet." Red; exit 1 }
    $Tag = $release.tag_name
}

# Asked for by name rather than by a URL built from the tag, so a release that
# was published without its binary says so instead of handing back GitHub's
# 404 page.
$download = @($release.assets | Where-Object { $_.name -eq $asset }) | Select-Object -First 1
if (-not $download) {
    Say "$Tag has no $asset attached to it, so there is nothing to install." Red
    Say "Releases: https://github.com/$repo/releases" Yellow
    exit 1
}
Say "Installing $Tag" Cyan

$work = Join-Path ([IO.Path]::GetTempPath()) ('kebacc-switch-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $work -Force | Out-Null

try {
    $zip = Join-Path $work 'source.zip'
    Invoke-WebRequest -Uri "https://github.com/$repo/archive/refs/tags/$Tag.zip" -OutFile $zip -Headers @{ 'User-Agent' = 'kebacc-switch-bootstrap' }
    Expand-Archive -LiteralPath $zip -DestinationPath $work -Force

    $installer = Get-ChildItem -LiteralPath $work -Recurse -Filter 'install.ps1' -File |
        Where-Object { $_.DirectoryName -like '*kebacc-switch' } |
        Select-Object -First 1
    if (-not $installer) { throw "The $Tag source archive has no plugins/kebacc-switch/install.ps1." }

    $exe = Join-Path $work 'kebacc-switch.exe'
    Invoke-WebRequest -Uri $download.browser_download_url -OutFile $exe -Headers @{ 'User-Agent' = 'kebacc-switch-bootstrap' }
    if ((Get-Item -LiteralPath $exe).Length -lt 100KB) { throw "$asset came back too small to be the binary." }

    & $installer.FullName -Binary $exe @InstallerArgs
    exit $LASTEXITCODE
} finally {
    Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue
}
