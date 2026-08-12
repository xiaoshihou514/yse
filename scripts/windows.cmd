@echo off
setlocal EnableExtensions
pushd "%~dp0.." || exit /b 1
set "REPO=%CD%"
call "C:\Program Files\Microsoft Visual Studio\2022\Community\Common7\Tools\VsDevCmd.bat" -arch=x64 || exit /b 1
set "QT_ROOT=%APPDATA%\gansi\qt\6.8.3\windows\x86_64"
set "QMAKE=%QT_ROOT%\bin\qmake.exe"
set "PATH=%QT_ROOT%\bin;%USERPROFILE%\.cargo\bin;%PATH%"
set "CARGO_TARGET_DIR=%USERPROFILE%\.cargo\target\gansi"

if "%~1"=="diag" goto diag
if "%~1"=="setup" goto setup
if "%~1"=="build" goto build
if "%~1"=="check" goto check
if "%~1"=="smoke" goto smoke
if "%~1"=="bundle-smoke" goto bundle_smoke
if "%~1"=="example" goto example
if "%~1"=="run" goto run
echo unknown command: %~1 1>&2
exit /b 2

:diag
echo repo              : %REPO%
if exist Cargo.toml (echo manifest reachable: True) else (echo manifest reachable: False & exit /b 1)
if exist "%QMAKE%" ("%QMAKE%" -query QT_VERSION) else (echo qmake installed: False & exit /b 1)
cargo --version
exit /b %ERRORLEVEL%

:setup
cargo run -p gansi -- setup
exit /b %ERRORLEVEL%

:build
cargo build --workspace --exclude yse-media-converter
exit /b %ERRORLEVEL%

:check
cargo fmt --check || exit /b 1
cargo clippy --workspace --exclude yse-media-converter --all-targets -- -D warnings || exit /b 1
cargo test --workspace --exclude yse-media-converter || exit /b 1
exit /b 0

:smoke
set "QT_QPA_PLATFORM=offscreen"
set "YSE_SMOKE=1"
cargo run -p yse-ui --example settings || exit /b 1
cargo run -p yse-taskmgr || exit /b 1
exit /b 0

:bundle_smoke
set "SMOKE_ROOT=%TEMP%\yse-gansi-windows-smoke"
if exist "%SMOKE_ROOT%" rmdir /s /q "%SMOKE_ROOT%"
mkdir "%SMOKE_ROOT%" || exit /b 1
cargo build -p gansi || exit /b 1
pushd "%SMOKE_ROOT%" || exit /b 1
"%CARGO_TARGET_DIR%\debug\gansi.exe" create --local "%REPO%" smoke-app || exit /b 1
cd smoke-app || exit /b 1
"%CARGO_TARGET_DIR%\debug\gansi.exe" build || exit /b 1
if not exist "dist\smoke-app\smoke-app.exe" exit /b 1
if not exist "dist\smoke-app\Qt6Core.dll" exit /b 1
if not exist "dist\smoke-app\platforms\qwindows.dll" exit /b 1
set "PATH=C:\Windows\System32;C:\Windows"
"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -ExecutionPolicy Bypass -File "%REPO%\scripts\windows-isolated-smoke.ps1" "%CD%\dist\smoke-app\smoke-app.exe" || exit /b 1
popd
exit /b 0

:example
if "%~2"=="" exit /b 2
if exist "crates\yse-ui\examples\%~2.rs" (
  cargo run -p yse-ui --example %~2
) else (
  cargo run -p yse-%~2
)
exit /b %ERRORLEVEL%

:run
if "%~2"=="" exit /b 2
cargo run -p %~2
exit /b %ERRORLEVEL%
