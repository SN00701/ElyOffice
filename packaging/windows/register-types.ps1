# Writes MPelyoffice's file associations for the current user.
#
# The rules (docs/mp/05-Windows集成规范.md §3.2):
#   - the default program is never taken; only "Open with" is offered
#   - `UserChoice` is never written
#   - `HKCR\.<ext>` keeps its default value, also at uninstall
#   - `SHChangeNotify` tells the shell that something changed
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File register-types.ps1 -Exe <path to MPelyoffice.exe>
#   powershell -ExecutionPolicy Bypass -File register-types.ps1 -Exe <path> -Uninstall
param(
    [Parameter(Mandatory = $true)][string]$Exe,
    [switch]$Uninstall
)

$ErrorActionPreference = 'Stop'

$classes = 'HKCU:\Software\Classes'
$app = 'MPelyoffice'

# The format families and the extensions each answers to.
$families = [ordered]@{
    'Document'    = @('.docx', '.odt', '.doc', '.rtf')
    'Spreadsheet' = @('.xlsx', '.ods', '.xls', '.csv', '.tsv')
    'Presentation' = @('.pptx', '.odp', '.ppt')
    'Pdf'         = @('.pdf')
    'Markdown'    = @('.md', '.markdown')
    'Text'        = @('.txt')
}

# The names shown for each family.
$names = @{
    'Document'     = 'MPelyoffice 文档'
    'Spreadsheet'  = 'MPelyoffice 表格'
    'Presentation' = 'MPelyoffice 演示'
    'Pdf'          = 'MPelyoffice PDF'
    'Markdown'     = 'MPelyoffice Markdown'
    'Text'         = 'MPelyoffice 文本'
}

function Get-Native {
    if (-not ('Win32.Shell' -as [type])) {
        Add-Type -Namespace Win32 -Name Shell -MemberDefinition @'
[DllImport("shell32.dll", CharSet = CharSet.Auto, SetLastError = false)]
public static extern void SHChangeNotify(uint wEventId, uint uFlags, IntPtr dwItem1, IntPtr dwItem2);
'@
    }
}

function Get-ProgId([string]$family) {
    return "$app.$family.1"
}

if ($Uninstall) {
    foreach ($family in $families.Keys) {
        $progid = Get-ProgId $family
        Remove-Item -Path "$classes\$progid" -Recurse -Force -ErrorAction SilentlyContinue
        foreach ($ext in $families[$family]) {
            $key = "$classes\$ext\OpenWithProgids"
            if (Test-Path $key) {
                $values = (Get-Item $key).GetValueNames()
                if ($values -contains $progid) {
                    Remove-ItemProperty -Path $key -Name $progid -Force
                }
            }
        }
    }
    Remove-Item -Path "$classes\$app.Capabilities" -Recurse -Force -ErrorAction SilentlyContinue
    Remove-Item -Path "$classes\Applications\$app.exe" -Recurse -Force -ErrorAction SilentlyContinue
    $registered = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\RegisteredApplications'
    if (Test-Path $registered) {
        $values = (Get-Item $registered).GetValueNames()
        if ($values -contains $app) {
            Remove-ItemProperty -Path $registered -Name $app -Force
        }
    }
    Get-Native
    [Win32.Shell]::SHChangeNotify(0x08000000, 0, [IntPtr]::Zero, [IntPtr]::Zero)
    Write-Output "unregistered: $app"
    exit 0
}

$exePath = (Resolve-Path -LiteralPath $Exe -ErrorAction SilentlyContinue)
if (-not $exePath) { $exePath = $Exe }

foreach ($family in $families.Keys) {
    $progid = Get-ProgId $family
    $key = "$classes\$progid"
    New-Item -Path $key -Force | Out-Null
    New-ItemProperty -Path $key -Name '(Default)' -Value $names[$family] -PropertyType String -Force | Out-Null
    New-ItemProperty -Path $key -Name 'FriendlyTypeName' -Value $names[$family] -PropertyType String -Force | Out-Null
    New-ItemProperty -Path $key -Name 'DefaultIcon' -Value "$exePath,0" -PropertyType String -Force | Out-Null
    $command = "$key\shell\open\command"
    New-Item -Path $command -Force | Out-Null
    New-ItemProperty -Path $command -Name '(Default)' -Value ('"' + $exePath + '" "%1"') -PropertyType String -Force | Out-Null
    foreach ($ext in $families[$family]) {
        $openWith = "$classes\$ext\OpenWithProgids"
        New-Item -Path $openWith -Force | Out-Null
        New-ItemProperty -Path $openWith -Name $progid -Value '' -PropertyType String -Force | Out-Null
    }
}

# The program itself.
$applications = "$classes\Applications\$app.exe"
New-Item -Path $applications -Force | Out-Null
New-ItemProperty -Path $applications -Name 'FriendlyAppName' -Value 'MPelyoffice' -PropertyType String -Force | Out-Null
$supported = @()
foreach ($family in $families.Keys) { $supported += $families[$family] }
New-ItemProperty -Path $applications -Name 'SupportedTypes' -Value $supported -PropertyType MultiString -Force | Out-Null

# What the "Default apps" page reads.
$capabilities = "$classes\$app.Capabilities"
New-Item -Path $capabilities -Force | Out-Null
New-ItemProperty -Path $capabilities -Name 'ApplicationName' -Value 'MPelyoffice' -PropertyType String -Force | Out-Null
New-ItemProperty -Path $capabilities -Name 'ApplicationDescription' -Value 'MPelyoffice 统一办公套件' -PropertyType String -Force | Out-Null
$associations = "$capabilities\FileAssociations"
New-Item -Path $associations -Force | Out-Null
foreach ($family in $families.Keys) {
    foreach ($ext in $families[$family]) {
        New-ItemProperty -Path $associations -Name $ext -Value (Get-ProgId $family) -PropertyType String -Force | Out-Null
    }
}
$registered = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\RegisteredApplications'
New-Item -Path $registered -Force | Out-Null
New-ItemProperty -Path $registered -Name $app -Value "Software\Classes\$app.Capabilities" -PropertyType String -Force | Out-Null

Get-Native
[Win32.Shell]::SHChangeNotify(0x08000000, 0, [IntPtr]::Zero, [IntPtr]::Zero)

Write-Output "registered: $app -> $exePath"
foreach ($family in $families.Keys) {
    Write-Output ("  {0} : {1}" -f (Get-ProgId $family), ($families[$family] -join ' '))
}
