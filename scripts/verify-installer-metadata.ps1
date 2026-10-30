[CmdletBinding()]
param([string]$OutputDirectory='release/installer-metadata-tests')
$ErrorActionPreference='Stop'
if($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted' -or $env:RUNNER_OS -ne 'Windows' -or $env:GITHUB_REPOSITORY -ne 'bluntvoice/In-Line') { throw 'Metadata fixtures require this repository on fresh GitHub-hosted Windows only.' }
$root=[IO.Path]::GetFullPath((Join-Path $env:RUNNER_TEMP ('inline-metadata-'+[guid]::NewGuid().ToString('N'))))
$out=[IO.Path]::GetFullPath($OutputDirectory);New-Item -ItemType Directory -Path $root,$out | Out-Null
$arp='Software\Microsoft\Windows\CurrentVersion\Uninstall\In Line';$vendor='Software\github\In Line'
$desktop=Join-Path ([Environment]::GetFolderPath('DesktopDirectory')) 'In Line.lnk'
$menu=Join-Path ([Environment]::GetFolderPath('Programs')) 'In Line.lnk'
$checks=[Collections.Generic.List[object]]::new()
function Check($condition,$name) { $checks.Add(@{name=$name;passed=[bool]$condition});if(!$condition) { throw $name } }
function Hash($p) { (Get-FileHash -LiteralPath $p).Hash }
function Same($a,$b) { ($a | ConvertTo-Json -Depth 10 -Compress) -ceq ($b | ConvertTo-Json -Depth 10 -Compress) }
function Snapshot {
    $files=[ordered]@{};foreach($p in @($desktop,$menu)+@([InLine.Installer.Guard]::Names | ForEach-Object { Join-Path $install $_ })) { $files[$p]=if(Test-Path -LiteralPath $p -PathType Leaf) { Hash $p } else { $null } }
    $reg=[ordered]@{};foreach($path in @($arp,$vendor)) {
        $key=[Microsoft.Win32.Registry]::CurrentUser.OpenSubKey($path);$v=[ordered]@{}
        if($key) { try { foreach($name in @($key.GetValueNames() | Sort-Object)) { $v[$name]=$key.GetValue($name) } } finally { $key.Dispose() } }
        $reg[$path]=@{exists=($null -ne $key);values=$v}
    };return @{files=$files;registry=$reg}
}
try {
    Check (!(Test-Path "Registry::HKEY_CURRENT_USER\$arp") -and !(Test-Path "Registry::HKEY_CURRENT_USER\$vendor") -and !(Test-Path $desktop) -and !(Test-Path $menu)) 'Fresh hosted account has no product registration or shortcuts'
    $repo=Split-Path $PSScriptRoot -Parent
    Add-Type -Path "$repo/src-tauri/windows/InstallGuard.cs","$repo/src-tauri/windows/InstallMetadata.cs" -ReferencedAssemblies 'System.dll','System.Core.dll','System.Xml.dll'
    $install=Join-Path $root 'install';$payload=Join-Path $root 'payload';New-Item -ItemType Directory -Path $install,$payload | Out-Null
    foreach($name in [InLine.Installer.Guard]::Names) { [IO.File]::WriteAllText((Join-Path $payload $name),('synthetic-'+$name)) }
    $memory=[Microsoft.Win32.Registry]::CurrentUser.CreateSubKey($vendor)
    try { $memory.SetValue('',(Join-Path $root 'previous-directory'));$memory.SetValue('SyntheticPreference','retain') } finally { $memory.Dispose() }
    $options=[InLine.Installer.MetadataOptions]::new();$options.Version='0.5.0';$options.Publisher='github';$options.BundleId='com.inline.app';$options.Hive='CurrentUser';$options.Desktop=Split-Path $desktop;$options.Programs=Split-Path $menu;$options.StartMenuFolder='';$options.StartMenu=$true;$options.DesktopShortcut=$true;$options.RemoveShortcuts=$true
    $before=Snapshot
    $r=[InLine.Installer.Guard]::Run($install,$payload,'install',-1,$options);Check ($r.Code -eq 0) ('Prepared metadata installation: '+$r.Message)
    $key=[Microsoft.Win32.Registry]::CurrentUser.OpenSubKey($vendor);try { Check ($key.GetValue('') -eq $install) 'Successful installation updates remembered directory' } finally { $key.Dispose() }
    $after=Snapshot;$saved=[IO.File]::ReadAllBytes($desktop)
    $shell=New-Object -ComObject WScript.Shell;$link=$shell.CreateShortcut($desktop);$link.TargetPath=Join-Path $root 'synthetic-concurrent-target.exe';$link.Save();$foreign=Hash $desktop
    $r=[InLine.Installer.Guard]::Run($install,$payload,'rollback',-1,$options)
    Check ($r.Code -ne 0 -and (Hash $desktop) -eq $foreign) 'Rollback refuses to overwrite a concurrent foreign shortcut'
    Check (Test-Path (Join-Path $install '.in-line-install-transaction/journal.xml')) 'Incomplete recovery keeps journal and startup barrier'
    # Explicit external fixture reset, not product recovery; retain the refusal assertion.
    Check ((Hash $desktop) -eq $foreign) 'Fixture reset checks exact foreign bytes';[IO.File]::WriteAllBytes($desktop,$saved)
    $r=[InLine.Installer.Guard]::Run($install,$payload,'rollback',-1,$options);Check ($r.Code -eq 0) ('Explicit install rollback: '+$r.Message)
    Check (Same $before (Snapshot)) 'Install rollback restores absent links, exact registry and previous remembered directory'
    $r=[InLine.Installer.Guard]::Run($install,$payload,'install',-1,$options);Check ($r.Code -eq 0) 'Second metadata install'
    $r=[InLine.Installer.Guard]::Run($install,$payload,'finalize',-1,$options);Check ($r.Code -eq 0) 'Metadata finalize verifies registry and shortcuts'
    $installed=Snapshot
    $r=[InLine.Installer.Guard]::Run($install,$payload,'uninstall',-1,$options);Check ($r.Code -eq 0) 'Prepared metadata uninstall'
    $link=$shell.CreateShortcut($desktop);$link.TargetPath=Join-Path $root 'synthetic-concurrent-target.exe';$link.Save();$foreign=Hash $desktop
    $r=[InLine.Installer.Guard]::Run($install,$payload,'rollback',-1,$options);Check ($r.Code -ne 0 -and (Hash $desktop) -eq $foreign) 'Uninstall rollback refuses concurrent shortcut replacement'
    Check (Test-Path (Join-Path $install '.in-line-install-transaction/journal.xml')) 'Uninstall recovery refusal retains evidence and barrier'
    Check ((Hash $desktop) -eq $foreign) 'Deletion fixture reset verifies exact test bytes';Remove-Item -LiteralPath $desktop
    $r=[InLine.Installer.Guard]::Run($install,$payload,'rollback',-1,$options);Check ($r.Code -eq 0) ('Explicit uninstall rollback: '+$r.Message)
    Check (Same $installed (Snapshot)) 'Uninstall rollback recreates exact link bytes, file group and product registry'
    $link=$shell.CreateShortcut($desktop);$link.TargetPath=Join-Path $root 'synthetic-optional-foreign-target.exe';$link.Save();$foreign=Hash $desktop
    $r=[InLine.Installer.Guard]::OptionalDesktop($install,$options);Check ($r.Code -ne 0 -and (Hash $desktop) -eq $foreign) 'Optional finish-page action reports failure and preserves foreign shortcut'
} catch { $checks.Add(@{name=$_.Exception.Message;passed=$false}) }
finally { @{checks=$checks;passed=(@($checks | Where-Object { !$_.passed }).Count -eq 0);syntheticDataOnly=$true;nativeNSIS=$false} | ConvertTo-Json -Depth 8 | Set-Content (Join-Path $out 'results.json') }
if(@($checks | Where-Object { !$_.passed }).Count) { exit 1 }
Write-Output "PASS: $($checks.Count) hosted metadata recovery checks; native lifecycle is validated separately."
