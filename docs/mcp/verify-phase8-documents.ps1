param([string]$RepositoryRoot = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path)
$ErrorActionPreference='Stop'
& (Join-Path $PSScriptRoot 'verify-phase1-documents.ps1') -RepositoryRoot $RepositoryRoot
$taskRows=@(Import-Csv -LiteralPath (Join-Path $PSScriptRoot 'requirements-traceability.csv'))
foreach($taskRow in $taskRows){
    if($taskRow.实现阶段 -ne 'P9' -and $taskRow.拟实现文件函数 -match '拟建/拟改|尚未实施'){throw "Stale implementation mapping: $($taskRow.编号)"}
    if($taskRow.实现阶段 -match 'P[1-8]' -and $taskRow.状态 -match '^通过|^全部实现|^已验收'){throw "False user acceptance: $($taskRow.编号)"}
}
foreach($taskId in @('MCP-044','MCP-093','MCP-100')){
    $taskRow=$taskRows | Where-Object 编号 -eq $taskId
    if(($taskRow.状态+' '+$taskRow.验收证据) -notmatch '未通过|阻断|部分|unauthenticated'){throw "Current-session limitation missing: $taskId"}
}
$taskContract=Get-Content (Join-Path $PSScriptRoot 'contracts/tools-v1.json') -Raw | ConvertFrom-Json
if($taskContract.apiVersion -ne 1 -or $taskContract.tools.Count -ne 16){throw 'Actual tool contract baseline missing'}
foreach($taskTool in @('get_report_summary','list_report_items')){
    if(-not ($taskContract.tools | Where-Object name -eq $taskTool)){throw 'Legacy tool removed without approval'}
}
foreach($taskWorkflow in @('ci','dev-build','release')){
    $taskText=Get-Content (Join-Path $RepositoryRoot ".github/workflows/$taskWorkflow.yml") -Raw
    if($taskText -notmatch 'verify-mcp-contract.mjs' -or $taskText -notmatch 'verify-release-stdio.mjs'){throw "Contract/auth gate missing in $taskWorkflow"}
    $taskLines=$taskText -split '\r?\n'
    for($taskIndex=0;$taskIndex -lt $taskLines.Count;$taskIndex++){
        if($taskLines[$taskIndex] -match '^\s+node (?:scripts/verify-mcp-contract|docs/mcp/verify-release-stdio)\.mjs ' -and $taskLines[$taskIndex+1] -notmatch 'if \(\$LASTEXITCODE -ne 0\) \{ exit \$LASTEXITCODE \}'){
            throw "Native command failure could be hidden in $taskWorkflow"
        }
    }
}
$taskCurrent=Get-Content (Join-Path $PSScriptRoot 'acceptance/evidence/P8-current-session.json') -Raw | ConvertFrom-Json
if($taskCurrent.result.error.code -ne 'unauthenticated' -or $taskCurrent.authorizedAccessVerified -or $taskCurrent.configurationChanged){throw 'Current-session result overstated or configuration changed'}
Write-Output 'PASS: 144 current mappings; P1–P8 not falsely accepted; P9 not entered; strict client limit disclosed; real 16-tool baseline with legacy tools; all three workflows gate actual contracts/authentication.'
