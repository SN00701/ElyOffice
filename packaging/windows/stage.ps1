# Assembles what the installer puts on a computer.
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File stage.ps1 -TargetDir F:\mpely-office-dist
#
# It takes the built program, the installer, and whatever the engines are in
# `engines/`, and lays them out as one folder that the installer can copy.
param(
    [Parameter(Mandatory = $true)][string]$TargetDir,
    [string]$Profile = 'release',
    [string]$Target = 'x86_64-pc-windows-msvc'
)

$ErrorActionPreference = 'Stop'
$root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$target = $TargetDir
$build = Join-Path $root "target\$Profile\$Target"

if (-not (Test-Path $build)) {
    $build = 'F:\mpely-target\debug'
    Write-Output "stage: the build folder is not where it was expected; using $build"
}

Write-Output "stage: clearing $target"
if (Test-Path $target) { Remove-Item $target -Recurse -Force }
New-Item -ItemType Directory -Path $target -Force | Out-Null

$program = Join-Path $build 'mpelyoffice.exe'
$setup = Join-Path $build 'mpelyoffice-setup.exe'
if (-not (Test-Path $program)) { throw "stage: $program is not built" }
if (-not (Test-Path $setup)) { throw "stage: $setup is not built" }
Copy-Item $program $target
Copy-Item $setup $target

# The engines that are built, not their sources.  The source trees are big
# and are not part of what a computer needs.
$engines = Join-Path $root 'engines'
if (Test-Path $engines) {
    Write-Output 'stage: the engines'
    $built = Get-ChildItem $engines -Directory | Where-Object {
        $_.Name -notlike '*-src' -and $_.Name -notlike '.*'
    }
    foreach ($engine in $built) {
        Copy-Item $engine.FullName (Join-Path $target 'engines') -Recurse -Force
        Write-Output "  $($engine.Name)"
    }
}

# The licenses of everything that is shipped.
$licenses = Join-Path $target 'Licenses'
New-Item -ItemType Directory -Path $licenses -Force | Out-Null
Copy-Item (Join-Path $root 'LICENSE') $licenses -ErrorAction SilentlyContinue
Copy-Item (Join-Path $root 'docs\licenses.md') $licenses -ErrorAction SilentlyContinue

$size = [math]::Round(((Get-ChildItem $target -Recurse -File | Measure-Object Length -Sum).Sum / 1MB), 1)
Write-Output "stage: $target ($size MB)"
Get-ChildItem $target | ForEach-Object { Write-Output "  $($_.Name)" }
