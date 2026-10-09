# Phase 0 文档生成/校验工具；不访问业务数据库，不实现产品功能。
$ErrorActionPreference = 'Stop'
$docRoot = $PSScriptRoot
$source = Join-Path $docRoot 'inputs/01-In-Line-MCP-PRD-144项完整需求.md'
$raw = Get-Content -LiteralPath $source -Raw -Encoding utf8
$matches = [regex]::Matches($raw, '(?m)^\| (MCP-\d{3}) \| ([^|]+) \| ([^|]+) \| ([^|]+) \|\s*$')
$definitions = @{}
foreach ($line in Get-Content -LiteralPath (Join-Path $docRoot 'traceability-map.tsv') -Encoding utf8) {
    if ([string]::IsNullOrWhiteSpace($line)) { continue }
    $parts = $line.Split('|')
    if ($parts.Count -ne 5) { throw "Invalid definition: $line" }
    $id = "MCP-$($parts[0])"
    if ($definitions.ContainsKey($id)) { throw "Duplicate definition: $id" }
    $definitions[$id] = $parts
}
$expected = @(1..144 | ForEach-Object { 'MCP-{0:D3}' -f $_ })
if ($matches.Count -ne 144 -or $definitions.Count -ne 144) { throw 'Must have exactly 144 source rows and definitions' }
$actual = @($matches | ForEach-Object { $_.Groups[1].Value })
if (@(Compare-Object $expected $actual).Count -ne 0 -or @($actual | Select-Object -Unique).Count -ne 144) { throw 'Missing or duplicate requirement IDs' }
$rows = foreach ($match in $matches) {
    $id = $match.Groups[1].Value
    $d = $definitions[$id]
    if ($null -eq $d) { throw "Missing definition: $id" }
    $status = if ($id -eq 'MCP-036') { '待验收（P0排除边界）；运行时待验证' }
              elseif ($id -in @('MCP-052','MCP-054','MCP-089')) { '进行中（P0草案）；跨阶段未完成' }
              else { '未开始（有现状审查；新增验收未执行）' }
    [pscustomobject][ordered]@{
        编号 = $id
        功能说明 = $match.Groups[2].Value.Trim()
        实现阶段 = $match.Groups[3].Value.Trim()
        差距分类 = $d[1]
        现状代码证据 = $d[2] + '：详见architecture.md第1节，均为41383aa源码核查'
        拟实现文件函数 = $d[3] + '（拟建/拟改，尚未实施）'
        测试用例 = "T-$id（计划，未执行）：$($d[4])"
        验收条件 = $match.Groups[4].Value.Trim()
        验收证据 = 'P0：acceptance/PHASE-00.md；现有基线B1-B4不代表本条新增功能通过；未来逐阶段补证据'
        状态 = $status
        用户确认 = '需求由输入01确认；本轮设计/实现验收待用户确认'
        CommitSHA = '未提交；审查基线41383aad406f376ef83bd29284463ee953a0a768'
        阶段Tag = '未创建'
    }
}
$rows | Export-Csv -LiteralPath (Join-Path $docRoot 'requirements-traceability.csv') -NoTypeInformation -Encoding utf8BOM
$lines = [System.Collections.Generic.List[string]]::new()
$lines.Add('# 144 项需求追踪矩阵（P0草案，未实现验收）')
$lines.Add('')
$lines.Add('日期：2026-10-09。源：inputs/01-In-Line-MCP-PRD-144项完整需求.md。主阶段逐条保留原文，未自行调阶段或改变验收口径。')
$lines.Add('')
$lines.Add('完整字段见 [CSV](requirements-traceability.csv)。以下代码证据S1–S9对应 [架构现状表](architecture.md)；拟文件/函数和T-MCP测试均未实施/执行。基线测试只验证现有软件；任何新安全/业务需求均未标记通过。')
$lines.Add('')
$lines.Add('所有行的用户实现验收=待确认，Commit=未提交，阶段Tag=未创建；源码审查基线为41383aad406f376ef83bd29284463ee953a0a768。输入01对需求的确认不等于对本轮实现或设计验收。')
$lines.Add('')
$lines.Add('## P0–P9 映射')
$lines.Add('')
$lines.Add('P0对全部144行做现状/设计映射；主实施阶段及多阶段范围逐行如下，具体阶段出口见 [阶段计划](phase-plan.md)。052、054贯穿P0–P9，089从P0维护至P8结项。')
$lines.Add('')
$lines.Add('| 主实施阶段 | 条数（多阶段取起始阶段计数） |')
$lines.Add('| --- | --- |')
foreach ($phase in 0..9) {
    $count = @($rows | Where-Object { [regex]::Match($_.实现阶段, '^P\d').Value -eq "P$phase" }).Count
    $lines.Add("| P$phase | $count |")
}
$lines.Add('')
$lines.Add('## 逐项矩阵')
$lines.Add('')
$lines.Add('| 编号 | 功能/阶段 | 差距分类/现状证据 | 拟实现文件或函数 | 计划测试及验收条件 | 证据/状态 | 用户确认 | Commit/Tag |')
$lines.Add('| --- | --- | --- | --- | --- | --- | --- | --- |')
foreach ($row in $rows) {
    $lines.Add("| $($row.编号) | $($row.功能说明)；$($row.实现阶段) | $($row.差距分类)；$($definitions[$row.编号][2]) | $($row.拟实现文件函数) | $($row.测试用例)<br>验收：$($row.验收条件) | P0源码审查；B1-B4仅现有基线；$($row.状态) | 原需求已确认；本轮待验收 | 未提交 / 未创建 |")
}
$lines.Add('')
$lines.Add('更新要求：逐阶段填入实际文件/函数、运行测试及证据、全部验收点状态、用户原话、提交SHA及不可移动Tag。当前生成工具只用于初始化P0草案；实施后不得直接重跑覆盖人工补充的验收信息。')
$lines | Set-Content -LiteralPath (Join-Path $docRoot 'requirements-traceability.md') -Encoding utf8
Write-Output 'PASS: 144 unique requirements, original phases/acceptance rules preserved, 144 planned implementation/test mappings, no new feature marked passed.'
$rows | Group-Object { [regex]::Match($_.实现阶段, '^P\d').Value } | Sort-Object Name | Select-Object Name,Count
