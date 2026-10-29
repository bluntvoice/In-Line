[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$InstallerPath,
    [Parameter(Mandatory)][string]$BinaryDirectory,
    [string]$ExpectedCommit,
    [string]$SevenZip = '7z.exe'
)
$ErrorActionPreference = 'Stop'
$installer = (Resolve-Path -LiteralPath $InstallerPath).Path
$binaryRoot = (Resolve-Path -LiteralPath $BinaryDirectory).Path
$inspector = (Get-Command $SevenZip -ErrorAction Stop).Source
$repository = Split-Path -Parent $PSScriptRoot
$inspection = Join-Path $repository ('release/installer-inspection-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $inspection | Out-Null
# The guarded NSIS template stages programs under this exact private path.
# Extract only those entries; never execute the installer or its hooks.
$payloadEntries = @('in-line.exe','in-line-mcp.exe','in-line-cli.exe') | ForEach-Object { '$PLUGINSDIR\inline-payload\' + $_ }
& $inspector e -y "-o$inspection" -- $installer @payloadEntries
if ($LASTEXITCODE -ne 0) { throw 'NSIS payload extraction failed' }
foreach ($name in @('in-line.exe','in-line-mcp.exe','in-line-cli.exe')) {
    $payload = Join-Path $inspection $name
    if (!(Test-Path -LiteralPath $payload -PathType Leaf)) { throw "Installer missing $name" }
}
# Tauri patches the GUI bundle marker for NSIS and restores its source file.
# The Node verifier reconstructs that exact payload and compares every byte;
# companions allow no transformation. Original and payload hashes are recorded.
$verify = @(Join-Path $PSScriptRoot 'verify-cli-package.mjs') + @($inspection,'--nsis-source',$binaryRoot,'--output',(Join-Path $repository 'release/installer-payload.json'))
if ($ExpectedCommit) { $verify += @('--expected-commit',$ExpectedCommit) }
& node @verify
if ($LASTEXITCODE -ne 0) { throw 'Extracted payload identity validation failed' }
$hash = (Get-FileHash -LiteralPath $installer -Algorithm SHA256).Hash
[pscustomobject]@{Installer=(Split-Path -Leaf $installer);Sha256=$hash;ExpectedCommit=$ExpectedCommit;PayloadMatched=$true;InstallerExecuted=$false;Installed=$false} |
    ConvertTo-Json | Set-Content -LiteralPath (Join-Path $repository 'release/installer-verification.json') -Encoding utf8
Write-Output "PASS: actual NSIS payload contains three matching binaries; installer not executed. SHA256 $hash"
