# Windows host helper for Yse: setup, build, example, check, diag.
#
# Usage (PowerShell 5.1+):
#   powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/windows.ps1 setup
#   powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/windows.ps1 diag
#   powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/windows.ps1 build
#   powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/windows.ps1 example settings
#   powershell.exe -NoProfile -ExecutionPolicy Bypass -File scripts/windows.ps1 check

param(
    [Parameter(Mandatory = $true)][string]$Command,
    [string]$Name
)

$ErrorActionPreference = 'Stop'

# The repo may live on a \\wsl$ share; cmd and native tools cannot start from
# a UNC working directory, so always work from C:\ and address the repo by
# absolute path.
Set-Location 'C:\'

$QtRoot = "$env:ProgramFiles\Qt\6.8.3\msvc2022_64"
$QtBin = "$QtRoot\bin"
$VsDevCmd = 'C:\Program Files\Microsoft Visual Studio\2022\Community\Common7\Tools\VsDevCmd.bat'
$Repo = Split-Path -Parent $PSScriptRoot
$Manifest = "$Repo\Cargo.toml"

# Keep build artifacts on the Windows disk, not on the (slow) WSL share.
$env:CARGO_TARGET_DIR = "$env:USERPROFILE\.cargo\target\gansi"

function Import-VsEnv {
    # Load the MSVC environment into this PowerShell process. cmd cannot
    # start from a UNC working directory, so run the import from C:\.
    $previousErrorAction = $ErrorActionPreference
    $ErrorActionPreference = 'SilentlyContinue'
    $envOutput = cmd /c "cd /d C:\ && call `"$VsDevCmd`" -arch=x64 >nul && set" 2>$null
    $ErrorActionPreference = $previousErrorAction
    foreach ($line in $envOutput) {
        if ($line -match '^([^=]+)=(.*)$') {
            [Environment]::SetEnvironmentVariable($matches[1], $matches[2], 'Process')
        }
    }
    $msvc = Get-ChildItem "$env:ProgramFiles\Microsoft Visual Studio\2022\Community\VC\Tools\MSVC" `
        -Directory | Sort-Object Name -Descending | Select-Object -First 1
    $env:Path = "$($msvc.FullName)\bin\Hostx64\x64;$env:Path"
    if (Test-Path "$QtBin\qmake.exe") {
        $env:Path = "$QtBin;$env:Path"
        $env:QMAKE = "$QtBin\qmake.exe"
    }
    # scoop's fake link.exe shim shadows MSVC's linker; force the real one.
    $env:CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER = "$($msvc.FullName)\bin\Hostx64\x64\link.exe"
}

function Invoke-Cargo([string[]]$ArgsList) {
    Import-VsEnv
    if (-not (Test-Path $Manifest)) {
        throw "manifest not found at $Manifest - is the WSL share reachable from Windows?"
    }
    # cargo fmt cannot resolve a UNC --manifest-path; running from the repo
    # directory works (build artifacts stay in CARGO_TARGET_DIR on C:).
    Set-Location $Repo
    # Cargo writes progress to stderr; PowerShell 5.1 surfaces that as
    # NativeCommandError records, which `$ErrorActionPreference = 'Stop'`
    # would turn into a terminating error and abort every rebuild.
    $previousErrorAction = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    & cargo @ArgsList
    $cargoExit = $LASTEXITCODE
    $ErrorActionPreference = $previousErrorAction
    if ($cargoExit -ne 0) {
        throw "cargo $($ArgsList -join ' ') failed with exit code $cargoExit"
    }
}

switch ($Command) {
    'diag' {
        Write-Host "repo             : $Repo"
        Write-Host "manifest reachable: $(Test-Path $Manifest)"
        if (Test-Path "$QtBin\qmake.exe") {
            Write-Host "qmake installed  : True"
            Write-Host "Qt version       : $(& "$QtBin\qmake.exe" -query QT_VERSION 2>&1)"
        } else {
            Write-Host "qmake installed  : False"
            Write-Host "Qt version       : N/A"
        }
        Import-VsEnv
        Write-Host "cargo            : $(cargo --version 2>&1)"
    }
    'setup' {
        Write-Host "Running gansi setup from Rust toolchain bootstrap..."
        Invoke-Cargo @('run', '-p', 'gansi', '--', 'setup')
    }
    'build' {
        Invoke-Cargo @('build', '--workspace')
    }
    'example' {
        if (-not $Name) {
            throw "example requires a name, e.g. just windows-example settings"
        }
        # Yse examples live in crates/yse-ui/examples; workspace binaries such
        # as the Task Manager are separate packages (yse-taskmgr).
        if (Test-Path "$Repo\crates\yse-ui\examples\$Name.rs") {
            # No QT_QPA_PLATFORM / YSE_SMOKE: show a real window.
            Invoke-Cargo @('run', '-p', 'yse-ui', '--example', $Name)
        } else {
            Invoke-Cargo @('run', '-p', "yse-$Name")
        }
    }
    'run' {
        if (-not $Name) {
            throw "run requires a package name, e.g. just windows-run yse-taskmgr"
        }
        Invoke-Cargo @('run', '-p', $Name)
    }
    'check' {
        Invoke-Cargo @('fmt', '--check')
        Invoke-Cargo @('clippy', '--workspace', '--all-targets', '--', '-D', 'warnings')
        Invoke-Cargo @('test', '--workspace')
    }
    default {
        throw "unknown command: $Command (expected setup|diag|build|example|check)"
    }
}
