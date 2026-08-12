param([Parameter(Mandatory = $true)][string]$Executable)

$env:Path = 'C:\Windows\System32;C:\Windows'
$env:QT_QPA_PLATFORM = 'offscreen'
$process = Start-Process -FilePath $Executable -PassThru
Start-Sleep -Seconds 2
if ($process.HasExited) {
    exit $process.ExitCode
}
$process.CloseMainWindow() | Out-Null
if (-not $process.WaitForExit(3000)) {
    $process.Kill()
    $process.WaitForExit()
}
exit 0
