@echo off
rem Double-click installer for kebacc-switch on Windows.
rem
rem It downloads bootstrap.ps1 from the repository, shows you where it landed,
rem and runs it. That script fetches the newest release and installs it. Nothing
rem here needs a clone, a Rust toolchain, or an administrator.
rem
rem Arguments are passed through to the installer, so this works:
rem
rem   install.bat -StatusLine -AutoSwitch all

setlocal

set "BOOTSTRAP=https://raw.githubusercontent.com/kebab1337420/kebacc-switch/master/plugins/kebacc-switch/bootstrap.ps1"
set "SCRIPT=%TEMP%\kebacc-switch-bootstrap.ps1"

rem PowerShell 7 when it is here, the one Windows ships with otherwise.
set "PS=powershell"
where pwsh >nul 2>&1 && set "PS=pwsh"

echo Fetching the installer...
"%PS%" -NoProfile -ExecutionPolicy Bypass -Command "$ProgressPreference='SilentlyContinue'; try { [Net.ServicePointManager]::SecurityProtocol=[Net.SecurityProtocolType]::Tls12 } catch {}; Invoke-WebRequest -Uri $env:BOOTSTRAP -OutFile $env:SCRIPT -Headers @{'User-Agent'='kebacc-switch-installer'}"
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
