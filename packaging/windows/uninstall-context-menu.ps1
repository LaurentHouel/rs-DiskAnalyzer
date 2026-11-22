$ErrorActionPreference = 'Stop'

$menuKeys = @(
    'HKCU:\Software\Classes\Directory\shell\DiskAnalyzerScan'
    'HKCU:\Software\Classes\Drive\shell\DiskAnalyzerScan'
    'HKCU:\Software\Classes\Directory\Background\shell\DiskAnalyzerScan'
)

foreach ($menuKey in $menuKeys) {
    if (Test-Path -LiteralPath $menuKey) {
        Remove-Item -LiteralPath $menuKey -Recurse -Force
    }
}

Write-Host 'File Explorer context-menu action removed for the current user.'
