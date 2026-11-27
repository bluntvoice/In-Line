param([Parameter(Mandatory)][string]$Exe,[Parameter(Mandatory)][string]$Output)
$ErrorActionPreference='Stop'
if($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted' -or $env:RUNNER_OS -ne 'Windows' -or $env:GITHUB_REPOSITORY -ne 'bluntvoice/In-Line' -or $env:GITHUB_REF -ne 'refs/heads/diagnostics/v050-f4-uia-20261127'){throw 'Native UIA requires the pinned hosted workflow'}
Add-Type -AssemblyName UIAutomationClient,UIAutomationTypes,System.Windows.Forms,System.Drawing
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class InlineUiWindow {
 [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
 [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hwnd,out uint pid);
 [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hwnd);
 [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hwnd);
 [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr hwnd);
 [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr hwnd,IntPtr after,int x,int y,int w,int h,uint flags);
 delegate bool Enumerate(IntPtr hwnd, IntPtr param);
 [DllImport("user32.dll")] static extern bool EnumWindows(Enumerate callback, IntPtr param);
 [DllImport("user32.dll", CharSet=CharSet.Unicode)] static extern int GetWindowText(IntPtr hwnd, StringBuilder name, int size);
 public static IntPtr ProductWindow(uint expectedPid, string expectedTitle) {
  IntPtr found=IntPtr.Zero;
  EnumWindows((hwnd, unused)=>{
   uint owner; GetWindowThreadProcessId(hwnd,out owner);
   if(owner!=expectedPid || !IsWindowVisible(hwnd))return true;
   var name=new StringBuilder(256); GetWindowText(hwnd,name,name.Capacity);
   if(!String.Equals(name.ToString(),expectedTitle,StringComparison.Ordinal))return true;
   found=hwnd;return false;
  },IntPtr.Zero);
  return found;
 }
}
'@
$lab=Join-Path $env:RUNNER_TEMP ("inline-f4-uia-"+[guid]::NewGuid().ToString('N'))
[void][IO.Directory]::CreateDirectory($lab)
$env:F4_NATIVE_LAB=$lab
$db=Join-Path $env:APPDATA 'in-line/inline.db'
$info=[Diagnostics.ProcessStartInfo]::new($Exe)
$info.UseShellExecute=$false
$info.RedirectStandardError=$true
$info.RedirectStandardOutput=$true
foreach($k in @($info.Environment.Keys)){if($k -match 'TOKEN|SECRET|PASSWORD|CREDENTIAL|^IN_LINE_MCP_'){[void]$info.Environment.Remove($k)}}
$info.Environment['TEMP']=$lab
$info.Environment['TMP']=$lab
$pidSession=(Get-Process -Id $PID).SessionId
$process=[Diagnostics.Process]::Start($info)
$ticks=$process.StartTime.ToUniversalTime().Ticks
$stdout=$process.StandardOutput.ReadToEndAsync()
$stderr=$process.StandardError.ReadToEndAsync()
$records=[Collections.Generic.List[object]]::new()
$window=[IntPtr]::Zero
function Children {
  $root=[Windows.Automation.AutomationElement]::FromHandle($window)
  $root.FindAll([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.Condition]::TrueCondition)
}
function Get-Control([string]$pattern,[string]$type='',[int]$timeout=8) {
  $until=[DateTime]::UtcNow.AddSeconds($timeout)
  while([DateTime]::UtcNow -lt $until) {
    foreach($node in (Children)){
      try {
        $summary=$node.Current
        if($summary.Name -match $pattern -and (!$type -or $summary.ControlType.ProgrammaticName -eq "ControlType.$type") -and $summary.IsEnabled){return $node}
      } catch [Windows.Automation.ElementNotAvailableException] {}
    }
    Start-Sleep -Milliseconds 160
  }
  throw "UIA control unavailable ($type): $pattern"
}
function Click([string]$pattern,[string]$type='Button'){
  $node=Get-Control $pattern $type
  $inv=$null
  if($node.TryGetCurrentPattern([Windows.Automation.InvokePattern]::Pattern,[ref]$inv)){$inv.Invoke();return}
  $toggle=$null
  if($node.TryGetCurrentPattern([Windows.Automation.TogglePattern]::Pattern,[ref]$toggle)){$toggle.Toggle();return}
  Set-InputFocus $node
  [Windows.Forms.SendKeys]::SendWait('{ENTER}')
}
function Set-InputFocus($node) {
  [void][InlineUiWindow]::SetForegroundWindow($window)
  [uint32]$owner=0
  [void][InlineUiWindow]::GetWindowThreadProcessId([InlineUiWindow]::GetForegroundWindow(),[ref]$owner)
  if($owner -ne $process.Id){throw 'Input focus is outside verified In Line PID'}
  $node.SetFocus()
}
function Fill([string]$pattern,[string]$value,[bool]$commit=$false){
  $node=Get-Control $pattern 'Edit'
  Set-InputFocus $node
  [Windows.Forms.Clipboard]::SetText($value)
  [Windows.Forms.SendKeys]::SendWait('^a^v')
  if($commit){[Windows.Forms.SendKeys]::SendWait('{ENTER}')}
}
function Screen([string]$label){
  [void][InlineUiWindow]::SetForegroundWindow($window)
  $r=[Windows.Automation.AutomationElement]::FromHandle($window).Current.BoundingRectangle
  $b=[Drawing.Bitmap]::new([int]$r.Width,[int]$r.Height)
  $g=[Drawing.Graphics]::FromImage($b)
  try{
    [uint32]$owner=0
    [void][InlineUiWindow]::GetWindowThreadProcessId([InlineUiWindow]::GetForegroundWindow(),[ref]$owner)
    if($owner -ne $process.Id){throw 'Screenshot target is outside product PID'}
    $g.CopyFromScreen([int]$r.X,[int]$r.Y,0,0,$b.Size)
    $b.Save((Join-Path $Output "$label.png"),[Drawing.Imaging.ImageFormat]::Png)
  } finally {$g.Dispose();$b.Dispose()}
}
function Step([string]$name,[scriptblock]$body){
  try {$value=& $body;$records.Add(@{name=$name;status='PASS';observed=$value});"F4_UIA PASS $name"}
  catch {
    $records.Add(@{name=$name;status='FAIL';error=$_.Exception.Message})
    "F4_UIA FAIL $name $($_.Exception.Message)"
    try {Screen "$name-failure"} catch {}
    if(!(Test-Path -LiteralPath "$Output/$name-tree.json")){
      $items=foreach($n in (Children)){try {$x=$n.Current;if($x.Name){@{name=$x.Name;type=$x.ControlType.ProgrammaticName;visible=!$x.IsOffscreen}} } catch {}}
      @($items|Select-Object -First 240)|ConvertTo-Json -Depth 5|Set-Content "$Output/$name-tree.json" -Encoding utf8
    }
  }
  $records|ConvertTo-Json -Depth 12 | Set-Content "$Output/uia-evidence.json" -Encoding utf8
}
function Dialog([string]$mode,[string]$path){
  & (Join-Path $PSScriptRoot 'f4-win-controls.ps1') -Mode $mode -Path $path -ProductPid $process.Id
  if($LASTEXITCODE){throw "Native common dialog $mode failed"}
}
function Verify-Db([string]$mode){
  & python (Join-Path $PSScriptRoot 'f4_uia_db.py') $db $mode
  if($LASTEXITCODE){throw "SQLite $mode failed"}
}
function New-Task([string]$title,[string]$note=''){
  Click '^新增取号'
  Fill '^(输入或选择部门|部门 / 团队)' 'F4 UIA Team' $true
  Fill '^(输入或选择对接人|对接人)' 'F4 UIA Contact' $true
  Fill '^(输入或选择事项类型|事项类型)' 'F4 UIA Type'
  Fill '^(一句话说明|事项标题)' $title
  if($note){Fill '^(记录判断|内部备注)' $note}
  Click '^保存并取号$'
  [void](Get-Control ('^'+[regex]::Escape($title)+'$'))
}
try {
  $until=[DateTime]::UtcNow.AddSeconds(35)
  while([DateTime]::UtcNow -lt $until -and !$process.HasExited){
    $window=[InlineUiWindow]::ProductWindow([uint32]$process.Id,'In Line｜排着呢')
    if($window -ne [IntPtr]::Zero -and [InlineUiWindow]::IsWindowVisible($window)){break}
    Start-Sleep -Milliseconds 250
  }
  if($window -eq [IntPtr]::Zero -or !$pidSession -or $process.SessionId -ne $pidSession){throw 'Installed GUI main HWND not admitted'}
  $dpi=[InlineUiWindow]::GetDpiForWindow($window)
  if($dpi -ne 96){throw "Initial runner DPI is $dpi, expected 96"}
  "F4_UIA hwnd=$window pid=$($process.Id) session=$pidSession dpi=$dpi sha256=$((Get-FileHash -LiteralPath $Exe).Hash)"
  $productRoot=[Windows.Automation.AutomationElement]::FromHandle($window)
  "F4_UIA selected product title=$($productRoot.Current.Name) descendants=$(@(Children).Count)"
  try{Screen 'first-visible-window'}catch{"First native screenshot unavailable: $($_.Exception.Message)"}
  Step 'about-and-native-window' {
    Click '^关于$'
    [void](Get-Control '\b0\.5\.0\b')
    Screen 'about'
    Click '^待办队列$'
    @{dpi=$dpi;actualHwnd=$true;about='0.5.0'}
  }
  Step 'three-real-restore-rounds' {
    New-Task 'F4 source' 'original source note'
    Click '^软件设置$';Click '^立即备份$'
    $backupDir=Join-Path (Split-Path $db) 'backups'
    $backup=$null
    $until=[DateTime]::UtcNow.AddSeconds(10)
    while(!$backup -and [DateTime]::UtcNow -lt $until){
      $backup=Get-ChildItem -LiteralPath $backupDir -Filter '*-manual.db' -File -ErrorAction SilentlyContinue | Select-Object -First 1
      if(!$backup){Start-Sleep -Milliseconds 200}
    }
    if(!$backup){throw 'GUI did not create backup'}
    $source=Join-Path $lab 'import-source.db'
    Copy-Item -LiteralPath $backup.FullName -Destination $source
    $sourceHash=(Get-FileHash -LiteralPath $source).Hash
    (Get-Item -LiteralPath $source).LastWriteTimeUtc=[DateTime]::new(2024,1,1)
    for($i=1;$i -le 13;$i++){Copy-Item -LiteralPath $source -Destination (Join-Path $backupDir ('InLine-backup-20261001-0101{0:00}-manual.db' -f $i))}
    Click '^待办队列$';Click '^F4 source$' ''
    Click '^编辑$';Fill '^(记录判断|内部备注)' 'local edited note';Click '^保存修改$'
    try{Click '^关闭$'}catch{}
    New-Task 'F4 source（冲突）'
    Click '^软件设置$'
    $expected=@('新增 1 项，合并 0 项，冲突保留 1 项','新增 0 项，合并 1 项，冲突保留 0 项','新增 0 项，合并 1 项，冲突保留 0 项')
    for($i=0;$i -lt 3;$i++){
      Click '^导入并恢复$'
      Dialog 'open' $source
      Click '^(确定|OK)$'
      [void](Get-Control $expected[$i])
      if($i -eq 0){[void](Get-Control 'F4 source（冲突 2）');Click '^知道了，稍后处理$'}
      [void](Get-Control '刚刚导入')
      Verify-Db 'restore'
      if((Get-FileHash -LiteralPath $source).Hash -ne $sourceHash){throw 'Source bytes changed'}
      Screen ("restore-"+($i+1))
      if($i -eq 2){Click '^查看待办$'}
    }
    [void](Get-Control '^F4 source（冲突 2）$')
    @{sourceSha256=$sourceHash;rounds=$expected}
  }
  Step 'timeline-keyboard-and-real-stats' {
    Click '^待办队列$'
    New-Task 'F4 processed'
    Click '^F4 processed$' ''
    Click '^本轮已处理$'
    [void](Get-Control '事项时间线')
    Fill '^补充一条普通处理备注' 'F4 UIA memo'
    Click '^添加$'
    [void](Get-Control 'F4 UIA memo')
    $toggle=Get-Control '原始记录' 'Button'
    Set-InputFocus $toggle
    [Windows.Forms.SendKeys]::SendWait('{ENTER}')
    Screen 'timeline'
    Click '^关闭$'
    Click '^统计中心$'
    foreach($name in '本周','上周','本月','上月','本季度','上季度','自定义'){Click "^$name$"}
    Click '^本周$'
    [void](Get-Control '工作量趋势')
    if(![InlineUiWindow]::SetWindowPos($window,[IntPtr]::Zero,0,0,900,720,0x16)){throw 'Native narrow window resize failed'}
    Screen 'statistics-native-narrow'
    @{dpi=[InlineUiWindow]::GetDpiForWindow($window);nativeResize=$true}
  }
  Step 'real-clipboard-report' {
    Click '^统计中心$';Click '^生成报告$'
    Click '原生工作报告';Click '^预览报告$'
    Click '^复制报告$'
    $text=[Windows.Forms.Clipboard]::GetText()
    if($text.Length -lt 100 -or $text -notmatch '工作报告'){throw 'System clipboard does not contain the expected synthetic native report'}
    Screen 'native-report'
    Click '^关闭$'
    @{characters=$text.Length;clipboard='Windows STA readback'}
  }
  Step 'real-png-save' {
    Click '^统计中心$';Click '^导出图表$'
    Click '^导出 300 DPI PNG$'
    $png=Join-Path $lab 'actual-statistics.png'
    Dialog 'save' $png
    $until=[DateTime]::UtcNow.AddSeconds(10)
    while(!(Test-Path -LiteralPath $png) -and [DateTime]::UtcNow -lt $until){Start-Sleep -Milliseconds 200}
    $image=[Drawing.Image]::FromFile($png)
    try {if([Math]::Abs($image.HorizontalResolution-300) -gt 0.5 -or [Math]::Abs($image.VerticalResolution-300) -gt 0.5){throw 'PNG was not saved at 300 DPI'};@{dpiX=$image.HorizontalResolution;dpiY=$image.VerticalResolution;sha256=(Get-FileHash -LiteralPath $png).Hash}}finally{$image.Dispose()}
    Copy-Item -LiteralPath $png -Destination "$Output/actual-statistics.png"
  }
} finally {
  if(!$process.HasExited){
    $found=Get-Process -Id $process.Id
    if($found.StartTime.ToUniversalTime().Ticks -ne $ticks -or $found.Path -ne $Exe){throw 'Cannot verify native process cleanup ownership'}
    $process.Kill($true);[void]$process.WaitForExit(10000)
  }
  $stderr.GetAwaiter().GetResult() | Set-Content "$Output/uia-gui-stderr.txt" -Encoding utf8
  "F4_UIA cleanup exited=$($process.HasExited)"
}
if($records.FindAll({param($x) $x.status -eq 'FAIL'}).Count){exit 1}
exit 0
