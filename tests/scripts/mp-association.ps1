# Checks that MPelyoffice's file associations are written and that the
# default program is left alone.
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File mp-association.ps1 -Exe <path to MPelyoffice.exe>
#
# It registers, looks, uninstalls, and looks again; it fails loudly on the
# first thing that is wrong.
param(
    [Parameter(Mandatory = $true)][string]$Exe
)

$ErrorActionPreference = 'Stop'
$root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$register = Join-Path $root 'packaging\windows\register-types.ps1'

$failures = 0
function Assert-True([bool]$condition, [string]$message) {
    if ($condition) {
        Write-Output "  ok   $message"
    } else {
        Write-Output "  FAIL $message"
        $script:failures += 1
    }
}

$userChoice = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts'
$watched = @('.docx', '.xlsx', '.pptx', '.pdf', '.md', '.txt')

function Get-UserChoice([string]$ext) {
    $key = "$userChoice$ext\UserChoice"
    if (-not (Test-Path $key)) { return '<none>' }
    $progid = (Get-ItemProperty -Path $key -Name 'ProgId' -ErrorAction SilentlyContinue).ProgId
    if (-not $progid) { return '<none>' }
    return $progid
}

Write-Output 'before: the default program'
$before = @{}
foreach ($ext in $watched) { $before[$ext] = Get-UserChoice $ext }
foreach ($ext in $watched) { Write-Output "  $ext -> $($before[$ext])" }

Write-Output 'install: registering'
& $register -Exe $Exe | Out-Null

Write-Output 'after: the associations are written'
foreach ($family in @('Document', 'Spreadsheet', 'Presentation', 'Pdf', 'Markdown', 'Text')) {
    $progid = "MPelyoffice.$family.1"
    $key = "HKCU:\Software\Classes\$progid"
    Assert-True (Test-Path $key) "the ProgID $progid exists"
    Assert-True (Test-Path "$key\shell\open\command") "the ProgID $progid has an open command"
}
foreach ($ext in $watched) {
    $key = "HKCU:\Software\Classes\$ext\OpenWithProgids"
    $names = if (Test-Path $key) { (Get-Item $key).GetValueNames() } else { @() }
    Assert-True (($names | Where-Object { $_ -like 'MPelyoffice.*' }).Count -gt 0) "$ext is offered in the open-with list"
}
$registered = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\RegisteredApplications'
$appNames = (Get-Item $registered).GetValueNames()
Assert-True ($appNames -contains 'MPelyoffice') 'the program is in the default-apps page'

Write-Output 'the default program is not taken'
foreach ($ext in $watched) {
    Assert-True ((Get-UserChoice $ext) -eq $before[$ext]) "$ext still opens with $($before[$ext])"
}

Write-Output 'uninstall: removing'
& $register -Exe $Exe -Uninstall | Out-Null
foreach ($family in @('Document', 'Spreadsheet', 'Presentation', 'Pdf', 'Markdown', 'Text')) {
    $progid = "MPelyoffice.$family.1"
    Assert-True (-not (Test-Path "HKCU:\Software\Classes\$progid")) "the ProgID $progid is gone"
}
foreach ($ext in $watched) {
    $key = "HKCU:\Software\Classes\$ext\OpenWithProgids"
    $names = if (Test-Path $key) { (Get-Item $key).GetValueNames() } else { @() }
    Assert-True (($names | Where-Object { $_ -like 'MPelyoffice.*' }).Count -eq 0) "$ext no longer names us"
    Assert-True ((Get-UserChoice $ext) -eq $before[$ext]) "$ext still opens with $($before[$ext])"
}

Write-Output ''
if ($failures -gt 0) {
    Write-Output "$failures check(s) failed"
    exit 1
}
Write-Output 'all checks passed'
exit 0
