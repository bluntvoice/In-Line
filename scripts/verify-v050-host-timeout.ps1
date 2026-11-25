param(
    [Parameter(Mandatory,Position=0)]
    [ValidateSet('Build','Fixtures','HostTest','Gated','Leaf')]
    [string]$Phase,
    [ValidateSet('Host','Sleep','Crash','Owner')]
    [string]$Scenario='Sleep',
    [string]$Token=''
)
$ErrorActionPreference='Stop'
[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false)
if($env:GITHUB_ACTIONS -cne 'true' -or $env:RUNNER_ENVIRONMENT -cne 'github-hosted' -or
   $env:RUNNER_OS -cne 'Windows' -or $env:GITHUB_REPOSITORY -ine 'bluntvoice/In-Line' -or
   $env:GITHUB_REF -cne 'refs/heads/diagnostics/v050-host-timeout' -or
   $env:GITHUB_EVENT_NAME -cne 'push' -or $env:GITHUB_RUN_ID -notmatch '^\d+$' -or $env:GITHUB_RUN_ATTEMPT -notmatch '^\d+$'){
    throw 'Diagnostic work must run on the authorized GitHub-hosted Windows job'
}
$repo = [IO.Path]::GetFullPath($env:GITHUB_WORKSPACE)
$runnerTemp = Get-Item -LiteralPath $env:RUNNER_TEMP
for($n=$runnerTemp; $n; $n=$n.Parent){
    if($n.Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'RUNNER_TEMP ancestry is redirected'}
}
$root = Join-Path $runnerTemp.FullName "inline-v050-diag-$env:GITHUB_RUN_ID-$env:GITHUB_RUN_ATTEMPT"
$paths = @{
    TEMP='temp'; TMP='temp'; APPDATA='appdata'; LOCALAPPDATA='localappdata'
    CARGO_HOME='cargo-home'; CARGO_TARGET_DIR='target'; npm_config_cache='npm-cache'
}
if($Phase -eq 'Build'){
    if(Test-Path -LiteralPath $root){throw 'Diagnostic root already exists'}
    [void](New-Item -ItemType Directory -Path $root)
    foreach($directory in @($paths.Values | Sort-Object -Unique)){
        [void](New-Item -ItemType Directory -Path (Join-Path $root $directory))
    }
}
if(!(Test-Path -LiteralPath $root -PathType Container)){throw 'Diagnostic root missing'}
foreach($name in $paths.Keys){[Environment]::SetEnvironmentVariable($name,(Join-Path $root $paths[$name]),'Process')}
Get-ChildItem Env: | Where-Object {$_.Name -like 'IN_LINE_*'} | ForEach-Object {
    [Environment]::SetEnvironmentVariable($_.Name,$null,'Process')
}
Set-Location -LiteralPath $repo
$manifest=Join-Path $root 'exe-inventory.json'

function Invoke-Checked([scriptblock]$Action){
    & $Action
    if($LASTEXITCODE -ne 0){throw "Command failed with exit $LASTEXITCODE"}
}

if($Phase -eq 'Build'){
    $identity=[Security.Principal.WindowsIdentity]::GetCurrent()
    $principal=[Security.Principal.WindowsPrincipal]::new($identity)
    $admin=$principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
    Write-Host "DIAG image=$env:ImageOS version=$env:ImageVersion Windows=$([Environment]::OSVersion.Version) user=$($identity.Name) sid=$($identity.User.Value) elevated=$admin session=$([Diagnostics.Process]::GetCurrentProcess().SessionId) pwsh=$($PSVersionTable.PSVersion)"
    if((git rev-parse HEAD) -cne $env:GITHUB_SHA){throw 'Checkout SHA mismatch'}
    $lock=(Get-FileHash src-tauri/Cargo.lock -Algorithm SHA256).Hash
    $installer=Join-Path ([Environment]::GetFolderPath('ProgramFilesX86')) 'Microsoft Visual Studio/Installer/vswhere.exe'
    $vs=& $installer -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    if($LASTEXITCODE -ne 0 -or !$vs){throw 'MSVC install not found'}
    & (Join-Path $vs 'Common7/Tools/Launch-VsDevShell.ps1') -Arch amd64 -HostArch amd64 -SkipAutomaticLocation
    foreach($tool in 'cl.exe','link.exe'){
        $p=Get-Command $tool -ErrorAction Stop
        Write-Host "DIAG $tool=$($p.Source) version=$((Get-Item -LiteralPath $p.Source).VersionInfo.FileVersion)"
    }
    $kernel=@($env:LIB -split ';' | Where-Object {$_ -and (Test-Path -LiteralPath (Join-Path $_ 'kernel32.lib'))})
    if(!$kernel.Count){throw 'SDK kernel32 import library not found'}
    Write-Host "DIAG SDK=$env:WindowsSDKVersion kernel32=$($kernel[0])"
    Invoke-Checked { rustc -vV }
    Invoke-Checked { cargo -V }
    Invoke-Checked { npm.cmd ci }
    Invoke-Checked { npm.cmd run build }
    Invoke-Checked { cargo fmt --manifest-path src-tauri/Cargo.toml -- --check }

    $artifacts=[Collections.Generic.List[object]]::new()
    cargo test --locked --manifest-path src-tauri/Cargo.toml --test mcp_host --no-run --message-format=json-render-diagnostics |
        ForEach-Object {
            if($_ -notmatch '^\{'){Write-Host $_;return}
            $r=ConvertFrom-Json -InputObject $_
            if($r.reason -eq 'compiler-message' -and $r.message.rendered){Write-Host $r.message.rendered}
            if($r.reason -eq 'compiler-artifact' -and $r.executable -and $r.target.name -in 'mcp_host','in-line','in-line-mcp'){
                $artifacts.Add($r)
            }
        }
    if($LASTEXITCODE -ne 0){throw "Target compilation failed: exit $LASTEXITCODE"}
    if((Get-FileHash src-tauri/Cargo.lock -Algorithm SHA256).Hash -cne $lock){throw 'Cargo.lock changed'}
    $inventory=[ordered]@{commit=$env:GITHUB_SHA; lock=$lock}
    foreach($name in 'mcp_host','in-line-mcp','in-line'){
        $match=@($artifacts | Where-Object {$_.target.name -ceq $name} | Select-Object -ExpandProperty executable -Unique)
        if($match.Count -ne 1){throw "Expected one Cargo artifact for $name, got $($match.Count)"}
        $file=Get-Item -LiteralPath $match[0]
        if(!$file.FullName.StartsWith($env:CARGO_TARGET_DIR+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)){
            throw 'Unexpected build output location'
        }
        $inventory[$name]=@{path=$file.FullName; sha=(Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash}
    }
    if((Join-Path (Split-Path $inventory['in-line-mcp'].path) 'in-line.exe') -ine $inventory['in-line'].path){
        throw 'Sidecar and host must be neighboring Cargo artifacts'
    }
    $json=$inventory | ConvertTo-Json -Depth 4 -Compress
    $json | Set-Content -LiteralPath $manifest -Encoding utf8NoBOM
    Write-Host "DIAG artifacts $json"
    exit 0
}

if($Phase -eq 'Leaf'){
    if([Console]::In.Read() -ne 71){exit 79}
    if($Token -notmatch '^[a-f0-9]{32}$'){throw 'Missing synthetic fixture token'}
    $staged=Join-Path $root "leaf-$Token.staged"
    [IO.File]::WriteAllText($staged,[Diagnostics.Process]::GetCurrentProcess().Id.ToString())
    [IO.File]::Move($staged,(Join-Path $root "leaf-$Token.ready"))
    Start-Sleep -Seconds 25
    exit 0
}

if($Phase -eq 'Gated'){
    if([Console]::In.Read() -ne 71){exit 79}
    if($Scenario -eq 'Host'){
        $inventory=Get-Content -LiteralPath $manifest -Raw | ConvertFrom-Json
        foreach($name in 'mcp_host','in-line-mcp','in-line'){
            $ent=$inventory.$name
            if($inventory.commit -cne $env:GITHUB_SHA -or (Get-FileHash -LiteralPath $ent.path -Algorithm SHA256).Hash -cne $ent.sha){
                throw "Candidate artifact mismatch: $name"
            }
        }
        & $inventory.mcp_host.path --exact real_host_background_single_instance_and_protocol --nocapture --test-threads=1
        exit $LASTEXITCODE
    }
}

Add-Type -TypeDefinition @'
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;
public sealed class InlineHostJob : IDisposable {
    [StructLayout(LayoutKind.Sequential)] struct Basic {
        public long ProcessTime, JobTime;
        public uint Flags;
        public UIntPtr Min, Max;
        public uint Active;
        public UIntPtr Affinity;
        public uint Priority, Schedule;
    }
    [StructLayout(LayoutKind.Sequential)] struct Extended {
        public Basic Basic;
        public ulong a,b,c,d,e,f;
        public UIntPtr g,h,i,j;
    }
    [DllImport("kernel32", SetLastError=true)] static extern IntPtr CreateJobObjectW(IntPtr a, IntPtr b);
    [DllImport("kernel32", SetLastError=true)] static extern bool SetInformationJobObject(IntPtr job,int kind,ref Extended value,uint size);
    [DllImport("kernel32", SetLastError=true)] static extern bool AssignProcessToJobObject(IntPtr job,IntPtr process);
    [DllImport("kernel32", SetLastError=true)] static extern bool QueryInformationJobObject(IntPtr job,int kind,IntPtr value,uint size,IntPtr len);
    [DllImport("kernel32", SetLastError=true)] static extern bool TerminateJobObject(IntPtr job,uint code);
    [DllImport("kernel32", SetLastError=true)] static extern bool IsProcessInJob(IntPtr process, IntPtr job, out bool result);
    [DllImport("kernel32")] static extern bool CloseHandle(IntPtr handle);
    [DllImport("user32")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32")] public static extern bool SetForegroundWindow(IntPtr window);
    [DllImport("user32")] public static extern bool IsWindowVisible(IntPtr window);
    [DllImport("user32", SetLastError=true)] static extern IntPtr OpenInputDesktop(uint flags,bool inherit,uint access);
    [DllImport("user32")] static extern bool CloseDesktop(IntPtr desktop);
    public static bool InputDesktopAvailable() {
        IntPtr desktop=OpenInputDesktop(0,false,1);
        return desktop!=IntPtr.Zero && CloseDesktop(desktop);
    }
    IntPtr handle;
    public InlineHostJob() {
        handle=CreateJobObjectW(IntPtr.Zero,IntPtr.Zero);
        if(handle==IntPtr.Zero) throw new Win32Exception();
        Extended config=new Extended(); config.Basic.Flags=0x2000;
        if(!SetInformationJobObject(handle,9,ref config,(uint)Marshal.SizeOf<Extended>())) {
            var err=new Win32Exception(); Dispose(); throw err;
        }
    }
    public int Assign(IntPtr process,bool reject) {
        return AssignProcessToJobObject(reject?IntPtr.Zero:handle,process)?0:Marshal.GetLastWin32Error();
    }
    public bool Contains(IntPtr process) {
        bool member; return IsProcessInJob(process,handle,out member) && member;
    }
    public int[] Pids() {
        const int capacity=8192;
        IntPtr buf=Marshal.AllocHGlobal(capacity);
        try {
            if(!QueryInformationJobObject(handle,3,buf,capacity,IntPtr.Zero)) throw new Win32Exception();
            int count=Marshal.ReadInt32(buf,4); int[] ids=new int[count];
            for(int n=0;n<count;n++) ids[n]=Marshal.ReadIntPtr(buf,8+n*IntPtr.Size).ToInt32();
            return ids;
        } finally { Marshal.FreeHGlobal(buf); }
    }
    public void Terminate() {
        if(Pids().Length>0 && !TerminateJobObject(handle,1)) throw new Win32Exception();
    }
    public void Dispose() {
        if(handle!=IntPtr.Zero){CloseHandle(handle);handle=IntPtr.Zero;}
    }
}
'@

function Start-Child([string]$ChildPhase,[string]$ChildScenario,[string]$ChildToken){
    $info=[Diagnostics.ProcessStartInfo]::new()
    $info.FileName=[Environment]::ProcessPath
    $info.UseShellExecute=$false
    $info.RedirectStandardInput=$true
    $info.StandardInputEncoding=[Text.UTF8Encoding]::new($false)
    foreach($arg in '-NoLogo','-NoProfile','-NonInteractive','-File',$PSCommandPath,$ChildPhase,'-Scenario',$ChildScenario,'-Token',$ChildToken){
        $info.ArgumentList.Add($arg)
    }
    return [Diagnostics.Process]::Start($info)
}
function Open-Gate($Process){
    $Process.StandardInput.Write([char]71)
    $Process.StandardInput.Close()
}
function Wait-Drain($Job){
    $clock=[Diagnostics.Stopwatch]::StartNew()
    do {
        if($Job.Pids().Length -eq 0){return}
        Start-Sleep -Milliseconds 50
    }while($clock.ElapsedMilliseconds -lt 5000)
    throw 'Job did not drain in 5s'
}
function Wait-Leaf([string]$LeafToken,[int]$Milliseconds=5000){
    $marker=Join-Path $root "leaf-$LeafToken.ready"
    $clock=[Diagnostics.Stopwatch]::StartNew()
    do {
        if(Test-Path -LiteralPath $marker){return [int][IO.File]::ReadAllText($marker)}
        Start-Sleep -Milliseconds 50
    }while($clock.ElapsedMilliseconds -lt $Milliseconds)
    throw 'Synthetic leaf did not reach its start marker'
}

if($Phase -eq 'Gated'){
    if($Token -notmatch '^[a-f0-9]{32}$'){throw 'Missing fixture token'}
    if($Scenario -eq 'Owner'){
        $nested=[InlineHostJob]::new()
        try{
            $leaf=Start-Child 'Leaf' 'Sleep' $Token
            try{
                if($nested.Assign($leaf.Handle,$false) -ne 0){$leaf.StandardInput.Close();throw 'Nested job assignment failed'}
                Open-Gate $leaf
                Wait-Leaf $Token | Out-Null
                [IO.File]::WriteAllText((Join-Path $root "owner-$Token.ready"),'bound')
                Start-Sleep -Seconds 25
            }finally{$leaf.Dispose()}
        }finally{$nested.Dispose()}
    }else{
        $leaf=Start-Child 'Leaf' 'Sleep' $Token
        try { Open-Gate $leaf; Wait-Leaf $Token | Out-Null } finally { $leaf.Dispose() }
        if($Scenario -eq 'Crash'){exit 23}
        Start-Sleep -Seconds 25
    }
    exit 0
}

function Test-Fixture([string]$Case){
    $fixture=[Guid]::NewGuid().ToString('N')
    $mode=switch($Case){'abnormal'{'Crash'} 'owner-kill'{'Owner'} default{'Sleep'}}
    $job=[InlineHostJob]::new()
    $process=$null
    try {
        $process=Start-Child 'Gated' $mode $fixture
        $deny=$Case -eq 'bind-denied'
        $bound=$job.Assign($process.Handle,$deny)
        if($deny){
            $process.StandardInput.Close()
            if($bound -eq 0 -or !$process.WaitForExit(7000) -or $process.ExitCode -ne 79 -or
               (Test-Path -LiteralPath (Join-Path $root "leaf-$fixture.ready"))){throw 'Bind denial did not prevent fixture start'}
            Write-Host "DIAG cleanup bind-denied PASS win32=$bound gate=closed"
            return
        }
        if($bound -ne 0){throw "Fixture Job assignment failed Win32=$bound"}
        Open-Gate $process
        $leafId=Wait-Leaf $fixture
        if($Case -ne 'abnormal'){
            $leaf=[Diagnostics.Process]::GetProcessById($leafId)
            try{if(!$job.Contains($leaf.Handle)){throw 'Leaf is outside outer Job'}}finally{$leaf.Dispose()}
        }
        switch($Case){
            'timeout' {
                if($process.WaitForExit(1000)){throw 'Sleeping worker unexpectedly exited'}
                $job.Terminate()
            }
            'abnormal' {
                if(!$process.WaitForExit(5000) -or $process.ExitCode -ne 23){throw 'Expected abnormal worker exit 23'}
                $leaf=[Diagnostics.Process]::GetProcessById($leafId)
                try{if(!$job.Contains($leaf.Handle)){throw 'Abnormal exit leaf is outside Job'}}finally{$leaf.Dispose()}
                $job.Terminate()
            }
            'owner-kill' {
                $ready=Join-Path $root "owner-$fixture.ready"
                $wait=[Diagnostics.Stopwatch]::StartNew()
                while(!(Test-Path -LiteralPath $ready) -and $wait.ElapsedMilliseconds -lt 5000){Start-Sleep -Milliseconds 50}
                if(!(Test-Path -LiteralPath $ready)){throw 'Nested watchdog never became ready'}
                $process.Kill()
                # Observe nested kill-on-close before terminating the outer Job.
            }
        }
        Wait-Drain $job
        if(!$process.WaitForExit(5000)){throw 'Fixture worker did not exit'}
        Write-Host "DIAG cleanup $Case PASS active=0"
    }finally{
        if($process){$process.Dispose()}
        $job.Dispose()
    }
}

if($Phase -eq 'Fixtures'){
    foreach($case in 'timeout','abnormal','bind-denied','owner-kill'){Test-Fixture $case}
    exit 0
}

$identity=[Security.Principal.WindowsIdentity]::GetCurrent()
$session=[Diagnostics.Process]::GetCurrentProcess().SessionId
if($session -eq 0 -or ![Environment]::UserInteractive -or ![InlineHostJob]::InputDesktopAvailable()){
    throw "BLOCKED no meaningful interactive input desktop (session $session)"
}
$webviewRoot=Join-Path ([Environment]::GetFolderPath('ProgramFilesX86')) 'Microsoft/EdgeWebView/Application'
$webview=@(Get-ChildItem -LiteralPath $webviewRoot -Filter msedgewebview2.exe -Recurse -File -ErrorAction SilentlyContinue)
if(!$webview.Count){throw 'BLOCKED WebView2 installation unavailable'}
Write-Host "DIAG desktop session=$session user=$($identity.Name) WebView2=$($webview[0].VersionInfo.FileVersion)"
if(@(Get-Process -Name in-line,in-line-mcp -ErrorAction SilentlyContinue).Count){throw 'BLOCKED existing In-Line instance on the runner'}
Add-Type -AssemblyName System.Windows.Forms
$form=[System.Windows.Forms.Form]::new()
$form.Text='In-Line diagnostic foreground witness'
$form.Width=360
$form.Height=110
$form.Show()
$form.Activate()
[void][InlineHostJob]::SetForegroundWindow($form.Handle)
[System.Windows.Forms.Application]::DoEvents()
$job=$null
$worker=$null
try{
    if([InlineHostJob]::GetForegroundWindow() -ne $form.Handle -or ![InlineHostJob]::IsWindowVisible($form.Handle)){
        throw 'BLOCKED no independent visible foreground witness'
    }
    $inventory=Get-Content -LiteralPath $manifest -Raw | ConvertFrom-Json
    $job=[InlineHostJob]::new()
    $worker=Start-Child 'Gated' 'Host' 'none'
    $bound=$job.Assign($worker.Handle,$false)
    if($bound -ne 0){
        $worker.StandardInput.Close()
        [void]$worker.WaitForExit(5000)
        throw "Host watchdog could not assign the gated worker Win32=$bound"
    }
    $clock=[Diagnostics.Stopwatch]::StartNew()
    $hostSeen=$false
    $sidecarSeen=$false
    $webviewSeen=$false
    $samples=0
    $withWitness=0
    $seen=[Collections.Generic.HashSet[int]]::new()
    Open-Gate $worker
    while(!$worker.WaitForExit(75)){
        [System.Windows.Forms.Application]::DoEvents()
        $samples++
        if([InlineHostJob]::GetForegroundWindow() -eq $form.Handle){$withWitness++}
        foreach($member in $job.Pids()){
            if($seen.Contains($member)){continue}
            try{$current=[Diagnostics.Process]::GetProcessById($member)}catch [ArgumentException]{continue}
            try{
                if(!$job.Contains($current.Handle)){continue}
                $path=$current.Path
                [void]$seen.Add($member)
                if($path -ieq $inventory.'in-line'.path){$hostSeen=$true;Write-Host "DIAG observed host pid=$member in Job"}
                if($path -ieq $inventory.'in-line-mcp'.path){$sidecarSeen=$true;Write-Host "DIAG observed sidecar pid=$member in Job"}
                if($current.ProcessName -ieq 'msedgewebview2'){$webviewSeen=$true;Write-Host "DIAG observed WebView2 pid=$member in Job"}
            }catch [InvalidOperationException]{}finally{$current.Dispose()}
        }
        if($clock.Elapsed.TotalSeconds -ge 150){
            $job.Terminate()
            Wait-Drain $job
            throw 'Host test exceeded external 150-second deadline; outer Job drained'
        }
    }
    $elapsed=[Math]::Round($clock.Elapsed.TotalSeconds,3)
    $remaining=$job.Pids().Length
    Write-Host "DIAG host result exit=$($worker.ExitCode) seconds=$elapsed active=$remaining focusSamples=$withWitness/$samples host=$hostSeen sidecar=$sidecarSeen webview=$webviewSeen"
    if($remaining){$job.Terminate();Wait-Drain $job;throw 'Host test leaked processes into outer Job'}
    if($worker.ExitCode -ne 0){throw 'Host test did not pass'}
    if(!$hostSeen -or !$sidecarSeen -or !$webviewSeen -or !$withWitness){throw 'BLOCKED required host/sidecar/WebView2/foreground observations incomplete'}
    if([InlineHostJob]::GetForegroundWindow() -ne $form.Handle){throw 'Foreground witness was lost after host test'}
}finally{
    if($job){$job.Dispose()}
    if($worker){$worker.Dispose()}
    $form.Dispose()
}
