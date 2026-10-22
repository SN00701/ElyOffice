# Assembles what MPelyoffice installs: the program, the installer, the
# engines and the notices of everything that is shipped.
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File stage.ps1 -TargetDir F:\mpely-office-dist
param(
    [Parameter(Mandatory = $true)][string]$TargetDir,
    [string]$Profile = 'release',
    [string]$Target = 'x86_64-pc-windows-msvc',
    [string]$BuildDir = 'F:\mpely-target',
    [string]$LokDir = 'F:\mpely-lo\src\instdir'
)

$ErrorActionPreference = 'Stop'
$root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$env:PYTHONUTF8 = '1'
$env:CARGO_TARGET_DIR = $BuildDir
$env:RUSTUP_TOOLCHAIN = '1.98.1-x86_64-pc-windows-msvc'

function Say([string]$message) { Write-Output "stage: $message" }

# A clean folder.
if (Test-Path $TargetDir) { Remove-Item $TargetDir -Recurse -Force }
New-Item -ItemType Directory -Path $TargetDir -Force | Out-Null
$licenses = Join-Path $TargetDir 'Licenses'
New-Item -ItemType Directory -Path $licenses -Force | Out-Null

# The program and the installer.
# The build may sit in the profile's own folder or in one named after the
# target; the folder that has the program is the one.
$build = $null
foreach ($candidate in @((Join-Path $BuildDir "$Profile\$Target"), (Join-Path $BuildDir $Profile), (Join-Path $BuildDir 'debug'))) {
    if (Test-Path (Join-Path $candidate 'mpelyoffice.exe')) { $build = $candidate; break }
}
if (-not $build) { throw 'stage: the program is not built' }
Say "the build is at $build"
foreach ($name in @('mpelyoffice.exe', 'mpelyoffice-setup.exe')) {
    $source = Join-Path $build $name
    if (-not (Test-Path $source)) { throw "stage: $name is not built" }
}
Copy-Item (Join-Path $build 'mpelyoffice.exe') (Join-Path $TargetDir 'MPelyoffice.exe')
Copy-Item (Join-Path $build 'mpelyoffice-setup.exe') (Join-Path $TargetDir 'MPelyoffice-Setup.exe')
Say 'the program and the installer are in place'

# The engines.
$engines = Join-Path $TargetDir 'engines'
New-Item -ItemType Directory -Path $engines -Force | Out-Null
$pdfium = Join-Path $root 'engines\pdfium\pdfium.dll'
if (Test-Path $pdfium) {
    # The program looks for the kernel next to itself.
    Copy-Item $pdfium (Join-Path $TargetDir 'pdfium.dll')
    Copy-Item (Join-Path $root 'engines\pdfium\LICENSE.txt') (Join-Path $licenses 'PDFium-LICENSE.txt')
    Say 'the PDF kernel is in place (BSD-3-Clause, Chromium)'
} else {
    Say 'the PDF kernel is not here; run scripts\fetch-pdfium.ps1'
}
if (Test-Path $LokDir) {
    $program = Join-Path $LokDir 'program'
    # The kernel is only there when the build has got far enough to make it.
    $kernel = Join-Path $program 'mergedlo.dll'
    if (Test-Path $kernel) {
        Copy-Item $program (Join-Path $engines 'lo') -Recurse -Force
        Say 'the document engine is in place'
    } else {
        Say 'the document engine is not built yet (the kernel is not in the program folder)'
    }
} else {
    Say 'the document engine is not built yet'
}

# The notices.
Copy-Item (Join-Path $root 'LICENSE') (Join-Path $licenses 'LICENSE-MPelyoffice.txt')
if (Test-Path (Join-Path $root 'docs\licenses.md')) {
    Copy-Item (Join-Path $root 'docs\licenses.md') (Join-Path $licenses 'licenses.md')
}
Say 'the notices of every crate that is linked in'
$notices = python (Join-Path $root 'packaging\crates.py') mpelyoffice mpelyoffice-setup 2>&1
if ($LASTEXITCODE -ne 0) { throw "stage: the crate notices could not be written" }
$notices | Out-File -Encoding utf8 (Join-Path $licenses 'rust-crates.txt')
Say ("$($notices.Count) lines of notices")

# What is in the package, with a hash of every file.
$version = (Select-String -Path (Join-Path $root 'Cargo.toml') -Pattern '^version' | Select-Object -First 1).Line
$version = ($version -split '"')[1]
$files = @()
foreach ($file in Get-ChildItem $TargetDir -Recurse -File | Sort-Object FullName) {
    $files += [ordered]@{
        path = $file.FullName.Substring($TargetDir.Length + 1).Replace('\', '/')
        bytes = $file.Length
        sha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $file.FullName).Hash.ToLower()
    }
}
$manifest = [ordered]@{
    product = 'MPelyoffice'
    version = $version
    platform = 'windows-x86_64'
    built = (Get-Date).ToUniversalTime().ToString('o')
    engines = [ordered]@{
        pdfium = '158.0.8111 (Chromium, BSD-3-Clause)'
        lok = if (Test-Path $LokDir) { 'LibreOffice 26.2.6.3 with the LOK patches (MPL-2.0)' } else { 'not in this package' }
    }
    files = $files
}
$manifest | ConvertTo-Json -Depth 5 | Out-File -Encoding utf8 (Join-Path $TargetDir 'MPelyoffice.json')
Say ("the manifest names $($files.Count) files")

$size = [math]::Round(((Get-ChildItem $TargetDir -Recurse -File | Measure-Object Length -Sum).Sum / 1MB), 1)
Say "the package is at $TargetDir ($size MB)"
Get-ChildItem $TargetDir | ForEach-Object { Say "  $($_.Name)" }
