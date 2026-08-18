@echo off
rem Double-click installer for kebacc-switch on Windows.
rem
rem It takes bootstrap.ps1 from the newest release and runs it. That script
rem fetches the published binary and the plugin and installs both. Nothing here
rem needs a clone, a Rust toolchain, or an administrator.
rem
rem The script comes from the release rather than from the branch: raw file URLs
rem are served through a cache that can be minutes behind, and an installer that
rem sometimes runs yesterday's code is worse than one pinned to a release.
rem
rem Arguments are passed through to the installer, so this works:
rem
rem   install.bat -StatusLine -AutoSwitch all

setlocal

set "REPO=kebab1337420/kebacc-switch"
set "SCRIPT=%TEMP%\kebacc-switch-bootstrap.ps1"

rem PowerShell 7 when it is here, the one Windows ships with otherwise.
set "PS=powershell"
where pwsh >nul 2>&1 && set "PS=pwsh"

echo Fetching the installer...
"%PS%" -NoProfile -ExecutionPolicy Bypass -Command "$ErrorActionPreference='Stop'; $ProgressPreference='SilentlyContinue'; try{[Net.ServicePointManager]::SecurityProtocol=[Net.SecurityProtocolType]::Tls12}catch{}; $h=@{'User-Agent'='kebacc-switch-installer'}; $r=@(Invoke-RestMethod ('https://api.github.com/repos/'+$env:REPO+'/releases') -Headers $h | Where-Object {-not $_.draft}) | Select-Object -First 1; if(-not $r){throw 'No release has been published yet.'}; $a=@($r.assets | Where-Object {$_.name -eq 'bootstrap.ps1'}) | Select-Object -First 1; if(-not $a){throw ($r.tag_name+' has no bootstrap.ps1 attached to it.')}; Invoke-WebRequest $a.browser_download_url -OutFile $env:SCRIPT -Headers $h"
if errorlevel 1 goto failed

echo Saved to %SCRIPT%
"%PS%" -NoProfile -ExecutionPolicy Bypass -File "%SCRIPT%" %*
if errorlevel 1 goto failed

del "%SCRIPT%" >nul 2>&1
echo.
echo Done. Restart Claude Code, then run /kebacc-add-claude to save the login you are on.
pause
exit /b 0

:failed
echo.
echo Install failed. The script it was running is at %SCRIPT% if you want to read it.
pause
exit /b 1
