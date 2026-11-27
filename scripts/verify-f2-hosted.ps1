$ErrorActionPreference='Stop'
$baseline='62288a61f673f49ec8530063bb8dc349dcf3a934'
$root=$env:F2_RUN_ROOT
if ($env:RUNNER_ENVIRONMENT -ne 'github-hosted' -or $env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_OS -ne 'Windows' -or !$root) { throw 'F2 isolation unavailable' }
$checkout=(Get-Location).ProviderPath
$probeSource=Join-Path $checkout 'scripts/f2_production_open_probe.rs'
$manifest='src-tauri/Cargo.toml'

function Assert-Native([string]$step) {
  if ($LASTEXITCODE) { throw "$step exited $LASTEXITCODE" }
}
function Invoke-Case([string[]]$CargoArgs) {
  $lines=@(cargo @CargoArgs 2>&1 | ForEach-Object { "$_" })
  $code=$LASTEXITCODE
  $lines | ForEach-Object { Write-Host $_ }
  if ($code -or -not ($lines -match 'test result: ok\. [1-9][0-9]* passed; 0 failed;')) {
    throw "No passing real tests: $($CargoArgs -join ' ') (exit $code)"
  }
}
function Build-Probe([string]$label) {
  Copy-Item -LiteralPath $probeSource -Destination 'src-tauri/tests/f2_production_open_probe.rs'
  $out=Join-Path $root "$label-cargo.jsonl"
  cargo test --locked --manifest-path $manifest --test f2_production_open_probe --no-run --message-format=json-render-diagnostics > $out
  Assert-Native "$label compile"
  $exe=Get-Content -LiteralPath $out -Encoding utf8 | ForEach-Object {
    try { $_ | ConvertFrom-Json -ErrorAction Stop } catch { }
  } | Where-Object { $_.reason -eq 'compiler-artifact' -and $_.target.name -eq 'f2_production_open_probe' -and $_.executable } | Select-Object -Last 1 -ExpandProperty executable
  if (!$exe -or !(Test-Path -LiteralPath $exe -PathType Leaf)) { throw "Missing $label test exe" }
  $keep=Join-Path $root "$label-probe.exe"
  Copy-Item -LiteralPath $exe -Destination $keep
  Write-Host "F2 $label probe SHA256=$((Get-FileHash -LiteralPath $keep).Hash)"
  return $keep
}
function Invoke-Probe([string]$exe,[string]$mode,[string]$data) {
  $env:F2_PROBE_MODE=$mode
  $env:IN_LINE_MCP_TEST_DATA_ROOT=$data
  try {
    & $exe --exact f2_production_open_probe --nocapture --test-threads=1
    Assert-Native "F2 $mode"
  } finally {
    Remove-Item Env:IN_LINE_MCP_TEST_DATA_ROOT,Env:F2_PROBE_MODE -ErrorAction SilentlyContinue
  }
}
$baseDir=Join-Path $root phase1
git worktree add --detach $baseDir $baseline
Assert-Native 'baseline checkout'
$fixture=Join-Path $root 'v14-to-v15'
Push-Location -LiteralPath $baseDir
try {
  if ((git rev-parse HEAD) -ne $baseline) { throw 'Wrong phase 1 baseline' }
  npm ci --no-audit --no-fund
  Assert-Native 'baseline npm ci'
  npm run build
  Assert-Native 'baseline build'
  Invoke-Case @('test','--locked','--manifest-path',$manifest,'--lib','production_upgrade_from_v8_preserves_data_and_creates_pre_migration_backup','--','--nocapture')
  Write-Host 'F2 actual phase1 v8 to v14 PASS'
  $old=Build-Probe phase1
  Invoke-Probe $old seed $fixture
} finally { Pop-Location }

Invoke-Case @('test','--locked','--manifest-path',$manifest,'--lib','timeline_','--','--nocapture','--test-threads=1')
foreach ($test in @('production_upgrade_from_v8_preserves_data_and_creates_pre_migration_backup','merge_tasks_preserves_history_and_deduplicates_events','archive_delete_restore_and_permanent_cleanup_apply_only_the_selected_scope','p4_batch_refs_two_level_structure_and_group_undo_are_atomic','p4_deadline_batch_requires_single_approval_and_cannot_be_smuggled_as_patch','gui_transactions_and_failed_batches_do_not_leave_derived_events','pre_v15_cursor_requires_full_resync_without_mutating_business_data','quota_lru_expiry_scope_and_clock_guards_do_not_touch_other_stores','statistics_half_open_periods_and_fixed_offset_trend_across_dst','reactivation_retains_historical_statistics_clears_deadline_and_starts_new_cycle')) {
  Invoke-Case @('test','--locked','--manifest-path',$manifest,'--lib',$test,'--','--nocapture','--test-threads=1')
}
# Exact build identity requires an unmodified checkout. Keep the temporary
# probe out of the source tree until protocol and host assertions complete.
if (git status --porcelain --untracked-files=normal) { throw 'Protocol build checkout must be clean' }
Write-Host "F2 exact identity protocol checkout=$(git rev-parse HEAD), clean=true"
Invoke-Case @('test','--locked','--manifest-path',$manifest,'--test','cli_foundation','phase4::','--','--nocapture','--test-threads=1')
Invoke-Case @('test','--locked','--manifest-path',$manifest,'--test','mcp_queries','--','--nocapture','--test-threads=1')
Invoke-Case @('test','--locked','--manifest-path',$manifest,'--test','mcp_host','real_host_background_single_instance_and_protocol','--','--nocapture','--test-threads=1')

$candidate=Build-Probe candidate
Invoke-Probe $candidate upgrade $fixture
$v15=(Get-FileHash -LiteralPath (Join-Path $fixture 'inline.db')).Hash
Invoke-Probe $old reject $fixture
$before=@(Get-ChildItem -LiteralPath (Join-Path $fixture backups) -File -Filter '*-before-migration.db')
if ($before.Count -ne 1) { throw 'Missing matched migration backup' }
$rollback=Join-Path $root rollback
[void][IO.Directory]::CreateDirectory($rollback)
$restored=Join-Path $rollback inline.db
Copy-Item -LiteralPath $before[0].FullName -Destination $restored
if ((Get-FileHash -LiteralPath $restored).Hash -ne (Get-FileHash -LiteralPath $before[0].FullName).Hash) { throw 'Rollback snapshot hash mismatch' }
Invoke-Probe $old rollback $rollback
if ((Get-FileHash -LiteralPath (Join-Path $fixture 'inline.db')).Hash -ne $v15) { throw 'v15 candidate was altered during rollback' }
Write-Host "F2 matching pre-upgrade copy verified and v15 preserved SHA256=$v15"
git diff --exit-code -- package-lock.json src-tauri/Cargo.lock .github/workflows/ci.yml
Assert-Native 'frozen files'
if (Get-Process -Name 'in-line','in-line-mcp','in-line-cli' -ErrorAction SilentlyContinue) { throw 'Unexpected residual product process' }
Write-Host 'F2 hosted migration, rollback and scoped protocol verification PASS'
