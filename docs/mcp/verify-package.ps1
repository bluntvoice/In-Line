param([string]$RepositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path)
$ErrorActionPreference = 'Stop'
$targets = @(
    'src-tauri/target/release/bundle/nsis/In Line_0.5.0_x64-setup.exe',
    'src-tauri/target/release/in-line.exe',
    'src-tauri/target/release/in-line-mcp.exe',
    'src-tauri/binaries/in-line-mcp-x86_64-pc-windows-msvc.exe'
)
$latestSource = (Get-ChildItem -LiteralPath (Join-Path $RepositoryRoot 'src-tauri/src') -Filter '*.rs' -Recurse | Sort-Object LastWriteTimeUtc -Descending | Select-Object -First 1).LastWriteTimeUtc
foreach ($relative in $targets) {
    $file = Get-Item -LiteralPath (Join-Path $RepositoryRoot $relative)
    if ($file.Length -le 0 -or $file.LastWriteTimeUtc -lt $latestSource) { throw "Missing/stale binary: $relative" }
    Write-Output "$relative | bytes=$($file.Length) | SHA256=$((Get-FileHash -LiteralPath $file.FullName).Hash) | modifiedUtc=$($file.LastWriteTimeUtc.ToString('o'))"
}
$sidecar = (Get-FileHash (Join-Path $RepositoryRoot $targets[2])).Hash
$external = (Get-FileHash (Join-Path $RepositoryRoot $targets[3])).Hash
Write-Output "Two existing NSIS sidecar inputs: byteIdentical=$($sidecar -eq $external); record both hashes and test both binaries."
foreach ($relative in $targets[2..3]) {
    & node (Join-Path $PSScriptRoot 'verify-release-stdio.mjs') (Join-Path $RepositoryRoot $relative)
    if ($LASTEXITCODE -ne 0) { throw "Sidecar protocol/authentication gate failed: $relative" }
}
$nsi = Get-Content -LiteralPath (Join-Path $RepositoryRoot 'src-tauri/target/release/nsis/x64/installer.nsi') -Raw
if ($nsi -notmatch '!define VERSION "0.5.0"' -or $nsi -notmatch 'MAINBINARYNAME "in-line"' -or $nsi -notmatch '/oname=in-line-mcp.exe') { throw 'NSIS version or content manifest mismatch' }
$main = [Diagnostics.FileVersionInfo]::GetVersionInfo((Join-Path $RepositoryRoot $targets[1]))
Write-Output "Main PE productVersion=$($main.ProductVersion), fileVersion=$($main.FileVersion)"
if ($main.ProductVersion -notmatch '^0\.5\.0(?:\.0)?$') { throw 'Main PE version mismatch' }
Write-Output 'PASS: installer and main/sidecar files newer than frozen Rust source; NSIS script version 0.5.0 and main/sidecar input paths verified; both sidecar inputs pass actual protocol/anonymous-authentication/rate-limit gates. This does not verify installed/upgrade behavior or extracted installed file hashes. Tauri restores main binary bundle metadata after packaging.'
Write-Output 'Build-input hashes are software artifact provenance, not a fault-data export checksum list.'
