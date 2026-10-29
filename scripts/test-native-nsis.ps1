[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidateSet('lifecycle','upgrade','file-lock','mcp-lock','uninstaller-lock','file-deny','registry-deny','uninstall-registry-deny','shortcut-deny','ps-exit','ps-compile-deny','ps-hang')][string]$Scenario,
    [Parameter(Mandatory)][string]$PackageDirectory,
    [Parameter(Mandatory)][string]$ExpectedCommit,
    [Parameter(Mandatory)][string]$OutputDirectory
)
$ErrorActionPreference = 'Stop'
# This entry point intentionally cannot install anything on a developer PC.
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted' -or $env:RUNNER_OS -ne 'Windows' -or $env:GITHUB_REPOSITORY -ne 'bluntvoice/In-Line') { throw 'Native installation is restricted to this repository on GitHub-hosted Windows runners.' }
$out = [IO.Path]::GetFullPath($OutputDirectory)
New-Item -ItemType Directory -Path $out -Force | Out-Null
$lab = Join-Path $env:RUNNER_TEMP ('inline-native-' + [guid]::NewGuid().ToString('N'))
$install = Join-Path $lab 'install'
$package = (Resolve-Path -LiteralPath $PackageDirectory).Path
$candidate = Join-Path $package 'candidate.exe'
$baseline = Join-Path $package 'baseline-0.4.0.exe'
$names = @('in-line.exe','in-line-mcp.exe','in-line-cli.exe','WebView2Loader.dll','uninstall.exe')
$arp = 'Software\Microsoft\Windows\CurrentVersion\Uninstall\In Line'
# Observed Publisher of the unchanged reviewed NSIS package is "github".
$vendor = 'Software\github\In Line'
$sid = [Security.Principal.WindowsIdentity]::GetCurrent().User.Value
$checks = [Collections.Generic.List[object]]::new()
$operations = [Collections.Generic.List[object]]::new()
$snapshots = [ordered]@{}
$owned = [Collections.Generic.List[object]]::new()
$streams = [Collections.Generic.List[object]]::new()
$restorations = [Collections.Generic.List[object]]::new()
$sequence = 0
$dataPaths = [Collections.Generic.List[string]]::new()
$result = [ordered]@{scenario=$Scenario;runnerOS=$env:RUNNER_OS;imageOS=$env:ImageOS;imageVersion=$env:ImageVersion;harnessCommit=$env:GITHUB_SHA;productCommit=$ExpectedCommit;nativeInstallerExecuted=$false;nativeUninstallerExecuted=$false;checks=$checks;operations=$operations;snapshots=$snapshots;findings=@();notCovered=@('interactive wizard and tray UX','physical power loss','actual enterprise policy','second Windows user session');credentialsUploaded=$false;formalEnvironmentConnected=$false}
function Check([bool]$condition,[string]$name,[bool]$fatal=$false) {
    $checks.Add([ordered]@{name=$name;passed=$condition})
    if (!$condition -and $fatal) { throw $name }
}
function Hash([string]$path) {
    if(Test-Path -LiteralPath $path -PathType Leaf) {
        try { return (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant() }
        catch { return $null } # Snapshot records existence separately; unreadable bytes never equal a known digest.
    }
    return $null
}
function ProgramState {
    $map=[ordered]@{}
    foreach($name in $names) { $path=Join-Path $install $name; $map[$name]=@{exists=(Test-Path -LiteralPath $path -PathType Leaf);sha256=(Hash $path)} }
    foreach($legacy in @(Get-ChildItem -LiteralPath $install -Filter '*.exe' -File -ErrorAction SilentlyContinue)) { if(!$map.Contains($legacy.Name)) { $map[$legacy.Name]=@{exists=$true;sha256=(Hash $legacy.FullName)} } }
    return $map
}
function RegistryState {
    $map=[ordered]@{}
    foreach($view in @([Microsoft.Win32.RegistryView]::Registry32,[Microsoft.Win32.RegistryView]::Registry64)) {
        $baseKey=[Microsoft.Win32.RegistryKey]::OpenBaseKey([Microsoft.Win32.RegistryHive]::CurrentUser,$view)
        try { foreach($path in @($arp,$vendor)) {
            $key=$baseKey.OpenSubKey($path); $values=[ordered]@{}
            if($key) { try { foreach($name in @($key.GetValueNames() | Sort-Object)) { $values[$name]=$key.GetValue($name) } } finally { $key.Dispose() } }
            $map["$view/$path"]=@{exists=($null -ne $key);values=$values}
        } } finally { $baseKey.Dispose() }
    }
    return $map
}
function ShortcutPaths {
    $desktop=[Environment]::GetFolderPath([Environment+SpecialFolder]::DesktopDirectory,[Environment+SpecialFolderOption]::Create)
    $programs=[Environment]::GetFolderPath([Environment+SpecialFolder]::Programs,[Environment+SpecialFolderOption]::Create)
    return @((Join-Path $desktop 'In Line.lnk'),(Join-Path $programs 'In Line.lnk'),(Join-Path $programs 'In Line/In Line.lnk'))
}
function ShortcutState {
    $map=[ordered]@{}; $shell=New-Object -ComObject WScript.Shell
    foreach($path in (ShortcutPaths)) {
        $entry=@{exists=(Test-Path -LiteralPath $path -PathType Leaf);sha256=(Hash $path);target=$null;arguments=$null}
        if($entry.exists) { try { $link=$shell.CreateShortcut($path);$entry.target=$link.TargetPath;$entry.arguments=$link.Arguments } catch { $entry.readable=$false } }
        $map[$path]=$entry
    }
    return $map
}
function DataState { $map=[ordered]@{}; foreach($path in $dataPaths) { $map[$path]=Hash $path }; return $map }
function Snapshot([string]$label) {
    $state=[ordered]@{programs=(ProgramState);registry=(RegistryState);shortcuts=(ShortcutState);data=(DataState);transactionPresent=(Test-Path -LiteralPath (Join-Path $install '.in-line-install-transaction'));at=[DateTime]::UtcNow.ToString('o')}
    $snapshots[$label]=$state; return $state
}
function Equal($a,$b) { return (($a | ConvertTo-Json -Depth 15 -Compress) -ceq ($b | ConvertTo-Json -Depth 15 -Compress)) }
function Start-Owned([string]$path,[string[]]$arguments=@(),[string]$tag='program',[string]$temporaryRoot='') {
    $info=[Diagnostics.ProcessStartInfo]::new($path);$info.UseShellExecute=$false;$info.CreateNoWindow=$true;$info.RedirectStandardOutput=$true;$info.RedirectStandardError=$true;$info.RedirectStandardInput=$true
    foreach($argument in $arguments) { $info.ArgumentList.Add($argument) }
    foreach($key in @($info.Environment.Keys)) { if($key -match 'TOKEN|SECRET|PASSWORD|CREDENTIAL|^IN_LINE_MCP_') { $info.Environment.Remove($key) | Out-Null } }
    if($temporaryRoot) { $info.Environment['TEMP']=$temporaryRoot;$info.Environment['TMP']=$temporaryRoot }
    $process=[Diagnostics.Process]::Start($info)
    $item=@{process=$process;pid=$process.Id;started=$process.StartTime.ToUniversalTime().Ticks;path=[IO.Path]::GetFullPath($path);stdout=$process.StandardOutput.ReadToEndAsync();stderr=$process.StandardError.ReadToEndAsync();tag=$tag}
    $owned.Add($item);return $item
}
function Stop-Owned($item) {
    $p=$item.process
    if(!$p.HasExited) {
        $current=Get-Process -Id $item.pid -ErrorAction Stop
        if($current.StartTime.ToUniversalTime().Ticks -ne $item.started -or ![string]::Equals($current.Path,$item.path,[StringComparison]::OrdinalIgnoreCase)) { throw 'Owned-process identity changed; refusing cleanup.' }
        $p.Kill($true);$p.WaitForExit(10000) | Out-Null
    }
}
Add-Type -TypeDefinition @'
using System; using System.Diagnostics; using System.Collections.Generic; using System.Runtime.InteropServices;
public static class NativeNsisLabThreads {
 [DllImport("kernel32.dll")] static extern IntPtr OpenThread(uint a,bool b,uint id);
 [DllImport("kernel32.dll")] static extern uint SuspendThread(IntPtr h);
 [DllImport("kernel32.dll")] static extern uint ResumeThread(IntPtr h);
 [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr h);
 [DllImport("kernel32.dll",SetLastError=true)] static extern bool GetExitCodeProcess(IntPtr h,out uint code);
 public static uint ReadExitCode(IntPtr handle) { uint code; if(!GetExitCodeProcess(handle,out code)) throw new InvalidOperationException("Cannot read held test process exit code"); return code; }
 public static IntPtr[] Pause(int id) { var handles=new List<IntPtr>(); foreach(ProcessThread t in Process.GetProcessById(id).Threads) { var h=OpenThread(2,false,(uint)t.Id); if(h!=IntPtr.Zero) { if(SuspendThread(h)!=uint.MaxValue) handles.Add(h); else CloseHandle(h); } } return handles.ToArray(); }
 public static void Resume(IntPtr[] handles) { foreach(var h in handles) { ResumeThread(h);CloseHandle(h); } }
}
'@
function Deny-Access([string]$path,[Security.AccessControl.FileSystemRights]$rights) {
    $acl=Get-Acl -LiteralPath $path; $original=$acl.GetSecurityDescriptorSddlForm('All')
    $rule=[Security.AccessControl.FileSystemAccessRule]::new([Security.Principal.SecurityIdentifier]::new($sid),$rights,[Security.AccessControl.AccessControlType]::Deny)
    $acl.AddAccessRule($rule);Set-Acl -LiteralPath $path -AclObject $acl
    $entry=@{path=$path;sddl=$original};$restorations.Add($entry);return $entry
}
function Restore-Access($entry) {
    $acl=Get-Acl -LiteralPath $entry.path;$acl.SetSecurityDescriptorSddlForm($entry.sddl);Set-Acl -LiteralPath $entry.path -AclObject $acl
}
function Run-NSIS([string]$exe,[string]$label,[bool]$uninstalling=$false,[string]$fault='') {
    $script:sequence++
    if($uninstalling) { $result.nativeUninstallerExecuted=$true;$arguments=@('/S') } else { $result.nativeInstallerExecuted=$true;$arguments=@('/S',"/D=$install") }
    $privateTemp=Join-Path $lab ("private-temp-$sequence");New-Item -ItemType Directory -Path $privateTemp | Out-Null
    $item=Start-Owned $exe $arguments $label $privateTemp;$process=$item.process
    $operation=[ordered]@{label=$label;pid=$item.pid;createdUtcTicks=$item.started;executable=$item.path;arguments=$arguments;fault=$fault;faultApplied=$false;helper=$null;timedOut=$false;exitCode=$null;stdoutFile="$sequence-stdout.txt";stderrFile="$sequence-stderr.txt";silentNsisDetailLogAvailable=$false}
    $operations.Add($operation);$deadline=[DateTime]::UtcNow.AddSeconds(180);$pause=@();$resumeAt=$null;$tempAcl=$null;$helperProcess=$null
    $descendants=[Collections.Generic.List[object]]::new();$operation.descendants=$descendants;$nextObservation=[DateTime]::MinValue;$rootExitedAt=$null
    try {
        while([DateTime]::UtcNow -lt $deadline) {
            if($uninstalling -and [DateTime]::UtcNow -ge $nextObservation) {
                # NSIS can copy its uninstaller and let the original stub exit.
                # Observe and hold the real temporary body's process handle.
                $observed=@(Get-CimInstance Win32_Process -Filter "ParentProcessId=$($item.pid)" | Where-Object { $_.ExecutablePath -and $_.ExecutablePath.StartsWith($privateTemp,[StringComparison]::OrdinalIgnoreCase) })
                foreach($child in $observed) {
                    if(!@($descendants | Where-Object pid -EQ $child.ProcessId).Count) {
                        $body=Get-Process -Id $child.ProcessId -ErrorAction SilentlyContinue
                        if($body -and $body.StartTime.ToUniversalTime().Ticks -ge $item.started) {
                            $descendants.Add(@{pid=$body.Id;parent=$item.pid;path=$child.ExecutablePath;created=$body.StartTime.ToUniversalTime().Ticks;process=$body;exitHandle=$body.SafeHandle})
                            $owned.Add(@{process=$body;pid=$body.Id;started=$body.StartTime.ToUniversalTime().Ticks;path=$child.ExecutablePath;tag='copied-native-uninstaller'})
                        }
                    }
                }
                $nextObservation=[DateTime]::UtcNow.AddMilliseconds(150)
            }
            if($process.HasExited) {
                if(!$uninstalling) { break }
                if(!$rootExitedAt) { $rootExitedAt=[DateTime]::UtcNow }
                $active=@($descendants | Where-Object { !$_.process.HasExited })
                if(!$active.Count -and ($descendants.Count -or ([DateTime]::UtcNow-$rootExitedAt).TotalSeconds -ge 2)) { break }
            }
            if($fault -eq 'deny-compile-temp' -and !$operation.faultApplied) {
                $wrapper=@(Get-ChildItem -LiteralPath $privateTemp -Filter install-guard.ps1 -Recurse -File -ErrorAction SilentlyContinue)
                if($wrapper.Count) { $tempAcl=Deny-Access $privateTemp ([Security.AccessControl.FileSystemRights]::CreateFiles);$operation.faultApplied=$true;$operation.privateTempDenied=$privateTemp }
            }
            if($fault -in @('terminate-helper','suspend-helper','deny-compile-temp') -and !$operation.helper) {
                $children=@(Get-CimInstance Win32_Process -Filter "ParentProcessId=$($item.pid)" | Where-Object { $_.Name -eq 'powershell.exe' -and $_.CommandLine -and $_.CommandLine.Contains('install-guard.ps1') -and $_.CommandLine.Contains($install) })
                if($children.Count -eq 1) {
                    $child=$children[0];$helperProcess=Get-Process -Id $child.ProcessId -ErrorAction SilentlyContinue
                    if($helperProcess -and $helperProcess.StartTime.ToUniversalTime().Ticks -ge $item.started) {
                        $operation.helper=@{pid=$child.ProcessId;parent=$child.ParentProcessId;image=$child.ExecutablePath;createdUtcTicks=$helperProcess.StartTime.ToUniversalTime().Ticks}
                        if($fault -eq 'terminate-helper') { $helperProcess.Kill($true);$operation.faultApplied=$true }
                        if($fault -eq 'suspend-helper') { $pause=@([NativeNsisLabThreads]::Pause($child.ProcessId));$operation.faultApplied=($pause.Count -gt 0);$resumeAt=[DateTime]::UtcNow.AddSeconds(15) }
                    }
                }
            }
            if($resumeAt -and [DateTime]::UtcNow -ge $resumeAt) {
                $operation.installerStillWaitingAfter15Seconds=!$process.HasExited
                $operation.stateDuringSuspension=Snapshot ($label+'-suspended')
                [NativeNsisLabThreads]::Resume($pause);$pause=@();$resumeAt=$null;$operation.resumedByHarness=$true
            }
            Start-Sleep -Milliseconds 30
        }
        if(!$process.HasExited -or @($descendants | Where-Object { !$_.process.HasExited }).Count) { $operation.timedOut=$true;Stop-Owned $item;foreach($body in $descendants) { $entry=@{process=$body.process;pid=$body.pid;started=$body.created;path=$body.path};Stop-Owned $entry } }
        $process.WaitForExit();$operation.rootExitCode=$process.ExitCode;$operation.exitCode=$process.ExitCode
        foreach($body in $descendants) { $body.process.WaitForExit();$body.exitCode=[NativeNsisLabThreads]::ReadExitCode($body.exitHandle.DangerousGetHandle());$body.exitCodeSource='GetExitCodeProcess-held-handle';$body.Remove('process');$body.Remove('exitHandle');$operation.exitCode=$body.exitCode }
        $operation.copiedUninstallerObserved=($descendants.Count -gt 0)
        if($helperProcess -and $helperProcess.HasExited) { $operation.helperExitCode=$helperProcess.ExitCode }
        $item.stdout.GetAwaiter().GetResult() | Set-Content -LiteralPath (Join-Path $out $operation.stdoutFile) -Encoding utf8
        $item.stderr.GetAwaiter().GetResult() | Set-Content -LiteralPath (Join-Path $out $operation.stderrFile) -Encoding utf8
    } finally {
        if($pause.Count) { [NativeNsisLabThreads]::Resume($pause) }
        if($tempAcl) { Restore-Access $tempAcl }
        foreach($body in $descendants) { $body.Remove('process');$body.Remove('exitHandle') }
    }
    return $operation
}
function Assert-Installed([string]$label) {
    foreach($record in $payload.records) { Check ((Hash (Join-Path $install ($record.name+'.exe'))) -eq $record.sha256) "$label payload $($record.name)" }
    Check ((@($names | Where-Object { !(Test-Path -LiteralPath (Join-Path $install $_) -PathType Leaf) })).Count -eq 0) "$label complete five-file group"
    $states=RegistryState;$registered=@($states.Values | Where-Object { $_.values.DisplayVersion -eq '0.5.0' -and $_.values.InstallLocation.Trim('"') -eq $install })
    Check ($registered.Count -ge 1) "$label registered version/location matches installed files"
    Check (!(Test-Path -LiteralPath (Join-Path $install '.in-line-install-transaction'))) "$label transaction finalized"
}
function Install-Candidate([string]$label='install') { $op=Run-NSIS $candidate $label;Check (!$op.timedOut -and $op.exitCode -eq 0) "$label native installer returned success" $true;Assert-Installed $label;return $op }
function Uninstall-Candidate([string]$label='uninstall') {
    $op=Run-NSIS (Join-Path $install 'uninstall.exe') $label $true
    Check (!$op.timedOut -and $op.exitCode -eq 0) "$label native uninstaller returned success"
    Check ((@($names | Where-Object { Test-Path -LiteralPath (Join-Path $install $_) })).Count -eq 0) "$label removed complete five-file group"
    $metadata=RegistryState;Check ((@($metadata.Values | Where-Object { $_.values.Contains('DisplayVersion') })).Count -eq 0) "$label removed ARP registration"
    return $op
}
function Seed-SyntheticData {
    $fixture=Join-Path $install 'synthetic-data';New-Item -ItemType Directory -Path $fixture -Force | Out-Null
    $db=Join-Path $fixture 'inline.db'
    & python -c 'import sqlite3,sys; c=sqlite3.connect(sys.argv[1]); c.execute("create table if not exists synthetic_work(id integer primary key,title text)"); c.execute("insert or ignore into synthetic_work values(1,?)",("NSIS synthetic record",)); c.commit(); c.close()' $db
    if($LASTEXITCODE -ne 0) { throw 'Synthetic SQLite creation failed' }
    $dataPaths.Add($db)
    Add-Type -AssemblyName System.Security.Cryptography.ProtectedData
    $preferences=Join-Path $fixture 'preferences.json';[IO.File]::WriteAllText($preferences,'{"theme":"synthetic","zoom":1}');$dataPaths.Add($preferences)
    $authorization=Join-Path $fixture 'authorization.dpapi';[IO.File]::WriteAllBytes($authorization,[Security.Cryptography.ProtectedData]::Protect([Text.Encoding]::UTF8.GetBytes('{"syntheticClient":"native-lab","regularRead":true,"write":false}'),$null,[Security.Cryptography.DataProtectionScope]::CurrentUser));$dataPaths.Add($authorization)
    $fixtureConfig=Join-Path $fixture 'config.toml';[IO.File]::WriteAllText($fixtureConfig,"[mcp_servers.synthetic]`ncommand='synthetic-fixture'`n");$dataPaths.Add($fixtureConfig)
    $config=Join-Path $env:USERPROFILE '.codex/config.toml';New-Item -ItemType Directory -Path (Split-Path $config) -Force | Out-Null
    [IO.File]::WriteAllText($config,"[mcp_servers.synthetic_other]`ncommand = 'synthetic-fixture'`n");$dataPaths.Add($config)
}
function Probe-Programs {
    foreach($name in @('in-line','in-line-mcp','in-line-cli')) {
        $args=if($name -eq 'in-line-cli') { @('--version','--format','json') } else { @('--build-info') }
        $item=Start-Owned (Join-Path $install ($name+'.exe')) $args ('offline-'+$name)
        Check ($item.process.WaitForExit(15000)) "$name offline startup completed" $true
        $info=$item.stdout.GetAwaiter().GetResult() | ConvertFrom-Json
        Check ($item.process.ExitCode -eq 0 -and $info.data.build.exactCommit -eq $ExpectedCommit -and !$info.data.build.sourceDirty) "$name exact build identity"
    }
    $gui=Start-Owned (Join-Path $install 'in-line.exe') @() 'native-gui-startup'
    $database=Join-Path $env:APPDATA 'in-line/inline.db';$authorization=Join-Path $env:USERPROFILE '.in-line/mcp-security/authorization.dpapi'
    $until=[DateTime]::UtcNow.AddSeconds(25)
    while(!$gui.process.HasExited -and (!(Test-Path -LiteralPath $database) -or !(Test-Path -LiteralPath $authorization)) -and [DateTime]::UtcNow -lt $until) { Start-Sleep -Milliseconds 200 }
    $result.guiStartup=@{processAlive=!$gui.process.HasExited;databaseCreated=(Test-Path -LiteralPath $database);authorizationCreated=(Test-Path -LiteralPath $authorization)}
    if(!$gui.process.HasExited -and (Test-Path -LiteralPath $database)) {
        Check $true 'GUI native process created its database'
        $beforeGuiInstall=Snapshot 'gui-running-before-install';$guiOp=Run-NSIS $candidate 'gui-running-install';$afterGuiInstall=Snapshot 'gui-running-after-install'
        Check ($guiOp.exitCode -ne 0 -and !$guiOp.timedOut) 'Actual running GUI blocks native installation'
        Check (Equal $beforeGuiInstall.programs $afterGuiInstall.programs) 'GUI occupation prevents partial writes'
        $gui.process.Refresh();$window=$gui.process.MainWindowHandle
        if($window -ne [IntPtr]::Zero) { $closeRequested=$gui.process.CloseMainWindow();Start-Sleep -Milliseconds 500;$result.guiStartup.closeRequested=$closeRequested;$result.guiStartup.aliveAfterClose=!$gui.process.HasExited }
        Stop-Owned $gui
        $result.guiStartup.cleanup='Harness-only identity-verified test process termination; tray exit not exercised'
        & python -c 'import sqlite3,sys; c=sqlite3.connect(sys.argv[1]); c.execute("create table if not exists nsis_lab_records(id integer primary key,title text)"); c.execute("insert or replace into nsis_lab_records values(1,?)",("NSIS synthetic business record",)); c.commit(); c.close()' $database
        if($LASTEXITCODE -ne 0) { throw 'Actual product database fixture failed' };$dataPaths.Add($database)
        if(Test-Path -LiteralPath $authorization) {
            Add-Type -AssemblyName System.Security.Cryptography.ProtectedData
            $clear=[Security.Cryptography.ProtectedData]::Unprotect([IO.File]::ReadAllBytes($authorization),$null,[Security.Cryptography.DataProtectionScope]::CurrentUser)
            $store=[Text.Encoding]::UTF8.GetString($clear) | ConvertFrom-Json
            $store.clients=@($store.clients)+@(@{id='nsis-lab-client';name='Synthetic native NSIS client';kind='codex';tokenHash=('a'*64);revoked=$false;permissions=@{regularRead=$true;fullRead=$false;write=$false};scope=@{departments=$null;taskTypes=$null}})
            $store.revision++
            $protected=[Security.Cryptography.ProtectedData]::Protect([Text.Encoding]::UTF8.GetBytes(($store | ConvertTo-Json -Depth 15 -Compress)),$null,[Security.Cryptography.DataProtectionScope]::CurrentUser)
            [IO.File]::WriteAllBytes($authorization,$protected);$dataPaths.Add($authorization)
            $result.syntheticAuthorization=@{dpapiProtected=$true;syntheticClientCount=1;permissions=@{regularRead=$true;fullRead=$false;write=$false};revision=$store.revision;contentsUploaded=$false}
            $clear=$null;$store=$null;$protected=$null
        }
    } else {
        $result.notCovered+=@('GUI functional startup unavailable on this runner; see gui stderr');$item=$gui
    }
    if(!$gui.process.HasExited) { Stop-Owned $gui }
    $gui.stderr.GetAwaiter().GetResult() | Set-Content -LiteralPath (Join-Path $out 'gui-stderr.txt') -Encoding utf8
    & node (Join-Path $PSScriptRoot '../docs/mcp/verify-release-stdio.mjs') (Join-Path $install 'in-line-mcp.exe')
    Check ($LASTEXITCODE -eq 0) 'Installed MCP actual initialize/list/anonymous calls'
    & (Join-Path $install 'in-line-cli.exe') doctor --format json > (Join-Path $out 'cli-doctor.json')
    $result.cliDoctorExit=$LASTEXITCODE
    $doctor=Get-Content -LiteralPath (Join-Path $out 'cli-doctor.json') -Raw | ConvertFrom-Json
    Check ($doctor.apiVersion -eq 1 -and $doctor.status -in @('ok','error')) 'Installed CLI doctor returns its structured protocol (profile authorization separately scoped)'
}
try {
    Check ([IO.Path]::GetFullPath($lab).StartsWith([IO.Path]::GetFullPath($env:RUNNER_TEMP)+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) 'Lab stays under RUNNER_TEMP' $true
    foreach($path in @((Join-Path $env:APPDATA 'in-line'),(Join-Path $env:USERPROFILE '.in-line'),(Join-Path $env:USERPROFILE '.codex/config.toml'))) { Check (!(Test-Path -LiteralPath $path)) 'Fresh hosted user has no preexisting In-Line/Codex data' $true }
    Check ((@((RegistryState).Values | Where-Object exists)).Count -eq 0) 'Fresh hosted user has no product registration' $true
    $provenance=Get-Content (Join-Path $package 'provenance.json') -Raw | ConvertFrom-Json
    $verification=Get-Content (Join-Path $package 'installer-verification.json') -Raw | ConvertFrom-Json
    $payload=Get-Content (Join-Path $package 'installer-payload.json') -Raw | ConvertFrom-Json
    Check ($provenance.productCommit -eq $ExpectedCommit -and $payload.build.exactCommit -eq $ExpectedCommit -and (Hash $candidate) -eq $verification.Sha256.ToLowerInvariant()) 'Verified formal source and installer digest' $true
    $result.installerSha256=Hash $candidate;$result.baselineSha256=Hash $baseline
    New-Item -ItemType Directory -Path $install -Force | Out-Null
    Seed-SyntheticData
    Check ((@((DataState).Values | Where-Object { !$_.Length })).Count -eq 0) 'Synthetic data baseline digests are readable' $true
    $before=Snapshot 'initial'
    if($Scenario -eq 'lifecycle') {
        $null=Install-Candidate 'fresh';$fresh=Snapshot 'fresh'
        Probe-Programs
        $preserved=DataState
        Check ((@($preserved.Values | Where-Object { !$_.Length })).Count -eq 0) 'Product data preservation baseline is readable' $true
        $null=Install-Candidate 'same-version-repair';$null=Snapshot 'repair'
        Check (Equal $preserved (DataState)) 'Repair preserves synthetic database, preferences, authorization and other MCP config'
        $null=Uninstall-Candidate 'actual-uninstall';$null=Snapshot 'uninstall'
        Check (Equal $preserved (DataState)) 'Uninstall preserves all seeded data and authorization bytes'
        $null=Install-Candidate 'reinstall';Check (Equal $preserved (DataState)) 'Reinstall preserves seeded data'
    } elseif($Scenario -eq 'upgrade') {
        Check ((Hash $baseline) -eq 'f8e0bf5d1ec4ef1c4759579713aa8ca49be05624acd74c50af49a62b30842f68') 'Baseline v0.4.0 digest' $true
        $op=Run-NSIS $baseline 'baseline-install';Check ($op.exitCode -eq 0 -and !$op.timedOut) 'Actual v0.4.0 install' $true
        $old=Snapshot 'baseline';$null=Install-Candidate 'upgrade';$new=Snapshot 'upgraded'
        Check (Equal $old.data $new.data) 'Upgrade preserves synthetic data and config'
        $null=Uninstall-Candidate 'upgrade-uninstall'
    } elseif($Scenario -in @('registry-deny','uninstall-registry-deny')) {
        if($Scenario -eq 'uninstall-registry-deny') { $null=Install-Candidate 'prepare' }
        $regPath='Registry::HKEY_CURRENT_USER\'+$arp
        if(!(Test-Path $regPath)) { New-Item -Path $regPath -Force | Out-Null;New-ItemProperty -Path $regPath -Name NativeLabSentinel -Value 'synthetic' | Out-Null }
        $acl=Get-Acl $regPath;$original=$acl.GetSecurityDescriptorSddlForm('All')
        $rights=if($Scenario -eq 'registry-deny') { [Security.AccessControl.RegistryRights]::SetValue } else { [Security.AccessControl.RegistryRights]::Delete }
        $acl.AddAccessRule([Security.AccessControl.RegistryAccessRule]::new([Security.Principal.SecurityIdentifier]::new($sid),$rights,[Security.AccessControl.AccessControlType]::Deny));Set-Acl $regPath $acl
        try {
            $old=Snapshot 'before-registry-denial'
            $savedLinks=@{}
            foreach($path in (ShortcutPaths)) { if(Test-Path -LiteralPath $path -PathType Leaf) { $savedLinks[$path]=[IO.File]::ReadAllBytes($path) } }
            $result.independentFixtureResets=@()
            for($attempt=1;$attempt -le 2;$attempt++) {
                if($attempt -eq 2) {
                    # Keep the first failed result. Restore only known test
                    # metadata to reproduce from the same independent baseline;
                    # this is explicitly not a product rollback or a passing fix.
                    if($Scenario -eq 'registry-deny') {
                        Check ($vendor -eq 'Software\github\In Line') 'Fixture reset restricted to the exact initially absent product key' $true
                        [Microsoft.Win32.Registry]::CurrentUser.DeleteSubKeyTree($vendor,$false)
                    } else {
                        foreach($path in $savedLinks.Keys) {
                            Check (!(Test-Path -LiteralPath $path) -or (Hash $path) -eq $old.shortcuts[$path].sha256) 'Fixture reset refuses an unexpected shortcut replacement' $true
                            [IO.File]::WriteAllBytes($path,$savedLinks[$path])
                        }
                    }
                    $reset=Snapshot 'external-fixture-reset-for-independent-attempt-2'
                    Check ((Equal $old.programs $reset.programs) -and (Equal $old.registry $reset.registry) -and (Equal $old.shortcuts $reset.shortcuts)) 'Independent replay fixture matches original baseline' $true
                    $result.independentFixtureResets+=@(@{forAttempt=2;externalHarnessAction=$true;productRollback=$false;originalFailureRetained=$true})
                }
                $attemptBefore=Snapshot "before-registry-denied-$attempt"
                $op=if($Scenario -eq 'registry-deny') { Run-NSIS $candidate "registry-denied-$attempt" } else { Run-NSIS (Join-Path $install 'uninstall.exe') "uninstall-registry-denied-$attempt" $true }
                $now=Snapshot "registry-denied-$attempt"
                Check ($op.exitCode -ne 0 -and !$op.timedOut) "Registry denied attempt $attempt returns failure"
                Check (Equal $attemptBefore.programs $now.programs) "Registry denied attempt $attempt restores full program group"
                Check (Equal $attemptBefore.registry $now.registry) "Registry denied attempt $attempt restores installation registry"
                Check (Equal $attemptBefore.shortcuts $now.shortcuts) "Registry denied attempt $attempt restores shortcuts"
                Check (Equal $attemptBefore.data $now.data) "Registry denied attempt $attempt preserves data"
            }
        } finally {
            # Do not reopen the denied key for ReadWrite/SetValue merely to
            # restore its DACL. Request only the unchanged WRITE_DAC permission.
            $restore=[Security.AccessControl.RegistrySecurity]::new()
            $restore.SetSecurityDescriptorSddlForm($original,[Security.AccessControl.AccessControlSections]::Access)
            $key=[Microsoft.Win32.Registry]::CurrentUser.OpenSubKey($arp,[Microsoft.Win32.RegistryKeyPermissionCheck]::ReadWriteSubTree,[Security.AccessControl.RegistryRights]::ChangePermissions)
            try { [Microsoft.Win32.RegistryAclExtensions]::SetAccessControl($key,$restore) } finally { $key.Dispose() }
        }
        $null=Install-Candidate 'retry-after-restoring-test-key-permissions';$null=Snapshot 'retry'
    } elseif($Scenario -eq 'shortcut-deny') {
        $desktop=(ShortcutPaths)[0];$program=(ShortcutPaths)[1];$shell=New-Object -ComObject WScript.Shell
        foreach($path in @($desktop,$program)) { New-Item -ItemType Directory -Path (Split-Path $path) -Force | Out-Null;$link=$shell.CreateShortcut($path);$link.TargetPath=Join-Path $lab 'synthetic-old-target.exe';$link.Save();$streams.Add([IO.File]::Open($path,'Open','Read','Read')) }
        $old=Snapshot 'locked-shortcuts'
        for($attempt=1;$attempt -le 2;$attempt++) {
            $op=Run-NSIS $candidate "shortcut-create-denied-$attempt";$now=Snapshot "shortcut-create-denied-$attempt"
            $correct=@($now.shortcuts.Values | Where-Object { $_.target -eq (Join-Path $install 'in-line.exe') })
            Check ($op.exitCode -ne 0 -or $correct.Count -ge 2) "Shortcut creation failure attempt $attempt is not falsely reported as success"
        }
        # Only links that actually target this installation are uninstall targets.
        # Preserve the failed-creation snapshots, then seed a separate valid-target
        # deletion fixture; retaining a foreign target must not be called a defect.
        foreach($stream in $streams) { $stream.Dispose() };$streams.Clear()
        foreach($path in (ShortcutPaths)) {
            if(Test-Path -LiteralPath $path -PathType Leaf) {
                $link=$shell.CreateShortcut($path);$link.TargetPath=Join-Path $install 'in-line.exe';$link.Save()
                $streams.Add([IO.File]::Open($path,'Open','Read','Read'))
            }
        }
        $null=Snapshot 'locked-valid-target-shortcut-deletion-fixture'
        if(Test-Path -LiteralPath (Join-Path $install 'uninstall.exe')) {
            $op=Run-NSIS (Join-Path $install 'uninstall.exe') 'shortcut-delete-denied' $true;$now=Snapshot 'shortcut-delete-denied'
            Check ($op.exitCode -ne 0 -or @($now.shortcuts.Values | Where-Object exists).Count -eq 0) 'Shortcut deletion failure is not falsely reported as success'
        }
        foreach($stream in $streams) { $stream.Dispose() };$streams.Clear()
    } else {
        if($Scenario -in @('ps-exit','ps-compile-deny','ps-hang')) {
            $fault=switch($Scenario) { 'ps-exit' {'terminate-helper'} 'ps-compile-deny' {'deny-compile-temp'} 'ps-hang' {'suspend-helper'} }
            $old=Snapshot 'before-helper-fault';$op=Run-NSIS $candidate $Scenario $false $fault;$now=Snapshot 'after-helper-fault'
            Check $op.faultApplied 'Fault reached the actual NSIS helper/private compiler temp'
            if($Scenario -eq 'ps-hang') {
                $result.hangObservation=@{seconds=15;installerStillWaiting=$op.installerStillWaitingAfter15Seconds;resumedByHarness=$op.resumedByHarness;productTimeoutObserved=(!$op.installerStillWaitingAfter15Seconds)}
                Check (!$op.installerStillWaitingAfter15Seconds) 'Product independently handled the hung helper in the observed window; harness resume does not verify bounded handling'
                $result.notCovered+=@('Longer hang bound and enterprise AppLocker/WDAC; harness resumed only its own helper')
            } else {
                Check ($op.exitCode -ne 0 -and !$op.timedOut) 'Actual NSIS safely rejects helper execution/compilation failure'
                Check (Equal $old.programs $now.programs) 'Helper failure leaves no partial program group'
            }
            $null=Install-Candidate 'retry-after-helper-fault-cleared'
        } else {
            $null=Install-Candidate 'prepare';$old=Snapshot 'before-occupation'
            if($Scenario -eq 'file-deny') { $denial=Deny-Access (Join-Path $install 'in-line-cli.exe') ([Security.AccessControl.FileSystemRights]::WriteData -bor [Security.AccessControl.FileSystemRights]::Delete) }
            elseif($Scenario -eq 'file-lock') { $streams.Add([IO.File]::Open((Join-Path $install 'in-line-cli.exe'),'Open','Read','Read')) }
            elseif($Scenario -eq 'uninstaller-lock') { $streams.Add([IO.File]::Open((Join-Path $install 'uninstall.exe'),'Open','Read','Read')) }
            elseif($Scenario -eq 'mcp-lock') {
                $other=Join-Path $lab 'other-directory';New-Item -ItemType Directory -Path $other | Out-Null
                Copy-Item (Join-Path $install 'in-line-mcp.exe') (Join-Path $other 'in-line-mcp.exe')
                $target=Start-Owned (Join-Path $install 'in-line-mcp.exe') @() 'target-mcp'
                $nonTarget=Start-Owned (Join-Path $other 'in-line-mcp.exe') @() 'other-directory-mcp'
                Start-Sleep -Milliseconds 300
                Check (!$target.process.HasExited -and !$nonTarget.process.HasExited) 'Actual target and same-name non-target MCP processes running' $true
                $result.nonTargetProof=@{before=@{targetPid=$target.pid;targetCreated=$target.started;otherPid=$nonTarget.pid;otherCreated=$nonTarget.started;otherSha256=(Hash $nonTarget.path)}}
            }
            $op=Run-NSIS $candidate 'occupied-install';$now=Snapshot 'occupied-install'
            Check ($op.exitCode -ne 0 -and !$op.timedOut) 'Occupied/denied file causes native installer failure'
            Check (Equal $old.programs $now.programs) 'No partial or mixed-version program writes'
            Check (Equal $old.registry $now.registry) 'Preflight failure leaves registry unchanged'
            Check (Equal $old.shortcuts $now.shortcuts) 'Preflight failure leaves shortcuts unchanged'
            Check (Equal $old.data $now.data) 'Preflight failure preserves data'
            $op=Run-NSIS (Join-Path $install 'uninstall.exe') 'occupied-uninstall' $true;$now=Snapshot 'occupied-uninstall'
            Check ($op.exitCode -ne 0 -and !$op.timedOut) 'Occupied/denied file causes native uninstaller failure'
            Check (Equal $old.programs $now.programs) 'Uninstall failure preserves complete program group'
            if($Scenario -eq 'mcp-lock') {
                Check (!$target.process.HasExited -and !$nonTarget.process.HasExited) 'Neither MCP process was terminated'
                $result.nonTargetProof.after=@{targetPid=$target.process.Id;targetCreated=$target.process.StartTime.ToUniversalTime().Ticks;otherPid=$nonTarget.process.Id;otherCreated=$nonTarget.process.StartTime.ToUniversalTime().Ticks;otherSha256=(Hash $nonTarget.path)}
                $target.process.StandardInput.Close();Check ($target.process.WaitForExit(10000)) 'Target MCP exits via its own EOF' $true
            }
            foreach($stream in $streams) { $stream.Dispose() };$streams.Clear()
            if($Scenario -eq 'file-deny') { Restore-Access $denial }
            $null=Install-Candidate 'retry-after-occupation-cleared'
            if($Scenario -eq 'mcp-lock') { Check (!$nonTarget.process.HasExited) 'Other-directory same-name MCP survives successful retry';$nonTarget.process.StandardInput.Close();Check ($nonTarget.process.WaitForExit(10000)) 'Non-target fixture normal EOF cleanup' }
        }
    }
} catch {
    $result.error=$_.Exception.Message
    $checks.Add(@{name='Scenario completed without harness/setup exception';passed=$false})
} finally {
    foreach($stream in $streams) { $stream.Dispose() }
    foreach($entry in $restorations) { try { if(Test-Path -LiteralPath $entry.path) { Restore-Access $entry } } catch { $result.cleanupError='Test-specific ACL restoration failed' } }
    foreach($item in $owned) { try { Stop-Owned $item } catch { $result.cleanupError='Owned test process identity/cleanup could not be verified' } }
    try { $null=Snapshot 'final' } catch { $result.snapshotError=$_.Exception.Message }
    $result.findings=@($checks | Where-Object { !$_.passed } | ForEach-Object name)
    $result.passed=($result.findings.Count -eq 0 -and !$result.Contains('cleanupError'))
    $result | ConvertTo-Json -Depth 30 | Set-Content -LiteralPath (Join-Path $out 'results.json') -Encoding utf8
    Write-Output ("Native NSIS $Scenario : passed=$($result.passed), checks=$($checks.Count), findings=$($result.findings.Count). See structured artifact; no credential contents included.")
}
if(!$result.passed) { exit 1 }
exit 0
