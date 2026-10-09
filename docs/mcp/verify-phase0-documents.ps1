# 只校验Phase 0草案/输入/证据，不访问业务库或改产品配置。
$ErrorActionPreference = 'Stop'
$docRoot = $PSScriptRoot
$repoRoot = (Resolve-Path -LiteralPath (Join-Path $docRoot '../..')).Path
$records = [System.Collections.Generic.List[string]]::new()
$manifest = foreach ($name in @('01-In-Line-MCP-PRD-144项完整需求.md','02-In-Line-MCP-分阶段开发与验收方案.md','03-In-Line-MCP-Codex-总执行提示词.md')) {
    $original = Join-Path 'D:/Downloads' $name
    $copy = Join-Path $docRoot "inputs/$name"
    $sourceHash = (Get-FileHash -LiteralPath $original -Algorithm SHA256).Hash
    $copyHash = (Get-FileHash -LiteralPath $copy -Algorithm SHA256).Hash
    if ($sourceHash -ne $copyHash) { throw "Input copy mismatch: $name" }
    [pscustomobject][ordered]@{ name=$name; originalPath=$original; repositoryPath="docs/mcp/inputs/$name"; bytes=(Get-Item -LiteralPath $copy).Length; sha256=$copyHash; checkedOn='2026-10-09'; identical=$true }
}
$manifest | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $docRoot 'inputs/source-manifest.json') -Encoding utf8
$records.Add('PASS: all three source copies byte-identical to D:/Downloads originals; SHA256/length manifest saved (source provenance, not a fault export hash list).')
$sourceText = Get-Content -LiteralPath (Join-Path $docRoot 'inputs/01-In-Line-MCP-PRD-144项完整需求.md') -Raw -Encoding utf8
$sourceRows = [regex]::Matches($sourceText, '(?m)^\| (MCP-\d{3}) \| ([^|]+) \| ([^|]+) \| ([^|]+) \|\s*$')
$rows = @(Import-Csv -LiteralPath (Join-Path $docRoot 'requirements-traceability.csv') -Encoding utf8)
if ($sourceRows.Count -ne 144 -or $rows.Count -ne 144) { throw '144-row coverage required' }
foreach ($sourceRow in $sourceRows) {
    $id = $sourceRow.Groups[1].Value
    $target = @($rows | Where-Object 编号 -eq $id)
    if ($target.Count -ne 1) { throw "Missing/duplicate row $id" }
    if ($target[0].功能说明 -ne $sourceRow.Groups[2].Value.Trim() -or $target[0].实现阶段 -ne $sourceRow.Groups[3].Value.Trim() -or $target[0].验收条件 -ne $sourceRow.Groups[4].Value.Trim()) { throw "Original requirement altered: $id" }
    foreach ($value in $target[0].PSObject.Properties.Value) { if ([string]::IsNullOrWhiteSpace($value)) { throw "Incomplete fields for $id" } }
    if ($target[0].状态 -match '^(通过|已实现|已完成)$' -or $target[0].阶段Tag -ne '未创建') { throw "Unverified feature or Tag claim: $id" }
}
$records.Add('PASS: MCP-001..144 unique; original names/phases/acceptance conditions preserved; all 13 fields filled; no new feature marked passed or accepted.')
$records | Set-Content -LiteralPath (Join-Path $docRoot 'acceptance/evidence/P0-document-checks.txt') -Encoding utf8
$linkCount = 0
foreach ($file in Get-ChildItem -LiteralPath $docRoot -Recurse -Filter '*.md' -File) {
    foreach ($link in [regex]::Matches((Get-Content -LiteralPath $file.FullName -Raw -Encoding utf8), '\[[^\]]+\]\(([^)]+)\)')) {
        $target = $link.Groups[1].Value
        if ($target -match '^(https?://|#|mailto:)') { continue }
        $target = $target.Split('#')[0]
        $resolved = [System.IO.Path]::GetFullPath((Join-Path $file.DirectoryName $target))
        if (-not (Test-Path -LiteralPath $resolved)) { throw "Broken local link $($file.FullName): $target" }
        $linkCount++
    }
}
$records.Add("PASS: $linkCount local Markdown links resolve.")
$head = (& git -C $repoRoot rev-parse HEAD).Trim()
if ($head -ne '41383aad406f376ef83bd29284463ee953a0a768') { throw 'Git baseline drift: refresh P0 review' }
$trackedChanges = @(& git -c core.quotePath=false -C $repoRoot diff --name-only HEAD)
if ($trackedChanges.Count -ne 0) { throw "Tracked product or prior files changed: $trackedChanges" }
$untracked = @(& git -c core.quotePath=false -C $repoRoot ls-files --others --exclude-standard)
foreach ($file in $untracked) { if (-not $file.StartsWith('docs/mcp/')) { throw "Out-of-scope file: $file" } }
$versions = @(
    (Get-Content -LiteralPath (Join-Path $repoRoot 'package.json') -Raw | ConvertFrom-Json).version,
    (Get-Content -LiteralPath (Join-Path $repoRoot 'package-lock.json') -Raw | ConvertFrom-Json -AsHashtable).version,
    ([regex]::Match((Get-Content -LiteralPath (Join-Path $repoRoot 'src-tauri/Cargo.toml') -Raw), '(?m)^version = "([^"]+)"')).Groups[1].Value,
    ([regex]::Match((Get-Content -LiteralPath (Join-Path $repoRoot 'src-tauri/Cargo.lock') -Raw), '(?ms)^\[\[package\]\]\r?\nname = "in-line"\r?\nversion = "([^"]+)"')).Groups[1].Value,
    (Get-Content -LiteralPath (Join-Path $repoRoot 'src-tauri/tauri.conf.json') -Raw | ConvertFrom-Json).version
)
if (@($versions | Select-Object -Unique).Count -ne 1 -or $versions[0] -ne '0.5.0') { throw "Version drift: $versions" }
$records.Add('PASS: HEAD unchanged; no tracked changes; untracked files only docs/mcp; five version sources unchanged at 0.5.0; no Commit/Push/Tag/Release performed by this task.')
$records.Add('BASELINE: B1 frontend 105 passed; B2 library59+MCP2+stdio1=62 passed; B3 build exit0; B4 fmt exit0. These are existing tests, not new MCP requirements.')
$records.Add('NOT RUN: new auth/IPC/scope/write/batch/jobs/changes/cache/diagnostics/export tests; NSIS/GUI/install/upgrade/multi-account/second-device; remote Actions/CI.')
$records | Set-Content -LiteralPath (Join-Path $docRoot 'acceptance/evidence/P0-document-checks.txt') -Encoding utf8
$records | ForEach-Object { Write-Output $_ }
