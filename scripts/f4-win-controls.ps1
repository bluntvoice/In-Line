param([ValidateSet('open','save','clipboard','window')][string]$Mode,[string]$Path,[int]$ProductPid,[int]$Width=0,[int]$Height=0)
$ErrorActionPreference='Stop'
$proc=Get-Process -Id $ProductPid
if($proc.ProcessName -ne 'in-line' -or $proc.SessionId -ne (Get-Process -Id $PID).SessionId -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted' -or $env:GITHUB_REPOSITORY -ne 'bluntvoice/In-Line'){throw 'Unexpected native GUI identity'}
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes, System.Windows.Forms
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class F4Win {
 [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
 [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr w,out uint pid);
 [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr window);
 [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr window);
 [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr w,IntPtr insertAfter,int x,int y,int cx,int cy,uint flags);
 [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr w);
}
'@
if($Mode -eq 'clipboard'){
   $content=[Windows.Forms.Clipboard]::GetText()
   # The independent runner clipboard contains only synthetic report data.
   [Console]::OutputEncoding=[Text.UTF8Encoding]::new($false)
   [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes($content))
   exit 0
}
if($Mode -eq 'window'){
  $w=$proc.MainWindowHandle
  if($w -eq [IntPtr]::Zero -or ![F4Win]::IsWindowVisible($w)){throw 'Native GUI main HWND is not visible'}
  if($Width -gt 0){if(![F4Win]::SetWindowPos($w,[IntPtr]::Zero,0,0,$Width,$Height,0x16)){throw 'Native resize failed'}}
  [void][F4Win]::SetForegroundWindow($w)
  $rect=[Windows.Automation.AutomationElement]::FromHandle($w).Current.BoundingRectangle
  @{pid=$ProductPid;dpi=[F4Win]::GetDpiForWindow($w);width=$rect.Width;height=$rect.Height;visible=$true}|ConvertTo-Json -Compress
  exit 0
}
$allowed=[IO.Path]::GetFullPath($env:F4_NATIVE_LAB)+[IO.Path]::DirectorySeparatorChar
if(![IO.Path]::GetFullPath($Path).StartsWith($allowed,[StringComparison]::OrdinalIgnoreCase)){throw 'Native picker path escapes test root'}
$deadline=[DateTime]::UtcNow.AddSeconds(14)
$dlg=$null
while([DateTime]::UtcNow -lt $deadline){
  $hwnd=[F4Win]::GetForegroundWindow();[uint32]$owner=0;[void][F4Win]::GetWindowThreadProcessId($hwnd,[ref]$owner)
  if($owner -eq $ProductPid -and $hwnd -ne $proc.MainWindowHandle){
    $candidate=[Windows.Automation.AutomationElement]::FromHandle($hwnd)
    if($candidate.Current.ClassName -eq '#32770'){$dlg=$candidate;break}
  }
  Start-Sleep -Milliseconds 120
}
if(!$dlg){throw "Native $Mode picker was not visible in foreground"}
$all=$dlg.FindAll([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.Condition]::TrueCondition)
$field=$all|Where-Object {$_.Current.ControlType -eq [Windows.Automation.ControlType]::Edit -and ($_.Current.AutomationId -in @('1001','1148','FileNameControl') -or $_.Current.Name -match 'File name|文件名') }|Select-Object -First 1
$ok=$all|Where-Object {$_.Current.ControlType -eq [Windows.Automation.ControlType]::Button -and ($_.Current.AutomationId -eq '1' -or $_.Current.Name -match '^(Open|Save|保存|打开)(\s.*)?$')}|Select-Object -First 1
if($field -and $ok){
  $field.GetCurrentPattern([Windows.Automation.ValuePattern]::Pattern).SetValue($Path)
  $ok.GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern).Invoke()
} else {
  [Windows.Forms.SendKeys]::SendWait('%n')
  Start-Sleep -Milliseconds 200
  $target=[Windows.Automation.AutomationElement]::FocusedElement
  [uint32]$focusedOwner=0
  [void][F4Win]::GetWindowThreadProcessId([F4Win]::GetForegroundWindow(),[ref]$focusedOwner)
  if($focusedOwner -ne $ProductPid -or $target.Current.ControlType -notin @([Windows.Automation.ControlType]::Edit,[Windows.Automation.ControlType]::ComboBox)){
    $known=@($all|ForEach-Object {$c=$_.Current;"$($c.ControlType.ProgrammaticName) [$($c.AutomationId)] $($c.Name)"}|Select-Object -First 30)
    throw "Native $Mode picker did not focus an owned filename control. Dialog=$($dlg.Current.Name); controls=$($known -join ' / ')"
  }
  [Windows.Forms.Clipboard]::SetText($Path)
  [Windows.Forms.SendKeys]::SendWait('^a')
  [Windows.Forms.SendKeys]::SendWait('^v')
  [Windows.Forms.SendKeys]::SendWait('{ENTER}')
}
@{mode=$Mode;owner=$owner;class='#32770'}|ConvertTo-Json -Compress
