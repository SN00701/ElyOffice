# Installs MPelyoffice for the current user and takes it away again.
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File mp-install.ps1 -Setup <path to mpelyoffice-setup.exe> -Payload <folder to install>
param(
    [Parameter(Mandatory = $true)][string]$Setup,
    [Parameter(Mandatory = $true)][string]$Payload,
    [string]$Target = "$env:LOCALAPPDATA\Programs\MPelyoffice"
)

$ErrorActionPreference = 'Stop'
$failures = 0
function Assert-True([bool]$condition, [string]$message) {
    if ($condition) { Write-Output "  ok   $message" } else { Write-Output "  FAIL $message"; $script:failures += 1 }
}

$program = Join-Path $Target 'MPelyoffice.exe'
$uninstallKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\MPelyoffice'

Write-Output 'install'
& $Setup /silent /dir $Target /payload $Payload | Out-Null
Assert-True (Test-Path $program) 'the program is in place'
Assert-True (Test-Path (Join-Path $Target '.ely-installed')) 'the list of installed files is written'
Assert-True (Test-Path 'HKCU:\Software\Classes\MPelyoffice.Document.1') 'the file types are written'
$menu = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs\MPelyoffice\MPelyoffice.lnk'
Assert-True (Test-Path $menu) 'the start menu entry is there'
Assert-True (Test-Path $uninstallKey) 'the program list entry is there'

Write-Output 'the program runs from where it was installed'
$started = Start-Process -FilePath $program -PassThru
Start-Sleep -Seconds 6
Assert-True (-not $started.HasExited) 'the installed program stays open'
Stop-Process -Id $started.Id -Force -ErrorAction SilentlyContinue
# The program holds its own file until it is gone; the uninstall waits for it.
Wait-Process -Id $started.Id -Timeout 20 -ErrorAction SilentlyContinue
Start-Sleep -Seconds 2

Write-Output 'uninstall'
& $Setup /silent /uninstall /dir $Target | Out-Null
Assert-True (-not (Test-Path $program)) 'the program is gone'
Assert-True (-not (Test-Path 'HKCU:\Software\Classes\MPelyoffice.Document.1')) 'the file types are gone'
Assert-True (-not (Test-Path $uninstallKey)) 'the program list entry is gone'
Assert-True (-not (Test-Path $menu)) 'the start menu entry is gone'

Write-Output ''
if ($failures -gt 0) { Write-Output "$failures check(s) failed"; exit 1 }
Write-Output 'all checks passed'
exit 0
