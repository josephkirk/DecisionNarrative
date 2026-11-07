@echo off
setlocal
rem Build the studio frontend and run the local service.
rem   run.bat          -> default database (data\storylet_studio_v5.db), http://127.0.0.1:3000
rem   run.bat fresh    -> clean database (data\storylet_studio_v5_clean.db), port 3001
rem Stops any active server from a previous run before starting.
cd /d "%~dp0"

rem Stop any active server so ports 3000/3001 are free (the build takes a
rem while, giving the OS time to release the ports before the new server binds).
echo [run] Stopping any active server...
taskkill /F /IM storylet-engine-service.exe /T >nul 2>&1
for %%P in (3000 3001) do (
  for /f "tokens=5" %%Q in ('netstat -ano ^| findstr /R /C:":%%P .*LISTENING"') do (
    taskkill /F /PID %%Q >nul 2>&1
  )
)

where bun >nul 2>nul || (echo [error] bun not found. Install Bun 1.3.14 or later: https://bun.sh & exit /b 1)
where cargo >nul 2>nul || (echo [error] cargo not found. Install Rust 1.90 or later: https://rustup.rs & exit /b 1)

if not exist node_modules (
  echo [setup] Installing dependencies...
  call bun install || exit /b 1
)

if /i "%~1"=="fresh" (
  set "SCRIPT=play:fresh"
  echo [run] Fresh database on http://127.0.0.1:3001
) else (
  set "SCRIPT=play"
  echo [run] Default database on http://127.0.0.1:3000
)

call bun run %SCRIPT%
exit /b %ERRORLEVEL%
