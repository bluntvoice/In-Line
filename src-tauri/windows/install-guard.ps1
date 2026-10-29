[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$InstallDirectory,
    [Parameter(Mandatory)][ValidateSet('install','uninstall','rollback','finalize')][string]$Operation,
    [string]$PayloadDirectory
)
$ErrorActionPreference = 'Stop'
try {
    Add-Type -Path (Join-Path $PSScriptRoot 'InstallGuard.cs') -ReferencedAssemblies 'System.dll','System.Core.dll','System.Xml.dll'
    $result = [InLine.Installer.Guard]::Run($InstallDirectory, $PayloadDirectory, $Operation, -1)
    $result | ConvertTo-Json -Depth 6 -Compress | Write-Output
    exit $result.Code
} catch {
    Write-Output 'Installer safety check unavailable. No process was terminated; installation stopped.'
    exit 3
}
