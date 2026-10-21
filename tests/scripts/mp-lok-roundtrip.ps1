# Checks that the document engine reads a real document.
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File mp-lok-roundtrip.ps1
#   powershell -ExecutionPolicy Bypass -File mp-lok-roundtrip.ps1 -LokDir F:\mpely-lo\src\instdir
param(
    [string]$LokDir = "$env:ELLY_LOK_DIR",
    [string]$Target = "F:\mpely-lo\src\instdir"
)

$ErrorActionPreference = 'Stop'
$root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path

# The kernel, where it is.
if (-not $LokDir -or -not (Test-Path $LokDir)) { $LokDir = $Target }
if (-not (Test-Path $LokDir)) {
    Write-Output "the kernel is not built yet; it will be at $Target"
    Write-Output 'the LibreOffice build is running; see docs/mp/07 section 4.2'
    exit 2
}
Write-Output "the kernel is at $LokDir"
$env:ELY_LOK_DIR = $LokDir

# A small document to read.
$sample = Join-Path $env:TEMP 'mpelyoffice-sample.docx'
$work = Join-Path $env:TEMP 'mpelyoffice-sample-src'
if (Test-Path $work) { Remove-Item $work -Recurse -Force }
New-Item -ItemType Directory -Path (Join-Path $work '_rels') -Force | Out-Null
New-Item -ItemType Directory -Path (Join-Path $work 'word') -Force | Out-Null
[System.IO.File]::WriteAllText((Join-Path $work '[Content_Types].xml'), @'
<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
<Default Extension="xml" ContentType="application/xml"/>
<Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>
'@)
[System.IO.File]::WriteAllText((Join-Path $work '_rels\.rels'), @'
<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>
'@)
[System.IO.File]::WriteAllText((Join-Path $work 'word\document.xml'), @'
<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
<w:body><w:p><w:r><w:t>MPelyoffice round trip</w:t></w:r></w:p></w:body>
</w:document>
'@)
Add-Type -AssemblyName System.IO.Compression.FileSystem
if (Test-Path $sample) { Remove-Item $sample -Force }
[System.IO.Compression.ZipFile]::CreateFromDirectory($work, $sample)
Write-Output "the sample is at $sample"

$env:ELY_TEST_DOCX = $sample
$env:CARGO_TARGET_DIR = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { 'F:\mpely-target' }
$env:RUSTUP_TOOLCHAIN = '1.98.1-x86_64-pc-windows-msvc'
Push-Location $root
cargo test -p ely-engine lok -- --nocapture 2>&1 | Tee-Object -FilePath (Join-Path $env:TEMP 'mpelyoffice-lok-test.log')
$code = $LASTEXITCODE
Pop-Location
Remove-Item $sample -Force -ErrorAction SilentlyContinue
Remove-Item $work -Recurse -Force -ErrorAction SilentlyContinue
if ($code -ne 0) { Write-Output "the test failed (exit $code)"; exit $code }
Write-Output 'the document engine reads the document'
exit 0
