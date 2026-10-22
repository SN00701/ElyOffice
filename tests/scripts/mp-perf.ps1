# Measures how the program starts, how much it holds, and what it does when
# it is left alone.
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File mp-perf.ps1
#   powershell -ExecutionPolicy Bypass -File mp-perf.ps1 -Runs 20 -Program F:\mpely-target\release\mpelyoffice.exe
param(
    [string]$Program = "F:\mpely-target\release\mpelyoffice.exe",
    [int]$Runs = 10,
    [string]$Pdf = "",
    [int]$IdleSeconds = 10
)

$ErrorActionPreference = 'Stop'

if (-not (Test-Path $Program)) {
    Write-Output "the program is not built at $Program; the development build is used"
    $Program = "F:\mpely-target\debug\mpelyoffice.exe"
}
if (-not (Test-Path $Program)) {
    Write-Output 'the program is not built at all'
    exit 2
}
if (-not $Pdf) {
    $sample = Join-Path $env:TEMP 'mpelyoffice-perf.pdf'
    if (-not (Test-Path $sample)) {
        # A small document of one's own, so the measure does not depend on the
        # files of the machine.
        Add-Type -AssemblyName System.Drawing
        $bitmap = New-Object System.Drawing.Bitmap 1240, 1754
        $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
        $graphics.Clear([System.Drawing.Color]::White)
        $graphics.DrawString('MPelyoffice', (New-Object System.Drawing.Font('Arial', 48)), [System.Drawing.Brushes]::Black, 80, 80)
        $graphics.DrawString('a page for the measure', (New-Object System.Drawing.Font('Arial', 24)), [System.Drawing.Brushes]::Black, 80, 200)
        $graphics.Dispose()
        $bitmap.Save($sample, [System.Drawing.Imaging.ImageFormat]::Png)
        $bitmap.Dispose()
    }
    $Pdf = $sample
}

function Get-WorkingSet([int]$processId) {
    $process = Get-Process -Id $processId -ErrorAction SilentlyContinue
    if ($process) { [math]::Round($process.WorkingSet64 / 1MB, 1) } else { 0 }
}

function Get-CpuSeconds([int]$processId) {
    $process = Get-Process -Id $processId -ErrorAction SilentlyContinue
    if ($process) { [math]::Round($process.TotalProcessorTime.TotalSeconds, 2) } else { 0 }
}

Write-Output "the program: $Program"
Write-Output "the runs: $Runs"

# The time from the start of the program to the window being there.
$starts = @()
$idle = 0
for ($run = 0; $run -lt $Runs; $run++) {
    $watch = [System.Diagnostics.Stopwatch]::StartNew()
    $process = Start-Process -FilePath $Program -PassThru
    $window = [IntPtr]::Zero
    while ($watch.ElapsedMilliseconds -lt 30000) {
        $process.Refresh()
        if ($process.MainWindowHandle -ne [IntPtr]::Zero) { $window = $process.MainWindowHandle; break }
        if ($process.HasExited) { break }
        Start-Sleep -Milliseconds 10
    }
    $watch.Stop()
    if ($window -eq [IntPtr]::Zero) {
        Write-Output "  the window did not come up on run $($run + 1)"
        if (-not $process.HasExited) { Stop-Process -Id $process.Id -Force }
        continue
    }
    $starts += $watch.Elapsed.TotalMilliseconds
    # The memory of the program when it is doing nothing, once it has settled.
    Start-Sleep -Seconds 3
    $idle = Get-WorkingSet $process.Id
    if (-not $process.HasExited) { Stop-Process -Id $process.Id -Force }
    Start-Sleep -Milliseconds 300
}
Write-Output "  the memory of the window alone: $idle MB"

if ($starts.Count -gt 0) {
    $sorted = $starts | Sort-Object
    $middle = $sorted[[int]($sorted.Count / 2)]
    $slow = $sorted[-1]
    Write-Output ("the time to the window: middle {0:N0} ms, slowest {1:N0} ms, of {2} runs" -f $middle, $slow, $starts.Count)
} else {
    Write-Output 'the window never came up'
    exit 1
}

# The program with a document of its own, and what it holds then.
$watch = [System.Diagnostics.Stopwatch]::StartNew()
$process = Start-Process -FilePath $Program -ArgumentList "`"$Pdf`"" -PassThru
while ($watch.ElapsedMilliseconds -lt 30000) {
    $process.Refresh()
    if ($process.MainWindowHandle -ne [IntPtr]::Zero) { break }
    if ($process.HasExited) { break }
    Start-Sleep -Milliseconds 10
}
Start-Sleep -Seconds 4
$withDocument = Get-WorkingSet $process.Id
Write-Output "  the memory with a document: $withDocument MB"

# What it does when it is left alone.
$before = Get-CpuSeconds $process.Id
Start-Sleep -Seconds $IdleSeconds
$after = Get-CpuSeconds $process.Id
$idleCpu = [math]::Round($after - $before, 2)
Write-Output "  the work it does when it is left alone: $idleCpu s in $IdleSeconds s"
if (-not $process.HasExited) { Stop-Process -Id $process.Id -Force }

Write-Output ''
Write-Output 'the numbers:'
Write-Output "  the time to the window (middle): $([math]::Round($middle, 0)) ms"
Write-Output "  the memory of the window alone: $idle MB"
Write-Output "  the memory with a document: $withDocument MB"
Write-Output "  the work when left alone: $idleCpu s in $IdleSeconds s"
