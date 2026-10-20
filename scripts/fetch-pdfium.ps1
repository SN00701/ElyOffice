# Takes the PDFium kernel and puts it where the program looks for it.
#
# PDFium is BSD-3; the licence travels with the file.  The binary comes from
# the NuGet package bblanchon.PDFium.Win32, which is built from Chromium's
# PDFium.  The binding to it is written in this project (crates/ely-engine/
# src/pdfium.rs); the ready-made Rust wrapper is GPL and is not used.
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File fetch-pdfium.ps1
param(
    [string]$Version = '158.0.8111',
    [string]$Target = 'engines\pdfium'
)

$ErrorActionPreference = 'Stop'
$root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$target = Join-Path $root $Target
$package = "bblanchon.pdfium.win32"
$url = "https://api.nuget.org/v3-flatcontainer/$package/$Version/$package.$Version.nupkg"

Write-Output "fetch-pdfium: $url"
$temp = Join-Path $env:TEMP "pdfium-$Version.nupkg"
Invoke-WebRequest -Uri $url -OutFile $temp -UseBasicParsing

New-Item -ItemType Directory -Path $target -Force | Out-Null
Add-Type -AssemblyName System.IO.Compression.FileSystem
$zip = [System.IO.Compression.ZipFile]::OpenRead($temp)
try {
    $entry = $zip.Entries | Where-Object { $_.FullName -eq "runtimes/win-x64/native/pdfium.dll" }
    if (-not $entry) { throw "the package has no win-x64 pdfium.dll" }
    [System.IO.Compression.ZipFileExtensions]::ExtractToFile($entry, (Join-Path $target 'pdfium.dll'), $true)
    Write-Output "fetch-pdfium: engines\pdfium\pdfium.dll ($([math]::Round($entry.Length/1MB,1)) MB)"
} finally {
    $zip.Dispose()
}
Remove-Item $temp -ErrorAction SilentlyContinue

$licence = @"
PDFium
======

The file pdfium.dll is the PDFium of Chromium, built by bblanchon and taken
from the NuGet package bblanchon.PDFium.Win32, version $Version, file
runtimes/win-x64/native/pdfium.dll.

Licence: BSD-3-Clause, the licence of Chromium's PDFium.  Chromium's own
notice must travel with any copy of this file:

  Copyright (c) 2012 The Chromium Authors. All rights reserved.

  Redistribution and use in source and binary forms, with or without
  modification, are permitted provided that the following conditions are
  met:

  * Redistributions of source code must retain the above copyright notice,
    this list of conditions and the following disclaimer.
  * Redistributions in binary form must reproduce the above copyright
    notice, this list of conditions and the following disclaimer in the
    documentation and/or other materials provided with the distribution.
  * Neither the name of Google Inc. nor the names of its contributors may
    be used to endorse or promote products derived from this software
    without specific prior written permission.

  THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS
  IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED
  TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A
  PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT
  OWNER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
  SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
  TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR
  PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF
  LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING
  NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS
  SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

The binding to it in crates/ely-engine/src/pdfium.rs is written for this
project; the ready-made Rust wrapper (pdfium-rs) is GPL and is not used.
"@
[System.IO.File]::WriteAllText((Join-Path $target 'LICENSE.txt'), $licence, (New-Object System.Text.UTF8Encoding $false))
Write-Output 'fetch-pdfium: engines\pdfium\LICENSE.txt'
