# Checks that a second copy of MPelyoffice hands its file to the first and
# leaves, and that only one program stays.
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File mp-single-instance.ps1 -Exe <path to MPelyoffice.exe>
param(
    [Parameter(Mandatory = $true)][string]$Exe,
    [int]$Seconds = 10
)

$ErrorActionPreference = 'Stop'
$failures = 0
function Assert-True([bool]$condition, [string]$message) {
    if ($condition) { Write-Output "  ok   $message" } else { Write-Output "  FAIL $message"; $script:failures += 1 }
}

$sample = Join-Path $env:TEMP 'mpelyoffice-single-instance.txt'
Set-Content -LiteralPath $sample -Value '交给第一个实例' -Encoding UTF8
$log = Join-Path $env:LOCALAPPDATA 'ElyOffice\mpelyoffice\ely.log'
$before = if (Test-Path $log) { (Get-Item $log).Length } else { 0 }

Write-Output 'the first copy starts'
$first = Start-Process -FilePath $Exe -PassThru
Start-Sleep -Seconds 4
Assert-True (-not $first.HasExited) 'the first copy is running'

Write-Output 'a second copy starts with a file'
$started = Get-Date
$second = Start-Process -FilePath $Exe -ArgumentList "`"$sample`"" -PassThru
$second.WaitForExit(30000) | Out-Null
$took = ((Get-Date) - $started).TotalMilliseconds
Assert-True ($second.HasExited) 'the second copy left'
Assert-True ($second.ExitCode -eq 0) "the second copy left with 0 (got $($second.ExitCode))"
Assert-True ($took -lt 15000) "the second copy left in time ($([int]$took) ms)"

Start-Sleep -Seconds 3
$running = @(Get-Process -Name ([System.IO.Path]::GetFileNameWithoutExtension($Exe)) -ErrorAction SilentlyContinue)
Assert-True ($running.Count -eq 1) "one program is left (got $($running.Count))"

Write-Output 'the first copy has the file'
if (Test-Path $log) {
    $added = (Get-Item $log).Length - $before
    $tail = Get-Content $log -Tail 40 -ErrorAction SilentlyContinue | Out-String
    Assert-True ($tail -match 'single-instance' -or $tail -match 'door' -or $tail -match 'handed' -or $tail -match 'opened') "the log says the file was handed over"
    Write-Output "  (log grew by $added bytes)"
} else {
    Write-Output '  (no log to read)'
}

foreach ($process in $running) { Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue }
Remove-Item $sample -ErrorAction SilentlyContinue

Write-Output ''
if ($failures -gt 0) { Write-Output "$failures check(s) failed"; exit 1 }
Write-Output 'all checks passed'
exit 0
