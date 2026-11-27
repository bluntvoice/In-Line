param([switch]$Desktop,[switch]$Installed)
$ErrorActionPreference='Stop'
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted' -or $env:RUNNER_OS -ne 'Windows' -or $env:GITHUB_REPOSITORY -ne 'bluntvoice/In-Line' -or $env:GITHUB_REF -notin @('refs/heads/diagnostics/v050-f4-integration-20261127','refs/heads/diagnostics/v050-f4-installed-20261127','refs/heads/diagnostics/v050-f4-uia-20261127','refs/heads/diagnostics/v050-f4-medium-20261127')) { throw 'Unapproved F4 hosted identity' }
if (Get-Process -Name 'in-line','in-line-cli','in-line-mcp' -ErrorAction SilentlyContinue) { throw 'Existing In Line process' }
$paths=@((Join-Path $env:APPDATA 'in-line'),(Join-Path $env:USERPROFILE '.in-line'),(Join-Path $env:USERPROFILE '.codex/config.toml'))
if(!$Installed){$paths+=@('HKCU:\Software\github\In Line','HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\In Line')}
elseif($env:GITHUB_REF -notin @('refs/heads/diagnostics/v050-f4-installed-20261127','refs/heads/diagnostics/v050-f4-medium-20261127')){throw 'Installed desktop probe outside its harness branch'}
foreach($p in $paths){
  if(Test-Path -LiteralPath $p){throw 'In Line or Codex state already exists'}
}
$identity=[Security.Principal.WindowsIdentity]::GetCurrent()
$session=(Get-Process -Id $PID).SessionId
$drive=[IO.DriveInfo]::new([IO.Path]::GetPathRoot($env:RUNNER_TEMP))
if($drive.AvailableFreeSpace -lt 15GB){throw 'F4 runner disk admission failed'}
"F4_ADMISSION host=$env:RUNNER_ENVIRONMENT image=$env:ImageOS/$env:ImageVersion sid=$($identity.User.Value) session=$session freeGiB=$([Math]::Round($drive.AvailableFreeSpace/1GB,1)) profileClean=true"
if($Desktop){
  Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class F4Desktop {
 [DllImport("user32.dll",SetLastError=true)] public static extern IntPtr OpenInputDesktop(uint flags,bool inherit,uint access);
 [DllImport("user32.dll")] public static extern bool CloseDesktop(IntPtr desktop);
 [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
 [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr window);
 [DllImport("user32.dll")] public static extern uint GetDpiForSystem();
}
'@
  $inputDesktop=[F4Desktop]::OpenInputDesktop(0,$false,1)
  $foreground=[F4Desktop]::GetForegroundWindow()
  $windowDpi=if($foreground -ne [IntPtr]::Zero){[F4Desktop]::GetDpiForWindow($foreground)}else{0}
  $sameSessionShell=@(Get-Process explorer -ErrorAction SilentlyContinue | Where-Object SessionId -eq $session).Count
  $accessible=$session -ne 0 -and $inputDesktop -ne [IntPtr]::Zero -and $foreground -ne [IntPtr]::Zero -and $sameSessionShell -gt 0
  "F4_DESKTOP admission=$accessible interactive=$([Environment]::UserInteractive) desktop=$($inputDesktop -ne [IntPtr]::Zero) foreground=$($foreground -ne [IntPtr]::Zero) explorer=$sameSessionShell systemDpi=$([F4Desktop]::GetDpiForSystem()) windowDpi=$windowDpi"
  if($inputDesktop -ne [IntPtr]::Zero){[void][F4Desktop]::CloseDesktop($inputDesktop)}
}
