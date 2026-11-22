$ErrorActionPreference = 'Stop'

$exe = Join-Path $PSScriptRoot 'rs-disk-analyzer.exe'
if (-not (Test-Path -LiteralPath $exe -PathType Leaf)) {
    throw "Could not find rs-disk-analyzer.exe beside this script: $exe"
}

$registrations = @(
    @{ Path = 'HKCU:\Software\Classes\Directory\shell\DiskAnalyzerScan'; Argument = '%1' }
    @{ Path = 'HKCU:\Software\Classes\Drive\shell\DiskAnalyzerScan'; Argument = '%1' }
    @{ Path = 'HKCU:\Software\Classes\Directory\Background\shell\DiskAnalyzerScan'; Argument = '%V' }
)
foreach ($registration in $registrations) {
    $menuKey = $registration.Path
    New-Item -Path $menuKey -Force | Out-Null
    Set-Item -Path $menuKey -Value 'Scan with Disk Analyzer'
    New-ItemProperty -Path $menuKey -Name 'Icon' -Value "`"$exe`"" -PropertyType String -Force | Out-Null

    $commandKey = Join-Path $menuKey 'command'
    New-Item -Path $commandKey -Force | Out-Null
    Set-Item -Path $commandKey -Value ('"{0}" --scan "{1}"' -f $exe, $registration.Argument)
}

Write-Host 'File Explorer context-menu action installed for the current user.'
