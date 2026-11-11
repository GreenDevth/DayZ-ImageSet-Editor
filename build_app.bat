@echo off
setlocal enabledelayedexpansion
cd /d "%~dp0"
title Building DayZ ImageSet Editor

echo =======================================================
echo   DayZ ImageSet Editor - All-in-One Release Builder
echo =======================================================
echo.

if not exist "%~dp0edds2png.exe" (
    echo [ERROR] Missing "edds2png.exe" in "%~dp0"!
    pause
    exit /b 1
)

if not exist "%~dp0imagesetconv.exe" (
    echo [ERROR] Missing "imagesetconv.exe" in "%~dp0"!
    pause
    exit /b 1
)

if not exist "%~dp0app_icon.ico" (
    echo [ERROR] Missing "app_icon.ico" in "%~dp0"!
    pause
    exit /b 1
)

echo [*] Embedded Binaries check passed:
echo     - edds2png.exe
echo     - imagesetconv.exe
echo     - app_icon.ico / app_icon.png
echo.
echo [*] Compiling Release executable via Cargo...
echo.

cargo build --release

if %ERRORLEVEL% NEQ 0 (
    echo.
    echo [ERROR] Build failed! Please check compiler errors above.
    echo.
    pause
    exit /b %ERRORLEVEL%
)

echo.
echo [*] Build Succeeded!
echo [*] Copying standalone binary to workspace root...
copy /Y "%~dp0target\release\dayz_imageset_editor.exe" "%~dp0..\dayz_imageset_editor.exe"

echo.
echo =======================================================
echo [SUCCESS] Everything bundled into single executable!
echo Output: "%~dp0..\dayz_imageset_editor.exe"
echo =======================================================
echo.
pause
