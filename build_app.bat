@echo off
setlocal
title Building DayZ ImageSet Editor (All-in-One Portable)

echo =======================================================
echo   DayZ ImageSet Editor - All-in-One Release Builder
echo =======================================================
echo.

:: ตรวจสอบไฟล์ Tools และ Resources ที่จำเป็นสำหรับการ Embed
if not exist "edds2png.exe" (
    echo [ERROR] Missing "edds2png.exe" in project folder!
    pause
    exit /b 1
)

if not exist "imagesetconv.exe" (
    echo [ERROR] Missing "imagesetconv.exe" in project folder!
    pause
    exit /b 1
)

if not exist "app_icon.ico" (
    echo [ERROR] Missing "app_icon.ico" in project folder!
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
copy /Y "target\release\dayz_imageset_editor.exe" "..\dayz_imageset_editor.exe"

echo.
echo =======================================================
echo [SUCCESS] Everything bundled into single executable!
echo Output: "P:\workbrench dayz tools\dayz_imageset_editor.exe"
echo =======================================================
echo.
pause
