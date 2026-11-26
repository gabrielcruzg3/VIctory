@echo off
setlocal
rem victory-3060TI — Quality / Glitch-Fix preset (water/boat intact, sharper mips)
rem Usage: play-3060ti-quality.bat [quiet]

set "REPO=%~dp0.."
set "EXE=%REPO%\target\release\victory.exe"
if "%GAME_DIR%"=="" set "GAME_DIR=E:\extcardd\Downloads\GTA6.Xbox.v1.0.2.1-CYBERLEEK"
if "%WORK_DIR%"=="" set "WORK_DIR=E:\extcardd\Downloads\victory_3060ti_work"

set "VICTORY_GAME_DIR=%GAME_DIR%"
set "VICTORY_CACHE_DIR=%WORK_DIR%\cache"
set "VICTORY_DATA_DIR=%WORK_DIR%\data"
set "VICTORY_LOG=warn,victory_gpu::host=info,victory_gpu::texback=info,victory_gpu::ambient=info"
if /i "%~1"=="quiet" set "VICTORY_LOG=error,victory_gpu::host=info"

set "CONSOLE=series-s"
set "VICTORY_TEXTURE_DROP_MIPS=2"
set "VICTORY_CARD_WINDOWS_MB=2048"
set "VICTORY_CARD_BUFFERS_MB=1024"
set "VICTORY_CARD_BUFFERS_MIN_KB=1024"
set "VICTORY_WINDOW_UPLOAD=1"
set "VICTORY_CARD_REHASH_RUNS=1"
set "VICTORY_JOIN_COPIES=1"
set "VICTORY_LEVELS_ON_DEMAND=0"
set "VICTORY_QUEUE2_REFETCH=0"
set "VICTORY_TEXTURE_BACK=1"
set "VICTORY_LATE_WAIT_LISTS=512"
set "VICTORY_MERGE_RUNS=4096"
set "VICTORY_MERGE_MASK=1"
set "VICTORY_ABSENT_ONCE=1"
set "VICTORY_ABSENT_MS=told"
set "VICTORY_LIGHT_REFRESH=1"
set "VICTORY_GUARD_OUTSIDE=1"
set "VICTORY_VIEW_KNOWN=1"
set "VICTORY_REMAP_LEAN=1"
set "VICTORY_PORT_CONSTANTS=1"
set "VICTORY_AMBIENT=0.6"
set "VICTORY_SKIP_WC_COMPARE=1"
set "VICTORY_WIDE_TEXTURES=1"
set "VICTORY_COPY_PLAIN=1"
set "VICTORY_INDIRECT_GATHER=1"
set "VICTORY_HEAP_WORKER=1"
set "VICTORY_ALIAS_ONCE=1"
set "VICTORY_JOIN_PARTS=1"
set "VICTORY_BACK_OWN_VIEW=1"
set "VICTORY_INDIRECT_WINDOW_DRAWS=1"
set "VICTORY_GUARD_HOLES_FRAME=1"
set "VICTORY_COPY_CHANGED=1"
set "VICTORY_CLAMP_WAITS=0"
set "VICTORY_WIDE_MERGE=1"
set "VICTORY_RELAY_OWN_QUEUE=1"

if not exist "%EXE%" (echo Binary not found: %EXE% & echo Run cargo build --release first. & pause & exit /b 1)
if not exist "%GAME_DIR%\game_scarlett_master.exe" (echo Game not found in %GAME_DIR%. Set GAME_DIR= first. & pause & exit /b 1)
if not exist "%VICTORY_DATA_DIR%" mkdir "%VICTORY_DATA_DIR%"
if not exist "%WORK_DIR%\logs" mkdir "%WORK_DIR%\logs"

for /f %%t in ('powershell -NoProfile -Command "Get-Date -Format yyyyMMdd-HHmmss"') do set "NOW=%%t"
if exist "%VICTORY_DATA_DIR%\7952987A\saves" robocopy "%VICTORY_DATA_DIR%\7952987A\saves" "%WORK_DIR%\backup-saves\%NOW%" /E /NFL /NDL /NJH /NJS /NP >nul

set "N=1"
:next_log
if exist "%WORK_DIR%\logs\3060ti-quality-%N%.log" (set /a N+=1 & goto next_log)

echo victory-3060TI [QUALITY / GLITCH-FIX] (profile %CONSOLE%)
echo Log: %WORK_DIR%\logs\3060ti-quality-%N%.log
"%EXE%" run "%GAME_DIR%\game_scarlett_master.exe" --execute --console %CONSOLE% > "%WORK_DIR%\logs\3060ti-quality-%N%.log" 2>&1
echo Exited with code %errorlevel%.
pause
exit /b 0
