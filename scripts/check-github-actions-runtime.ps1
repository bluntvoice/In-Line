[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"
$OutputEncoding = [System.Text.UTF8Encoding]::new()
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new()

$repositoryRoot = Split-Path -Parent $PSScriptRoot
$workflowDirectory = Join-Path $repositoryRoot ".github\workflows"
$workflowFiles = @(Get-ChildItem -LiteralPath $workflowDirectory -File | Where-Object {
    $_.Extension -in ".yml", ".yaml"
})

if ($workflowFiles.Count -eq 0) {
    throw "未找到 GitHub Actions 工作流。"
}

# 这些主版本均以 Node.js 24 运行。升级门槛与 Dependabot 共同防止工作流
# 长期停留在 GitHub 已弃用的 Node.js 20 Action 运行时。
$minimumMajors = @{
    "actions/checkout"             = 6
    "actions/setup-node"           = 6
    "actions/upload-artifact"      = 6
    "softprops/action-gh-release"  = 3
    "Swatinem/rust-cache"          = 2
}

# 复合 Action 不使用独立的 Node.js Action 运行时，但仍需显式列入清单。
$approvedCompositeActions = @{
    "dtolnay/rust-toolchain" = @("stable")
}

$violations = [System.Collections.Generic.List[string]]::new()
$checkedActions = [System.Collections.Generic.List[string]]::new()

foreach ($workflowFile in $workflowFiles) {
    $content = Get-Content -Raw -Encoding UTF8 -LiteralPath $workflowFile.FullName
    $usesMatches = [regex]::Matches($content, '(?m)^\s*uses:\s*(?<action>[^@\s#]+)@(?<ref>[^\s#]+)')

    foreach ($match in $usesMatches) {
        $action = $match.Groups["action"].Value
        $reference = $match.Groups["ref"].Value

        if ($approvedCompositeActions.ContainsKey($action)) {
            if ($reference -notin $approvedCompositeActions[$action]) {
                $violations.Add("$($workflowFile.Name)：$action@$reference 未列入已审查的复合 Action 引用。")
            }
            else {
                $checkedActions.Add("$action@$reference（复合 Action）")
            }
            continue
        }

        if (-not $minimumMajors.ContainsKey($action)) {
            $violations.Add("$($workflowFile.Name)：$action@$reference 尚未完成运行时审查，请确认不是 Node.js 20 后再加入策略清单。")
            continue
        }

        $majorMatch = [regex]::Match($reference, '^v(?<major>\d+)(?:$|\.)')
        if (-not $majorMatch.Success) {
            $violations.Add("$($workflowFile.Name)：$action@$reference 未使用可审查的 v<主版本> 引用。")
            continue
        }

        $major = [int]$majorMatch.Groups["major"].Value
        $minimum = $minimumMajors[$action]
        $checkedActions.Add("$action@$reference")
        if ($major -lt $minimum) {
            $violations.Add("$($workflowFile.Name)：$action@$reference 低于 Node.js 24 策略要求 v$minimum。")
        }
    }

    $nodeVersionMatches = [regex]::Matches($content, '(?m)^\s*node-version:\s*["'']?(?<major>\d+)')
    foreach ($match in $nodeVersionMatches) {
        $nodeMajor = [int]$match.Groups["major"].Value
        if ($nodeMajor -lt 24) {
            $violations.Add("$($workflowFile.Name)：node-version $nodeMajor 低于项目 CI 要求的 Node.js 24。")
        }
    }
}

if ($violations.Count -gt 0) {
    $violations | ForEach-Object { Write-Error $_ }
    throw "GitHub Actions 运行时策略检查失败。"
}

$checkedActions |
    Sort-Object -Unique |
    ForEach-Object { Write-Host "已确认 GitHub Action 运行时：$_" }
Write-Host "GitHub Actions 运行时策略检查通过。"
