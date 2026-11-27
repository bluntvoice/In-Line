param([Parameter(Mandatory)][string]$Exe,[Parameter(Mandatory)][string]$Output)
$ErrorActionPreference='Stop'
if($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted' -or $env:RUNNER_OS -ne 'Windows' -or $env:GITHUB_REPOSITORY -ne 'bluntvoice/In-Line' -or $env:GITHUB_REF -ne 'refs/heads/diagnostics/v050-f4-medium-20261127'){throw 'Restricted GUI launch is only for this hosted lab'}
# Restrict the current runner token for one child. No machine or user policies change.
Add-Type -TypeDefinition @'
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;
using System.Text;
public static class F4RestrictedLaunch {
 [StructLayout(LayoutKind.Sequential)] struct Label {public IntPtr sid; public int attr;}
 [StructLayout(LayoutKind.Sequential, CharSet=CharSet.Unicode)] struct Startup {
  public int cb; public string res,desktop,title; public int x,y,cx,cy,xc,yc,fill,flags; public ushort show,bytes; public IntPtr extra,stdin,stdout,stderr;
 }
 [StructLayout(LayoutKind.Sequential)] struct Child {public IntPtr process,thread; public uint pid,tid;}
 [DllImport("kernel32.dll")] static extern IntPtr GetCurrentProcess();
 [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr handle);
 [DllImport("kernel32.dll")] static extern IntPtr LocalFree(IntPtr ptr);
 [DllImport("kernel32.dll",SetLastError=true)] static extern uint WaitForSingleObject(IntPtr p,uint milliseconds);
 [DllImport("kernel32.dll",SetLastError=true)] static extern bool GetExitCodeProcess(IntPtr p,out uint code);
 [DllImport("advapi32.dll",SetLastError=true)] static extern bool OpenProcessToken(IntPtr p,uint access,out IntPtr t);
 [DllImport("advapi32.dll",SetLastError=true)] static extern bool CreateRestrictedToken(IntPtr original,uint flags,uint disabled,IntPtr sids,uint removed,IntPtr privs,uint count,IntPtr restricted,out IntPtr token);
 [DllImport("advapi32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern bool ConvertStringSidToSid(string value,out IntPtr sid);
 [DllImport("advapi32.dll")] static extern uint GetLengthSid(IntPtr sid);
 [DllImport("advapi32.dll",SetLastError=true)] static extern bool SetTokenInformation(IntPtr token,int information,ref Label label,uint length);
 [DllImport("advapi32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern bool CreateProcessAsUser(IntPtr token,string executable,StringBuilder line,IntPtr sa,IntPtr ta,bool inherit,uint options,IntPtr env,string root,ref Startup startup,out Child process);
 static void Check(bool ok,string operation){if(!ok)throw new Win32Exception(Marshal.GetLastWin32Error(),operation);}
 public static uint Run(string host,string line,string root){
  IntPtr original=IntPtr.Zero,restricted=IntPtr.Zero,sid=IntPtr.Zero;
  Child child=new Child();
  try{
   Check(OpenProcessToken(GetCurrentProcess(),0x018B,out original),"OpenProcessToken");
   Check(CreateRestrictedToken(original,5,0,IntPtr.Zero,0,IntPtr.Zero,0,IntPtr.Zero,out restricted),"CreateRestrictedToken");
   Check(ConvertStringSidToSid("S-1-16-8192",out sid),"Medium integrity SID");
   Label label=new Label{sid=sid,attr=0x20};
   Check(SetTokenInformation(restricted,25,ref label,(uint)Marshal.SizeOf<Label>()+GetLengthSid(sid)),"Set child integrity level");
   Startup si=new Startup();si.cb=Marshal.SizeOf<Startup>();si.desktop="winsta0\\default";
   Check(CreateProcessAsUser(restricted,host,new StringBuilder(line),IntPtr.Zero,IntPtr.Zero,false,0,IntPtr.Zero,root,ref si,out child),"CreateProcessAsUser");
   uint waited=WaitForSingleObject(child.process,240000);
   if(waited==258){
    try{using(var owned=System.Diagnostics.Process.GetProcessById((int)child.pid)){owned.Kill(true);}}catch(ArgumentException){}
    throw new TimeoutException("Restricted GUI process exceeded 240 seconds");
   }
   Check(waited==0,"WaitForSingleObject");
   uint exit;Check(GetExitCodeProcess(child.process,out exit),"GetExitCodeProcess");
   return exit;
  }finally{
   if(child.thread!=IntPtr.Zero)CloseHandle(child.thread);
   if(child.process!=IntPtr.Zero)CloseHandle(child.process);
   if(sid!=IntPtr.Zero)LocalFree(sid);
   if(restricted!=IntPtr.Zero)CloseHandle(restricted);
   if(original!=IntPtr.Zero)CloseHandle(original);
  }
 }
}
'@
$stage=Join-Path $env:RUNNER_TEMP ('f4-standard-'+[guid]::NewGuid().ToString('N'))
[void][IO.Directory]::CreateDirectory($stage)
foreach($f in 'f4-native-ui.ps1','f4-native-ui.mjs','f4-hosted-admission.ps1','f4-win-controls.ps1'){Copy-Item -LiteralPath (Join-Path $PSScriptRoot $f) -Destination $stage}
$program=(Get-Process -Id $PID).Path
$runner=Join-Path $stage 'Run.ps1'
$log=Join-Path $Output 'medium-child.log'
@"
`$ErrorActionPreference='Stop'
try {
  if(!(whoami /groups | Select-String 'S-1-16-8192')){throw 'Child mandatory label is not Medium'}
  'Measured child process integrity=Medium' | Set-Content -LiteralPath '$log'
  & '$stage/f4-native-ui.ps1' -Exe '$Exe' -Output '$Output' *>> '$log'
  exit `$LASTEXITCODE
} catch {
  `$_.Exception.Message | Add-Content -LiteralPath '$log'
  exit 1
}
"@ | Set-Content -LiteralPath $runner -Encoding utf8
$line='"'+$program+'" -NoProfile -STA -File "'+$runner+'"'
try{$code=[F4RestrictedLaunch]::Run($program,$line,$stage)}
finally{if(Test-Path -LiteralPath $log){Get-Content -LiteralPath $log}}
exit $code
