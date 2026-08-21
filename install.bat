@echo off
rem Double-click installer for kebacc on Windows.
rem
rem It downloads the published binary and asks it to install itself. The binary
rem carries the slash commands inside it, so there is nothing else to fetch:
rem no clone, no Rust toolchain, no administrator.
rem
rem The asset is fetched through the API rather than through the plain download
rem URL, which is served by a cache that keeps handing out the previous file for
rem a while after an asset is replaced.
rem
rem Invoke-RestMethod writes a JSON array as one object rather than as a stream
rem of them, so the answer is assigned before it is filtered. Filtering it inline
rem matches nothing once the repository has more than one release.
rem
rem The tag prefix, not "the newest release that is not a pre-release": the same
rem repository publishes the Codex half under kebacc-codex-v tags, and one of
rem those going out last would otherwise be the release this reaches into for a
rem file that is not on it.
rem
rem Arguments are passed through to the installer by name, so this works:
rem
rem   install.bat -StatusLine -AutoSwitch all
rem
rem Set KEBACC_NO_PAUSE to anything to skip the prompt at the end, which is what
rem CI does: there is nobody there to press a key.

setlocal

set "REPO=kebab1337420/kebacc-switch"
set "EXE=%TEMP%\kebacc-installer.exe"

rem The name is the one cmd/update.rs asks for, so the machine that updates
rem itself later looks for the same file it was installed from.
set "ASSET=kebacc-x86_64-pc-windows-msvc.exe"
if /i "%PROCESSOR_ARCHITECTURE%"=="ARM64" set "ASSET=kebacc-aarch64-pc-windows-msvc.exe"

rem PowerShell 7 when it is here, the one Windows ships with otherwise.
set "PS=powershell"
where pwsh >nul 2>&1 && set "PS=pwsh"

echo Fetching kebacc...
"%PS%" -NoProfile -ExecutionPolicy Bypass -Command "$ErrorActionPreference='Stop'; $ProgressPreference='SilentlyContinue'; try{[Net.ServicePointManager]::SecurityProtocol=[Net.SecurityProtocolType]::Tls12}catch{}; $h=@{'User-Agent'='kebacc-installer'}; if($env:GITHUB_TOKEN){$h['Authorization']='Bearer '+$env:GITHUB_TOKEN}; $answer=Invoke-RestMethod ('https://api.github.com/repos/'+$env:REPO+'/releases') -Headers $h; $r=@(@($answer) | Where-Object {-not $_.draft -and -not $_.prerelease -and $_.tag_name -like 'kebacc-v*'}) | Select-Object -First 1; if(-not $r){throw 'No kebacc-v* release has been published yet.'}; $a=@(@($r.assets) | Where-Object {$_.name -eq $env:ASSET}) | Select-Object -First 1; if(-not $a){throw ($r.tag_name+' has no '+$env:ASSET+' attached to it.')}; $d=$h.Clone(); $d['Accept']='application/octet-stream'; Invoke-WebRequest $a.url -OutFile $env:EXE -Headers $d"
if errorlevel 1 goto failed

echo Saved to %EXE%
"%EXE%" install %*
if errorlevel 1 goto failed

del "%EXE%" >nul 2>&1
echo.
echo Done. Restart Claude Code, then run /kebacc-add-claude to save the login you are on.
if defined KEBACC_NO_PAUSE exit /b 0
pause
exit /b 0

:failed
echo.
echo Install failed. The binary it was running is at %EXE% if you want to try it by hand.
if defined KEBACC_NO_PAUSE exit /b 1
pause
exit /b 1
