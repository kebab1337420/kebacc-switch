@echo off
rem Double-click installer for the Antigravity half, kebacc-antigravity, on Windows.
rem
rem It runs bootstrap.ps1, which installs the binary and this plugin. Another
rem switcher sharing the machine keeps its own slash commands: this widens the
rem pair of session hooks rather than replacing what they cover.
rem
rem It takes bootstrap.ps1 from the newest release and runs it. That script
rem fetches the published binary and the plugin and installs both. Nothing here
rem needs a clone, a Rust toolchain, or an administrator.
rem
rem The script comes from the release rather than from the branch: raw file URLs
rem are served through a cache that can be minutes behind, and an installer that
rem sometimes runs yesterday's code is worse than one pinned to a release.
rem
rem The script is fetched through the API rather than through the plain download
rem URL, which is served by a cache that keeps handing out the previous file for
rem a while after an asset is replaced.
rem
rem Invoke-RestMethod hands back a JSON array as one object, so @(...) around it
rem makes an array holding an array and any filter over that matches nothing.
rem ForEach-Object walks it out again before the filter runs.
rem
rem Arguments are passed through to the installer by name, so this works:
rem
rem   install-antigravity.bat -AutoSwitch
rem
rem It answers 2, not 1, when no release of this half is published yet: that is
rem nothing to install rather than a failure to install, and CI reads the two
rem differently.

setlocal

set "REPO=kebab1337420/kebacc-switch"
set "SCRIPT=%TEMP%\kebacc-antigravity-bootstrap.ps1"

rem PowerShell 7 when it is here, the one Windows ships with otherwise.
set "PS=powershell"
where pwsh >nul 2>&1 && set "PS=pwsh"

echo Fetching the installer...
"%PS%" -NoProfile -ExecutionPolicy Bypass -Command "$ErrorActionPreference='Stop'; $ProgressPreference='SilentlyContinue'; try{[Net.ServicePointManager]::SecurityProtocol=[Net.SecurityProtocolType]::Tls12}catch{}; $h=@{'User-Agent'='kebacc-antigravity-installer'}; $answer=Invoke-RestMethod ('https://api.github.com/repos/'+$env:REPO+'/releases') -Headers $h; $r=@($answer | ForEach-Object {$_} | Where-Object {-not $_.draft -and $_.tag_name -like 'kebacc-antigravity-v*'}) | Select-Object -First 1; if(-not $r){Write-Host 'No kebacc-antigravity-v* release has been published yet.'; exit 2}; $a=@($r.assets | ForEach-Object {$_} | Where-Object {$_.name -eq 'bootstrap.ps1'}) | Select-Object -First 1; if(-not $a){Write-Host ($r.tag_name+' has no bootstrap.ps1 attached to it.'); exit 2}; Invoke-WebRequest $a.url -OutFile $env:SCRIPT -Headers @{'User-Agent'='kebacc-antigravity-installer';'Accept'='application/octet-stream'}"
if errorlevel 2 goto unpublished
if errorlevel 1 goto failed

echo Saved to %SCRIPT%
"%PS%" -NoProfile -ExecutionPolicy Bypass -File "%SCRIPT%" %*
if errorlevel 1 goto failed

del "%SCRIPT%" >nul 2>&1
echo.
echo Done. Restart Claude Code, then run /kebacc-add-antigravity to save the login you are on.
call :wait
exit /b 0

:unpublished
echo.
echo Nothing to install yet: this half has no published release.
call :wait
exit /b 2

:failed
echo.
echo Install failed. The script it was running is at %SCRIPT% if you want to read it.
call :wait
exit /b 1

rem Held open so a double-click leaves its output readable. Set KEBACC_NO_PAUSE
rem when nobody is there to press a key.
:wait
if not defined KEBACC_NO_PAUSE pause
goto :eof
