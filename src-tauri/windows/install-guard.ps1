[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$InstallDirectory,
    [Parameter(Mandatory)][ValidateSet('install','uninstall','rollback','finalize','optional-desktop')][string]$Operation,
    [string]$PayloadDirectory,
    [string]$ProductVersion,
    [string]$Publisher,
    [string]$BundleId,
    [string]$DesktopDirectory,
    [string]$ProgramsDirectory,
    [string]$StartMenuFolder,
    [ValidateSet('CurrentUser','LocalMachine')][string]$RegistryHive = 'CurrentUser',
    [int]$CreateStartMenu = 0,
    [int]$CreateDesktop = 0,
    [int]$RemoveShortcuts = 1,
    [int]$EstimatedSize = 0
)
$ErrorActionPreference = 'Stop'
try {
    Add-Type -Path (Join-Path $PSScriptRoot 'InstallGuard.cs'),(Join-Path $PSScriptRoot 'InstallMetadata.cs') -ReferencedAssemblies 'System.dll','System.Core.dll','System.Xml.dll'
    $options = [InLine.Installer.MetadataOptions]::new()
    $options.Version=$ProductVersion; $options.Publisher=$Publisher; $options.BundleId=$BundleId
    if (!$StartMenuFolder.StartsWith('.')) { throw 'Invalid start menu folder transport' }
    $options.Desktop=$DesktopDirectory; $options.Programs=$ProgramsDirectory; $options.StartMenuFolder=$StartMenuFolder.Substring(1); $options.Hive=$RegistryHive
    $options.StartMenu=($CreateStartMenu -eq 1); $options.DesktopShortcut=($CreateDesktop -eq 1); $options.RemoveShortcuts=($RemoveShortcuts -eq 1); $options.EstimatedSize=$EstimatedSize
    if ($Operation -eq 'install') {
        # Match the original NSIS EstimatedSize: packaged payload plus the
        # generated uninstaller, in whole KiB. Read only the private staged file.
        $uninstaller = [IO.FileInfo]::new((Join-Path $PayloadDirectory 'uninstall.exe'))
        $options.EstimatedSize += [int][Math]::Floor($uninstaller.Length / 1024)
    }
    if ($Operation -eq 'optional-desktop') {
        $result = [InLine.Installer.Guard]::OptionalDesktop($InstallDirectory,$options)
    } else {
        $result = [InLine.Installer.Guard]::Run($InstallDirectory, $PayloadDirectory, $Operation, -1, $options)
    }
    $result | ConvertTo-Json -Depth 6 -Compress | Write-Output
    exit $result.Code
} catch {
    Write-Output 'Installer safety check unavailable. No process was terminated; installation stopped.'
    exit 3
}
