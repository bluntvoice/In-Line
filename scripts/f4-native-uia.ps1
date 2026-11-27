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
 [DllImport("user32.dll")] public static extern bool SetCursorPos(int x,int y);
 [DllImport("user32.dll")] public static extern void mouse_event(uint flags,uint x,uint y,uint data,IntPtr extra);
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
if($env:F4_APPROVAL_ONLY -eq '1'){
  Add-Type -AssemblyName System.Security.Cryptography.ProtectedData
  $auth=Join-Path $env:USERPROFILE '.in-line/mcp-security/authorization.dpapi'
  if(Test-Path -LiteralPath $auth){throw 'Approval fixture cannot use prior authorization'}
  $warm=[Diagnostics.Process]::Start($info)
  $warmOut=$warm.StandardOutput.ReadToEndAsync();$warmErr=$warm.StandardError.ReadToEndAsync()
  try{
    $until=[DateTime]::UtcNow.AddSeconds(28)
    while(!$warm.HasExited -and !(Test-Path -LiteralPath $auth) -and [DateTime]::UtcNow -lt $until){Start-Sleep -Milliseconds 250}
    if($warm.HasExited -or !(Test-Path -LiteralPath $auth)){throw 'Installed app did not create synthetic DPAPI root'}
  } finally {if(!$warm.HasExited){$warm.Kill($true);[void]$warm.WaitForExit(10000)}}
  $scope=[Security.Cryptography.DataProtectionScope]::CurrentUser
  $clear=[Security.Cryptography.ProtectedData]::Unprotect([IO.File]::ReadAllBytes($auth),$null,$scope)
  $store=[Text.Encoding]::UTF8.GetString($clear)|ConvertFrom-Json
  try{
    if($store.clients.Count -ne 0 -or $store.sid -ne [Security.Principal.WindowsIdentity]::GetCurrent().User.Value){throw 'Foreign or pre-existing DPAPI grant'}
    $localToken=[Convert]::ToHexString([Security.Cryptography.RandomNumberGenerator]::GetBytes(32)).ToLowerInvariant()
    $localClient=[Convert]::ToHexString([Security.Cryptography.RandomNumberGenerator]::GetBytes(32)).ToLowerInvariant()
    $permissions=@{regularRead=$true;fullRead=$false;write=$true}
    $store.groups=$permissions
    $hash=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($localToken))).ToLowerInvariant()
    $store.clients=@(@{id=$localClient;name='Synthetic F4 UIA client';kind='codex';tokenHash=$hash;revoked=$false;permissions=$permissions;scope=@{departments=@('F4 approval');taskTypes=$null}})
    $store.revision++
    $bytes=[Text.Encoding]::UTF8.GetBytes(($store|ConvertTo-Json -Depth 15 -Compress))
    try{[IO.File]::WriteAllBytes($auth,[Security.Cryptography.ProtectedData]::Protect($bytes,$null,$scope))}
    finally{[Array]::Clear($bytes)}
  } finally {[Array]::Clear($clear);$store=$null}
}
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
        $kind=$summary.ControlType.ProgrammaticName -replace '^ControlType\.',''
        if($summary.Name -match $pattern -and (!$type -or $kind -match "^($type)$") -and $summary.IsEnabled){return $node}
      } catch [Windows.Automation.ElementNotAvailableException] {}
    }
    Start-Sleep -Milliseconds 160
  }
  throw "UIA control unavailable ($type): $pattern"
}
function Click([string]$pattern,[string]$type='Button'){
  $node=Get-Control $pattern $type
  if($node.Current.IsOffscreen){
    $scroll=$null
    if($node.TryGetCurrentPattern([Windows.Automation.ScrollItemPattern]::Pattern,[ref]$scroll)){$scroll.ScrollIntoView()}
    elseif($type -eq 'Button'){Set-InputFocus $node}
    Start-Sleep -Milliseconds 150
  }
  $inv=$null
  if($node.TryGetCurrentPattern([Windows.Automation.InvokePattern]::Pattern,[ref]$inv)){$inv.Invoke();return}
  $toggle=$null
  if($node.TryGetCurrentPattern([Windows.Automation.TogglePattern]::Pattern,[ref]$toggle)){$toggle.Toggle();return}
  if(!$type){
    [void][InlineUiWindow]::SetForegroundWindow($window)
    [uint32]$owner=0
    [void][InlineUiWindow]::GetWindowThreadProcessId([InlineUiWindow]::GetForegroundWindow(),[ref]$owner)
    $rect=$node.Current.BoundingRectangle
    if($owner -ne $process.Id -or $node.Current.IsOffscreen -or $rect.IsEmpty){throw 'Native row is not a visible product control'}
    if(![InlineUiWindow]::SetCursorPos([int]($rect.X+$rect.Width/2),[int]($rect.Y+$rect.Height/2))){throw 'Native row pointer unavailable'}
    [InlineUiWindow]::mouse_event(2,0,0,0,[IntPtr]::Zero)
    [InlineUiWindow]::mouse_event(4,0,0,0,[IntPtr]::Zero)
    return
  }
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
  $node=Get-Control $pattern 'Edit|ComboBox'
  Set-InputFocus $node
  $inputValue=$null
  $hasValue=$node.TryGetCurrentPattern([Windows.Automation.ValuePattern]::Pattern,[ref]$inputValue) -and !$inputValue.Current.IsReadOnly
  if($hasValue){
    $inputValue.SetValue($value)
    Start-Sleep -Milliseconds 150
  }
  if(!$hasValue -or $inputValue.Current.Value -ne $value){
    [Windows.Forms.Clipboard]::SetText($value)
    [Windows.Forms.SendKeys]::SendWait('^a')
    [Windows.Forms.SendKeys]::SendWait('^v')
    $until=[DateTime]::UtcNow.AddSeconds(3)
    if($hasValue){
      while($inputValue.Current.Value -ne $value -and [DateTime]::UtcNow -lt $until){Start-Sleep -Milliseconds 100}
      if($inputValue.Current.Value -ne $value){throw "Native editor input did not settle: $pattern"}
    }else{Start-Sleep -Milliseconds 400}
  }
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
  & (Join-Path $PSScriptRoot 'f4-win-controls.ps1') -Mode $mode -Path $path -ProductPid $process.Id | ForEach-Object {Write-Host $_}
  if($LASTEXITCODE){throw "Native common dialog $mode failed"}
}
function Confirm-RestoreDialog {
  $due=[DateTime]::UtcNow.AddSeconds(7)
  while([DateTime]::UtcNow -lt $due){
    $saved=@(Children)|Where-Object {$_.Current.Name -eq '备份数据已成功导入'}|Select-Object -First 1
    if($saved){return}
    $front=[InlineUiWindow]::GetForegroundWindow()
    [uint32]$owner=0
    [void][InlineUiWindow]::GetWindowThreadProcessId($front,[ref]$owner)
    if($front -ne $window -and $owner -eq $process.Id){
      $popup=[Windows.Automation.AutomationElement]::FromHandle($front)
      $children=$popup.FindAll([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.Condition]::TrueCondition)
      $yes=$children|Where-Object {$_.Current.ControlType -eq [Windows.Automation.ControlType]::Button -and $_.Current.Name -match '^(确定|OK|Yes|是)$'}|Select-Object -First 1
      $question=$children|Where-Object {$_.Current.Name -match '备份|恢复|合并|backup|restore'}|Select-Object -First 1
      if($yes -and $question){$yes.GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern).Invoke();return}
      if($popup.Current.Name -match '了解|排着呢|In Line|确认' -and $question){
        [Windows.Forms.SendKeys]::SendWait('{ENTER}')
        return
      }
    }
    try{Click '^(确定|OK|Yes|是)$';return}catch{}
    Start-Sleep -Milliseconds 160
  }
  [void](Get-Control '^备份数据已成功导入$' '' 15)
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
function Invoke-Approval([string]$mode){
  $env:F4_GUI_CLIENT=$localClient
  $env:F4_GUI_TOKEN=$localToken
  try{
    & node (Join-Path $PSScriptRoot 'f4-uia-approval.mjs') $mode $Exe $db $lab | ForEach-Object {Write-Host $_}
    if($LASTEXITCODE){throw "Synthetic endpoint assertion failed at $mode"}
  }finally{Remove-Item Env:F4_GUI_CLIENT,Env:F4_GUI_TOKEN -ErrorAction SilentlyContinue}
}
function Set-HostedDisplayScale([int]$scale){
  Start-Process 'ms-settings:display'
  $until=[DateTime]::UtcNow.AddSeconds(12)
  $combo=$null
  while(!$combo -and [DateTime]::UtcNow -lt $until){
    $active=[InlineUiWindow]::GetForegroundWindow()
    [uint32]$owner=0;[void][InlineUiWindow]::GetWindowThreadProcessId($active,[ref]$owner)
    $settingsProcess=Get-Process -Id $owner -ErrorAction SilentlyContinue
    if($settingsProcess.ProcessName -in @('SystemSettings','ApplicationFrameHost') -and $settingsProcess.SessionId -eq $pidSession){
      $settingsRoot=[Windows.Automation.AutomationElement]::FromHandle($active)
      $controls=$settingsRoot.FindAll([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.Condition]::TrueCondition)
      $combo=$controls|Where-Object {
        $_.Current.ControlType -eq [Windows.Automation.ControlType]::ComboBox -and
        ($_.Current.Name -match 'scale|size of text|缩放' -or $_.Current.AutomationId -match 'Scale')
      }|Select-Object -First 1
    }
    if(!$combo){Start-Sleep -Milliseconds 250}
  }
  if(!$combo){throw 'Windows Display Settings did not expose an owned Scale control'}
  $expand=$combo.GetCurrentPattern([Windows.Automation.ExpandCollapsePattern]::Pattern)
  $expand.Expand()
  $optionsNames=@("$scale%","$scale% (Recommended)","$scale%（推荐）","$scale% (推荐)")
  [Windows.Automation.Condition[]]$match=@($optionsNames|ForEach-Object{[Windows.Automation.PropertyCondition]::new([Windows.Automation.AutomationElement]::NameProperty,$_)})
  $condition=[Windows.Automation.OrCondition]::new($match)
  $options=[Windows.Automation.AutomationElement]::RootElement.FindAll([Windows.Automation.TreeScope]::Descendants,$condition)
  $select=$null
  foreach($option in $options){
    $native=$option.Current
    $p=Get-Process -Id $native.ProcessId -ErrorAction SilentlyContinue
    if($p.ProcessName -in @('SystemSettings','ApplicationFrameHost') -and $p.SessionId -eq $pidSession -and !$native.IsOffscreen -and
       $option.TryGetCurrentPattern([Windows.Automation.SelectionItemPattern]::Pattern,[ref]$select)){break}
  }
  if(!$select){$expand.Collapse();throw "Windows Display Settings lacks a selectable $scale% scale"}
  $select.Select()
  $expected=[int](96*$scale/100)
  $end=[DateTime]::UtcNow.AddSeconds(8)
  while([InlineUiWindow]::GetDpiForWindow($window) -ne $expected -and [DateTime]::UtcNow -lt $end){Start-Sleep -Milliseconds 220}
  if([InlineUiWindow]::GetDpiForWindow($window) -ne $expected){throw "Native HWND did not receive $expected DPI after the Windows setting"}
  [void][InlineUiWindow]::SetForegroundWindow($window)
  $expected
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
  if($env:F4_APPROVAL_ONLY -eq '1'){
    Step 'real-batch-approve-reject-and-replay' {
      Click '^软件设置$'
      Invoke-Approval 'seed'
      foreach($mode in 'approve','reject'){
        Invoke-Approval $mode
        $section=Get-Control '^批量操作' 'Button'
        $expand=$null
        if(!$section.TryGetCurrentPattern([Windows.Automation.ExpandCollapsePattern]::Pattern,[ref]$expand) -or
           $expand.Current.ExpandCollapseState -ne [Windows.Automation.ExpandCollapseState]::Expanded){Click '^批量操作'}
        Click '^查看整批影响$'
        [void](Get-Control '2030-01-0[34]')
        if($mode -eq 'approve'){
          Click '^审阅并确认$';Click '^确认整批执行$'
        }else{Click '^拒绝$'}
        Start-Sleep -Milliseconds 700
        Invoke-Approval ("check-"+$mode)
        Screen ("native-batch-"+$mode)
      }
    }
    Step 'real-undo-approved-in-gui' {
      Invoke-Approval 'undo'
      Click '^待办队列(?:\s+\d+)?$';Click '^软件设置$'
      Click '^AI 操作记录与撤销申请'
      Click '^查看原操作前后变化$'
      Click '^审阅并撤销$';Click '^确认撤销$'
      Start-Sleep -Milliseconds 700
      Invoke-Approval 'check-undo'
      Screen 'native-undo'
    }
    if($records.FindAll({param($x) $x.status -eq 'FAIL'}).Count){exit 1}
    exit 0
  }
  if(!$env:F4_R1_MODE){
  Step 'about-and-native-window' {
    Click '^关于$'
    [void](Get-Control '^v?0\.5\.0$' 'Text')
    Screen 'about'
    Click '^待办队列(?:\s+\d+)?$'
    @{dpi=$dpi;actualHwnd=$true;about='0.5.0'}
  }
  }
  if($env:F4_R1_MODE){
    . (Join-Path $PSScriptRoot 'f4-r1-uia-support.ps1')
    . (Join-Path $PSScriptRoot 'f4-r1-uia-cases.ps1')
    switch($env:F4_R1_MODE){
      'timeline' {R1-Timeline}
      'statistics' {R1-Stats}
      'business' {R1-Business}
      'dpi125' {R1-Dpi 125}
      'dpi200' {R1-Dpi 200}
      'restore' {}
      default {throw 'Unknown F4-R1 scenario'}
    }
  }
  if(!$env:F4_R1_MODE -or $env:F4_R1_MODE -eq 'restore'){
  Step 'three-real-restore-rounds' {
    New-Task 'F4 source' 'original source note'
    Click '^软件设置$';Click '^立即备份$'
    [void](Get-Control '^备份完成：InLine-backup-.*-manual\.db$')
    $backupDir=Join-Path (Split-Path $db) 'backups'
    $backup=$null
    $until=[DateTime]::UtcNow.AddSeconds(10)
    while(!$backup -and [DateTime]::UtcNow -lt $until){
      $backup=Get-ChildItem -LiteralPath $backupDir -Filter '*-manual.db' -File -ErrorAction SilentlyContinue | Where-Object Length -GE 4096 | Select-Object -First 1
      if(!$backup){Start-Sleep -Milliseconds 200}
    }
    if(!$backup){throw 'GUI did not create backup'}
    $source=Join-Path $lab 'import-source.db'
    Copy-Item -LiteralPath $backup.FullName -Destination $source
    if($env:F4_R1_MODE -eq 'restore'){
      & python -X utf8 (Join-Path $PSScriptRoot 'f4-r1-fixtures.py') $source 'backup-source' | ForEach-Object {Write-Host "F4_R1 source $_"}
      if($LASTEXITCODE){throw 'GUI manual backup is not a valid synthetic source'}
    }
    $sourceHash=(Get-FileHash -LiteralPath $source).Hash
    (Get-Item -LiteralPath $source).LastWriteTimeUtc=[DateTime]::new(2024,1,1)
    for($i=1;$i -le 13;$i++){Copy-Item -LiteralPath $source -Destination (Join-Path $backupDir ('InLine-backup-20261001-0101{0:00}-manual.db' -f $i))}
    Click '^待办队列(?:\s+\d+)?$';Click '^F4 source$' ''
    Click '^编辑$';Fill '^(记录判断|内部备注)' 'local edited note';Click '^保存修改$'
    try{Click '^关闭$'}catch{}
    New-Task 'F4 source（冲突）'
    Click '^软件设置$'
    $expected=@('新增 1 项，合并 0 项，冲突保留 1 项','新增 0 项，合并 1 项，冲突保留 0 项','新增 0 项，合并 1 项，冲突保留 0 项')
    for($i=0;$i -lt 3;$i++){
      Click '^导入并恢复$'
      Dialog 'open' $source
      Confirm-RestoreDialog
      [void](Get-Control $expected[$i])
      if($i -eq 0){[void](Get-Control 'F4 source（冲突 2）');Click '^知道了，稍后处理$'}
      [void](Get-Control '刚刚导入')
      Verify-Db 'restore'
      if((Get-FileHash -LiteralPath $source).Hash -ne $sourceHash){throw 'Source bytes changed'}
      Screen ("restore-"+($i+1))
      if($i -eq 2){Click '^查看待办$'}else{Click '^关闭通知$'}
    }
    [void](Get-Control '^待办队列$' 'Text')
    $leaf=@(Children | Where-Object {
      $x=$_.Current
      $x.ControlType -eq [Windows.Automation.ControlType]::DataItem -and
        !$x.IsOffscreen -and $x.Name -ceq 'F4 source（冲突 2） 导入冲突'
    })
    if($leaf.Count -ne 1){throw "Expected one exact conflict title and badge in queue, found $($leaf.Count)"}
    $title=$leaf[0].Current.Name
    Screen 'restore-queue-conflict'
    @{sourceSha256=$sourceHash;rounds=$expected;queueConflict=$title;exactVisibleMatches=$leaf.Count;sqlIntegrity='ok';foreignKeyViolations=0}
  }
  }
  if(!$env:F4_R1_MODE){
  Step 'real-timeline-keyboard' {
    Click '^待办队列(?:\s+\d+)?$'
    New-Task 'F4 processed'
    Click '^F4 processed$' ''
    Click '^本轮已处理$'
    Click '^暂缓事项(?:\s+\d+)?$'
    Click '^F4 processed$' ''
    [void](Get-Control '^事项时间线$')
    Fill '^补充一条普通处理备注' 'F4 UIA memo'
    Click '^添加$'
    [void](Get-Control 'F4 UIA memo')
    $summary='^本轮已处理，移入暂缓事项$'
    $toggle=Get-Control $summary
    Set-InputFocus $toggle
    [Windows.Forms.SendKeys]::SendWait('{ENTER}')
    [void](Get-Control '^原始记录$')
    [void](Get-Control '^已记录本轮处理事项$|^状态变更为：|^退出 ')
    Screen 'timeline'
    Set-InputFocus (Get-Control $summary)
    [Windows.Forms.SendKeys]::SendWait('{ENTER}')
    $raw=@(Children|Where-Object {$_.Current.Name -eq '原始记录' -and !$_.Current.IsOffscreen})
    if($raw.Count){throw 'Keyboard collapse left raw entries visible'}
    Click '^关闭$'
    @{rawRecords=$true;keyboardExpandCollapse=$true;note=$true}
  }
  Step 'real-statistics-navigation' {
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
    [Windows.Forms.Clipboard]::Clear()
    Click '^复制报告$'
    $until=[DateTime]::UtcNow.AddSeconds(5)
    $text=''
    while([DateTime]::UtcNow -lt $until){
      $text=[Windows.Forms.Clipboard]::GetText()
      if($text -match '工作报告'){break}
      Start-Sleep -Milliseconds 150
    }
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
  Step 'real-display-dpi' {
    $verified=@(96)
    $prereq=$null
    try {
      foreach($scale in 125,150,200){
        $measured=Set-HostedDisplayScale $scale
        $verified+=$measured
        Write-Host "F4_UIA system scale=$scale HWND dpi=$measured"
        Click '^统计中心$'
        [void](Get-Control '工作量趋势')
        Screen "statistics-system-dpi-$scale"
      }
    } catch {$prereq=$_.Exception.Message;Write-Host "F4_UIA display primary=$prereq"}
    finally {
      if([InlineUiWindow]::GetDpiForWindow($window) -ne 96){
        try{[void](Set-HostedDisplayScale 100)}
        catch {Write-Host "F4_UIA display reset=$($_.Exception.Message)"; if(!$prereq){$prereq=$_.Exception.Message}}
      }
    }
    if($prereq){throw $prereq}
    @{physicalDpi=$verified;restoredDpi=[InlineUiWindow]::GetDpiForWindow($window)}
  }
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
