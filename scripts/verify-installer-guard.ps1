[CmdletBinding()]
param([string]$OutputDirectory)
$ErrorActionPreference = 'Stop'
$repository = [IO.Directory]::GetParent($PSScriptRoot).FullName
if (!$OutputDirectory) { $OutputDirectory = Join-Path $repository ('release/installer-guard-tests-' + [guid]::NewGuid().ToString('N')) }
$testRoot = [IO.Path]::GetFullPath($OutputDirectory)
if (Test-Path -LiteralPath $testRoot) { throw 'Use a new isolated output directory' }
New-Item -ItemType Directory -Path $testRoot | Out-Null
Add-Type -Path (Join-Path $repository 'src-tauri/windows/InstallGuard.cs') -ReferencedAssemblies 'System.dll','System.Core.dll','System.Xml.dll'
$names = [InLine.Installer.Guard]::Names
$results = [Collections.Generic.List[object]]::new()
function Assert($condition, [string]$message) { if (!$condition) { throw $message } }
function Fixture([string]$name, [bool]$old = $true) {
    $root = Join-Path $testRoot $name
    $payload = Join-Path $testRoot ($name + '-payload')
    New-Item -ItemType Directory -Path $root,$payload | Out-Null
    foreach ($file in $names) {
        [IO.File]::WriteAllText((Join-Path $payload $file), ('new-' + $file))
        if ($old) { [IO.File]::WriteAllText((Join-Path $root $file), ('old-' + $file)) }
    }
    foreach ($file in @('inline.db','authorization.dpapi','preferences.json','config.toml')) { [IO.File]::WriteAllText((Join-Path $root $file), 'synthetic protected bytes') }
    return @{root=$root;payload=$payload}
}
function Snapshot([string]$root) {
    $map = @{}
    Get-ChildItem -LiteralPath $root -File | ForEach-Object { $map[$_.Name] = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash }
    return $map
}
function EqualSnapshot($a, $b) { Assert ($a.Count -eq $b.Count) 'File count changed'; foreach ($name in $a.Keys) { Assert ($a[$name] -eq $b[$name]) ('Changed bytes: ' + $name) } }
function Pass([string]$name) { $results.Add(@{name=$name;passed=$true}) }

$f=Fixture 'fresh' $false; $before=Snapshot $f.root
$r=[InLine.Installer.Guard]::Run($f.root,$f.payload,'install',-1); Assert ($r.Code -eq 0) $r.Message
foreach ($name in $names) { Assert ((Get-FileHash (Join-Path $f.root $name)).Hash -eq (Get-FileHash (Join-Path $f.payload $name)).Hash) 'Incomplete fresh group' }
Assert (Test-Path (Join-Path $f.root '.in-line-install-transaction')) 'Missing launch barrier'
$r=[InLine.Installer.Guard]::Run($f.root,$f.payload,'finalize',-1); Assert ($r.Code -eq 0) $r.Message
foreach ($name in $before.Keys) { Assert ((Snapshot $f.root)[$name] -eq $before[$name]) 'Protected data changed' }; Pass 'fresh install, full group and protected data'

foreach ($operation in @('install','uninstall')) {
    foreach ($point in 1..5) {
        $f=Fixture ($operation+'-fault-'+$point); $before=Snapshot $f.root
        $r=[InLine.Installer.Guard]::Run($f.root,$f.payload,$operation,$point)
        Assert ($r.Code -ne 0 -and $r.RolledBack) ('Failure not rolled back: '+$r.Message)
        EqualSnapshot $before (Snapshot $f.root)
        Assert (!(Test-Path (Join-Path $f.root '.in-line-install-transaction'))) 'Rollback barrier not cleared'
        Pass ($operation+' failure rollback after file '+$point)
    }
}
$f=Fixture 'fresh-failure' $false; $before=Snapshot $f.root
$r=[InLine.Installer.Guard]::Run($f.root,$f.payload,'install',2); Assert $r.RolledBack $r.Message
EqualSnapshot $before (Snapshot $f.root); Pass 'failed fresh install leaves no partial programs'

$f=Fixture 'held-file'; $before=Snapshot $f.root
$held=[IO.File]::Open((Join-Path $f.root 'in-line-cli.exe'),[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read)
try { $r=[InLine.Installer.Guard]::Run($f.root,$f.payload,'install',-1); Assert ($r.Code -ne 0) 'Ignored held file'; EqualSnapshot $before (Snapshot $f.root) } finally { $held.Dispose() }
Pass 'all-file preflight rejects a locked companion before mutation'

$f=Fixture 'denied-access'; $before=Snapshot $f.root; $denied=Join-Path $f.root 'in-line-mcp.exe'
$acl=Get-Acl -LiteralPath $denied; $saved=$acl.Sddl
$rule=[Security.AccessControl.FileSystemAccessRule]::new([Security.Principal.WindowsIdentity]::GetCurrent().User,[Security.AccessControl.FileSystemRights]'Write,Delete',[Security.AccessControl.AccessControlType]::Deny)
$acl.AddAccessRule($rule); Set-Acl -LiteralPath $denied -AclObject $acl
try { $r=[InLine.Installer.Guard]::Run($f.root,$f.payload,'install',-1); Assert ($r.Code -ne 0) 'Ignored access denial'; EqualSnapshot $before (Snapshot $f.root) } finally { $restore=Get-Acl -LiteralPath $denied; $restore.SetSecurityDescriptorSddlForm($saved); Set-Acl -LiteralPath $denied -AclObject $restore }
Pass 'access denial stops without fallback or data change'

$f=Fixture 'serialized-installers'; $before=Snapshot $f.root
$lockPath=Join-Path $f.root '.in-line-install.lock'
$lock=[IO.File]::Open($lockPath,[IO.FileMode]::CreateNew,[IO.FileAccess]::ReadWrite,[IO.FileShare]::None)
try { $r=[InLine.Installer.Guard]::Run($f.root,$f.payload,'install',-1); Assert ($r.Code -ne 0) 'Concurrent installer accepted' } finally { $lock.Dispose(); [IO.File]::Delete($lockPath) }
EqualSnapshot $before (Snapshot $f.root); Pass 'concurrent installer rejected without deleting another lock'

$f=Fixture 'incomplete-payload'; $before=Snapshot $f.root
[IO.File]::Delete((Join-Path $f.payload 'in-line-mcp.exe'))
$r=[InLine.Installer.Guard]::Run($f.root,$f.payload,'install',-1); Assert ($r.Code -ne 0) 'Incomplete payload accepted'
EqualSnapshot $before (Snapshot $f.root); Pass 'missing payload member fails before target mutation'

$f=Fixture 'missing-finalize-journal'; $before=Snapshot $f.root
$r=[InLine.Installer.Guard]::Run($f.root,$f.payload,'finalize',-1); Assert ($r.Code -ne 0) 'Claimed completion without journal'
EqualSnapshot $before (Snapshot $f.root); Pass 'missing completion journal cannot report success'

$f=Fixture 'unknown-lock'; $lockPath=Join-Path $f.root '.in-line-install.lock'; [IO.File]::WriteAllText($lockPath,'retain unknown lock')
$before=Snapshot $f.root; $r=[InLine.Installer.Guard]::Run($f.root,$f.payload,'install',-1)
Assert ($r.Code -ne 0) 'Accepted unknown lock'; EqualSnapshot $before (Snapshot $f.root); Pass 'unknown lock contents retained'

$f=Fixture 'post-install-rollback'; $before=Snapshot $f.root
$rollbackFile=Join-Path $f.root 'in-line-mcp.exe'; $beforeAcl=(Get-Acl -LiteralPath $rollbackFile).Sddl; $beforeTime=[IO.File]::GetLastWriteTimeUtc($rollbackFile).Ticks
$r=[InLine.Installer.Guard]::Run($f.root,$f.payload,'install',-1); Assert ($r.Code -eq 0) $r.Message
$r=[InLine.Installer.Guard]::Run($f.root,$f.payload,'rollback',-1); Assert ($r.Code -eq 0) $r.Message
EqualSnapshot $before (Snapshot $f.root)
Assert ((Get-Acl -LiteralPath $rollbackFile).Sddl -eq $beforeAcl -and [IO.File]::GetLastWriteTimeUtc($rollbackFile).Ticks -eq $beforeTime) 'Rollback changed ACL or write timestamp'
Pass 'metadata-phase rollback restores previous complete group, ACL and timestamp'

$f=Fixture 'uninstall-success'; $before=Snapshot $f.root
$r=[InLine.Installer.Guard]::Run($f.root,$f.payload,'uninstall',-1); Assert ($r.Code -eq 0) $r.Message
foreach ($name in $names) { Assert (!(Test-Path (Join-Path $f.root $name))) 'Partial uninstall' }
$r=[InLine.Installer.Guard]::Run($f.root,$f.payload,'finalize',-1); Assert ($r.Code -eq 0) $r.Message
foreach ($name in @('inline.db','authorization.dpapi','preferences.json','config.toml')) { Assert ((Snapshot $f.root)[$name] -eq $before[$name]) 'Uninstall changed protected data' }
Pass 'uninstall removes complete program group and retains historical data'

$f=Fixture 'concurrent-change'; $r=[InLine.Installer.Guard]::Run($f.root,$f.payload,'install',-1); Assert ($r.Code -eq 0) $r.Message
[IO.File]::WriteAllText((Join-Path $f.root 'in-line-cli.exe'),'external edit')
$before=Snapshot $f.root; $r=[InLine.Installer.Guard]::Run($f.root,$f.payload,'rollback',-1)
Assert ($r.Code -ne 0) 'Overwrote concurrent edit'; EqualSnapshot $before (Snapshot $f.root)
Assert (Test-Path (Join-Path $f.root '.in-line-install-transaction')) 'Lost recovery barrier'; Pass 'concurrent edit refuses rollback and keeps backups/barrier'
$r=[InLine.Installer.Guard]::Run($f.root,$f.payload,'finalize',-1); Assert ($r.Code -ne 0) 'Finalized mixed group'; Pass 'finalize refuses changed group'

$f=Fixture 'unknown-transaction'; $before=Snapshot $f.root; $pending=Join-Path $f.root '.in-line-install-transaction'
New-Item -ItemType Directory -Path $pending | Out-Null; [IO.File]::WriteAllText((Join-Path $pending 'unknown.txt'),'retain')
$r=[InLine.Installer.Guard]::Run($f.root,$f.payload,'install',-1); Assert ($r.Code -ne 0) 'Overwrote pending recovery'
EqualSnapshot $before (Snapshot $f.root); Assert ([IO.File]::ReadAllText((Join-Path $pending 'unknown.txt')) -eq 'retain') 'Deleted old recovery'
Pass 'unknown pending transaction stops without cleanup'

$f=Fixture 'hard-link'; $link=Join-Path $testRoot 'other-hard-link.exe'
New-Item -ItemType HardLink -Path $link -Target (Join-Path $f.root 'in-line-mcp.exe') | Out-Null
$before=Snapshot $f.root; $r=[InLine.Installer.Guard]::Run($f.root,$f.payload,'install',-1)
Assert ($r.Code -ne 0) 'Accepted hard-linked target'; EqualSnapshot $before (Snapshot $f.root); Pass 'hard link cannot alter another directory'

$f=Fixture 'junction'; $junction=Join-Path $testRoot 'linked-install'
New-Item -ItemType Junction -Path $junction -Target $f.root | Out-Null
$before=Snapshot $f.root; $r=[InLine.Installer.Guard]::Run($junction,$f.payload,'install',-1)
Assert ($r.Code -ne 0) 'Accepted redirected directory'; EqualSnapshot $before (Snapshot $f.root); Pass 'junction target rejected'

# Real, harmless fixture executables: controlled only by their private exit file.
$fixtureSource='using System; using System.IO; using System.Threading; class Fixture { static void Main(string[] args) { while (!File.Exists(args[0])) Thread.Sleep(20); } }'
$fixtureExe=Join-Path $testRoot 'fixture.exe'
Add-Type -TypeDefinition $fixtureSource -OutputAssembly $fixtureExe -OutputType ConsoleApplication
$f=Fixture 'live-target'; $other=Fixture 'other-install'
Copy-Item -LiteralPath $fixtureExe -Destination (Join-Path $f.root 'in-line-mcp.exe') -Force
Copy-Item -LiteralPath $fixtureExe -Destination (Join-Path $other.root 'in-line-mcp.exe') -Force
$targetSignal=Join-Path $testRoot 'target.exit'; $otherSignal=Join-Path $testRoot 'other.exit'
$target=Start-Process -FilePath (Join-Path $f.root 'in-line-mcp.exe') -ArgumentList ('"'+$targetSignal+'"') -WindowStyle Hidden -PassThru
$nonTarget=Start-Process -FilePath (Join-Path $other.root 'in-line-mcp.exe') -ArgumentList ('"'+$otherSignal+'"') -WindowStyle Hidden -PassThru
try {
    Start-Sleep -Milliseconds 150
    $targetStart=$target.StartTime.Ticks; $otherStart=$nonTarget.StartTime.Ticks; $before=Snapshot $f.root; $otherBefore=Snapshot $other.root
    $identified=[InLine.Installer.Guard]::Inspect($f.root)
    Assert (@($identified | Where-Object { $_.Pid -eq $target.Id -and $_.IdentityConfirmed }).Count -eq 1) 'Target holder identity not confirmed'
    Assert (@($identified | Where-Object { $_.Pid -eq $nonTarget.Id }).Count -eq 0) 'Other directory misidentified'
    $r=[InLine.Installer.Guard]::Run($f.root,$f.payload,'install',-1); Assert ($r.Code -ne 0) 'Ignored mapped executable'
    EqualSnapshot $before (Snapshot $f.root); EqualSnapshot $otherBefore (Snapshot $other.root)
    Assert (!$target.HasExited -and !$nonTarget.HasExited) 'Guard stopped a process'
    Assert ($target.StartTime.Ticks -eq $targetStart -and $nonTarget.StartTime.Ticks -eq $otherStart) 'Process replaced'
    Pass 'running target blocks; same-name other directory and both process identities unchanged'
    $r=[InLine.Installer.Guard]::Run($f.root,$f.payload,'uninstall',-1); Assert ($r.Code -ne 0) 'Uninstall ignored mapped executable'
    EqualSnapshot $before (Snapshot $f.root); EqualSnapshot $otherBefore (Snapshot $other.root)
    Assert (!$target.HasExited -and !$nonTarget.HasExited) 'Uninstall stopped a process'
    Pass 'uninstall also stops before removing files while a target is running'
    [IO.File]::WriteAllText($targetSignal,'exit'); Assert ($target.WaitForExit(5000)) 'Fixture did not exit'
    $r=[InLine.Installer.Guard]::Run($f.root,$f.payload,'install',-1); Assert ($r.Code -eq 0) $r.Message
    Assert (!$nonTarget.HasExited) 'Exited PID was used to affect unrelated process'
    EqualSnapshot $otherBefore (Snapshot $other.root); Pass 'target exits after identification; retry succeeds without stale-PID action'
    $r=[InLine.Installer.Guard]::Run($f.root,$f.payload,'finalize',-1); Assert ($r.Code -eq 0) $r.Message
} finally {
    [IO.File]::WriteAllText($targetSignal,'exit'); [IO.File]::WriteAllText($otherSignal,'exit')
    Assert ($target.WaitForExit(5000) -and $nonTarget.WaitForExit(5000)) 'Fixture cleanup failed'
    $target.Dispose(); $nonTarget.Dispose()
}
$report=@{passed=$true;cases=$results.Count;checks=@($results);root=$testRoot;formalEnvironmentTouched=$false;processTerminationUsed=$false;differentUserProcessTerminationTested=$false;differentUserProtection='No process-termination API exists; actual second-user session remains untested';nativeInstallerExecuted=$false}
$report | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $testRoot 'results.json') -Encoding UTF8
Write-Output ('PASS: ' + $results.Count + ' isolated installer guard cases; no process termination or native installation.')
