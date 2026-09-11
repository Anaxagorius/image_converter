@echo off
setlocal enabledelayedexpansion

echo ============================================
echo  Image Converter - Bootstrap Installer
echo ============================================
echo.

:: Check for git
where git >nul 2>&1
if errorlevel 1 (
    echo [ERROR] git is not installed or not on PATH.
    echo Please install Git from https://git-scm.com/download/win
    echo Then re-run this script.
    pause
    exit /b 1
)

:: Check for cargo
where cargo >nul 2>&1
if errorlevel 1 (
    echo Rust/cargo not found. Installing Rust via rustup...
    echo.
    curl --proto "=https" --tlsv1.2 -sSf -o "%TEMP%\rustup-init.exe" https://win.rustup.rs/x86_64
    if errorlevel 1 (
        echo [ERROR] Failed to download rustup installer.
        pause
        exit /b 1
    )
    "%TEMP%\rustup-init.exe" -y --default-toolchain stable
    if errorlevel 1 (
        echo [ERROR] Rust installation failed.
        pause
        exit /b 1
    )
    :: Add cargo to PATH for this session
    set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
    echo.
    echo Rust installed successfully.
)

:: Determine install location
set "INSTALL_DIR=%USERPROFILE%\image_converter"

if exist "%INSTALL_DIR%" (
    if not exist "%INSTALL_DIR%\.git" (
        echo [ERROR] %INSTALL_DIR% exists but is not a git repository. Please remove it and re-run.
        pause
        exit /b 1
    )
    echo Repository already exists at %INSTALL_DIR%. Pulling latest changes...
    cd /d "%INSTALL_DIR%"
    git pull
    if errorlevel 1 (
        echo [ERROR] git pull failed. Check for merge conflicts or network issues.
        pause
        exit /b 1
    )
) else (
    echo Cloning repository...
    git clone https://github.com/Anaxagorius/image_converter.git "%INSTALL_DIR%"
    if errorlevel 1 (
        echo [ERROR] Failed to clone repository.
        pause
        exit /b 1
    )
    cd /d "%INSTALL_DIR%"
)

echo.
echo Building image-converter (this may take several minutes on first run)...
cargo build --release
if errorlevel 1 (
    echo [ERROR] Build failed.
    pause
    exit /b 1
)

echo.
echo ============================================
echo  Build complete!
echo  Binary: %INSTALL_DIR%\target\release\image-converter.exe
echo ============================================
echo.

set /p LAUNCH="Launch image-converter now? (Y/N): "
if /i "%LAUNCH%"=="Y" (
    start "" "%INSTALL_DIR%\target\release\image-converter.exe"
)

pause
