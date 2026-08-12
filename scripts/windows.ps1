# Windows host entry point. The batch implementation owns environment setup
# because cmd's `pushd` reliably maps WSL UNC checkouts for native tools.
param(
    [Parameter(Mandatory = $true)][string]$Command,
    [string]$Name
)

$ErrorActionPreference = 'Stop'
$entry = Join-Path $PSScriptRoot 'windows.cmd'
& 'C:\Windows\System32\cmd.exe' /d /c "`"$entry`" $Command $Name"
if ($LASTEXITCODE -ne 0) {
    throw "Windows action '$Command' failed with exit code $LASTEXITCODE"
}
