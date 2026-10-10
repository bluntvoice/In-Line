param([string]$RepositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path)
$ErrorActionPreference = 'Stop'
$docRoot = Join-Path $RepositoryRoot 'docs/mcp'
$manifest = Get-Content -LiteralPath (Join-Path $docRoot 'inputs/source-manifest.json') -Raw | ConvertFrom-Json
foreach ($item in $manifest) {
    $copy = Join-Path $RepositoryRoot $item.repositoryPath
    if ((Get-FileHash -LiteralPath $copy -Algorithm SHA256).Hash -ne $item.sha256) { throw "Input copy changed: $($item.name)" }
    if ((Get-FileHash -LiteralPath $item.originalPath -Algorithm SHA256).Hash -ne $item.sha256) { throw "Authority input changed: $($item.name)" }
}
$source = Get-Content -LiteralPath (Join-Path $docRoot 'inputs/01-In-Line-MCP-PRD-144项完整需求.md') -Raw
$matches = [regex]::Matches($source, '(?m)^\| (MCP-\d{3}) \| ([^|]+) \| ([^|]+) \| ([^|]+) \|\s*$')
$rows = @(Import-Csv -LiteralPath (Join-Path $docRoot 'requirements-traceability.csv'))
if ($matches.Count -ne 144 -or $rows.Count -ne 144 -or @($rows.编号 | Select-Object -Unique).Count -ne 144) { throw '144 row gate failed' }
foreach ($match in $matches) {
    $row = $rows | Where-Object 编号 -eq $match.Groups[1].Value
    if ($null -eq $row -or $row.功能说明 -ne $match.Groups[2].Value.Trim() -or $row.实现阶段 -ne $match.Groups[3].Value.Trim() -or $row.验收条件 -ne $match.Groups[4].Value.Trim()) { throw "Original decision altered: $($match.Groups[1].Value)" }
    if (@($row.PSObject.Properties).Count -ne 13) { throw 'Expected 13 traceability fields' }
    foreach ($property in $row.PSObject.Properties) { if ([string]::IsNullOrWhiteSpace($property.Value)) { throw "Empty field: $($row.编号) $($property.Name)" } }
    if ($row.实现阶段 -eq 'P1' -and ($row.状态 -match '^通过' -or $row.阶段Tag -match '^mcp-phase-01-accepted$')) { throw "Unaccepted P1 falsely passed: $($row.编号)" }
}
if (@($rows | Where-Object 实现阶段 -eq 'P1').Count -ne 21) { throw 'P1 primary row count changed' }
$versions = @(
    (Get-Content (Join-Path $RepositoryRoot 'package.json') -Raw | ConvertFrom-Json).version,
    (Get-Content (Join-Path $RepositoryRoot 'package-lock.json') -Raw | ConvertFrom-Json -AsHashtable).version,
    (Get-Content (Join-Path $RepositoryRoot 'src-tauri/tauri.conf.json') -Raw | ConvertFrom-Json).version
)
$cargo = Get-Content (Join-Path $RepositoryRoot 'src-tauri/Cargo.toml') -Raw
$lock = Get-Content (Join-Path $RepositoryRoot 'src-tauri/Cargo.lock') -Raw
$versions += [regex]::Match($cargo, '(?m)^version = "([^"]+)"').Groups[1].Value
$versions += [regex]::Match($lock, '(?s)name = "in-line"\s+version = "([^"]+)"').Groups[1].Value
if (@($versions | Where-Object { $_ -ne '0.5.0' }).Count -ne 0 -or $versions.Count -ne 5) { throw "Version mismatch: $versions" }
$head = & git -C $RepositoryRoot rev-parse HEAD
$tag = & git -C $RepositoryRoot rev-parse 'mcp-phase-00-accepted^{}'
if ($LASTEXITCODE -ne 0 -or $tag -ne 'f2a97b34410f850f3da8c19d4ec6e21c7d63048f') { throw 'P0 accepted tag changed' }
& git -C $RepositoryRoot merge-base --is-ancestor $tag $head
if ($LASTEXITCODE -ne 0) { throw 'Development HEAD does not descend from accepted P0' }
foreach ($file in Get-ChildItem -LiteralPath $docRoot -Filter '*.md' -Recurse | Where-Object { $_.FullName -notmatch '[\\/]inputs[\\/]' }) {
    foreach ($link in [regex]::Matches((Get-Content -LiteralPath $file.FullName -Raw), '\]\(([^)]+)\)')) {
        $path = $link.Groups[1].Value.Split('#')[0]
        if ($path -and $path -notmatch '^(?:https?:|[A-Za-z]:|/|<)') {
            if (-not (Test-Path -LiteralPath (Join-Path $file.DirectoryName $path))) { throw "Broken link: $($file.Name) -> $path" }
        }
    }
}
Write-Output 'PASS: 3 original inputs and copies unchanged; 144 unique original decisions, 13 nonempty fields, 21 P1 mappings, no P1 user acceptance claim; 5 versions 0.5.0; P0 tag fixed and development HEAD descends from P0; local documentation links valid. User authorized a development commit without accepting P1.'
