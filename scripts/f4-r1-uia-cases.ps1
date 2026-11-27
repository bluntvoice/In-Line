function R1-CheckBusiness([string]$title,[string]$state,[string]$expected='-'){
  $row=R1-Read 'business-read' @($title,$state,$expected)
  ConvertTo-Json -InputObject $row -Depth 9 | Set-Content -LiteralPath "$Output/business-$state.json" -Encoding utf8
  $row
}
function R1-Business {
  Step 'r1-business-new-edit-defer-queue' {
    New-Task 'F4 R1 main' 'original'
    R1-Open '待办队列' 'F4 R1 main'
    Click '^编辑$'
    Fill '^(记录判断|内部备注)' 'edited'
    Click '^保存修改$'
    $a=R1-CheckBusiness 'F4 R1 main' 'edited' 'edited'
    try{Click '^关闭$'}catch{}
    R1-Open '待办队列' 'F4 R1 main'
    Click '^本轮已处理$'
    R1-Open '暂缓事项' 'F4 R1 main'
    $b=R1-CheckBusiness 'F4 R1 main' 'deferred'
    Click '^加入今日队列$'
    Click '^确认加入$'
    try{Click '^关闭$'}catch{}
    R1-Open '待办队列' 'F4 R1 main'
    $c=R1-CheckBusiness 'F4 R1 main' 'enqueued'
    if($a.number -cne $b.number -or $a.number -cne $c.number){throw 'Permanent number changed'}
    Screen 'business-requeue'
    Click '^关闭$'
    @{permanentNumber=$a.number;activeQueue=$true;history=$true}
  }
  Step 'r1-business-complete-archive-reopen' {
    Click '^待办队列(?:\s+\d+)?$'
    New-Task 'F4 R1 complete'
    R1-Open '待办队列' 'F4 R1 complete'
    Click '^本轮已完成$'
    R1-Open '历史归档' 'F4 R1 complete'
    $before=R1-CheckBusiness 'F4 R1 complete' 'completed'
    Screen 'business-archive'
    Click '^重新开启并加入今日队列$'
    Fill '^重新开启原因|^选填，将记录到事项时间线$' 'R1 reopen reason'
    Click '^确认加入$'
    try{Click '^关闭$'}catch{}
    R1-Open '待办队列' 'F4 R1 complete'
    $after=R1-CheckBusiness 'F4 R1 complete' 'reopened'
    if($before.number -cne $after.number){throw 'Reopen changed permanent number'}
    Click '^关闭$'
    @{archived=$true;reopened=$true;priorHistory=$true}
  }
  Step 'r1-business-relationship' {
    Click '^待办队列(?:\s+\d+)?$'
    New-Task 'F4 R1 parent' 'parent attribute'
    New-Task 'F4 R1 parent two' 'second parent attribute'
    New-Task 'F4 R1 child' 'child attribute'
    R1-Open '待办队列' 'F4 R1 child'
    R1-Select '^设置所属任务$' '^F4 R1 parent · '
    [void](Get-Control '^所属任务已更新$')
    [void](R1-CheckBusiness 'F4 R1 child' 'parent' 'F4 R1 parent')
    R1-Select '^更换或解除所属$' '^F4 R1 parent two · '
    [void](Get-Control '^所属任务已更新$')
    $parent=R1-CheckBusiness 'F4 R1 child' 'parent' 'F4 R1 parent two'
    Screen 'business-parent'
    R1-Select '^更换或解除所属$' '^不设置所属任务$'
    [void](Get-Control '^已解除所属任务$')
    [void](R1-CheckBusiness 'F4 R1 child' 'unlinked')
    Click '^关闭$'
    @{parentId=$parent.parent;changedAndRemoved=$true;independentAttributes=$true}
  }
  Step 'r1-business-urgent-deadline' {
    try{Click '^关闭$'}catch{}
    Click '^待办队列(?:\s+\d+)?$'
    New-Task 'F4 R1 urgent'
    R1-Open '待办队列' 'F4 R1 urgent'
    Click '^编辑$'
    Click '^标记为加急事项$' 'CheckBox'
    Fill '^加急申请人' 'R1 synthetic requester'
    Fill '^加急原因' 'R1 deadline check'
    Click '^1 小时后$'
    Click '^保存修改$'
    [void](R1-CheckBusiness 'F4 R1 urgent' 'urgent')
    Screen 'business-urgent'
    try{Click '^关闭$'}catch{}
    @{deadline='one-hour preset';urgency=$true}
  }
  Step 'r1-business-future-and-early-entry' {
    try{Click '^关闭$'}catch{}
    Click '^待办队列(?:\s+\d+)?$'
    Click '^新增取号'
    Fill '^(输入或选择部门|部门 / 团队)' 'F4 UIA Team' $true
    Fill '^(输入或选择对接人|对接人)' 'F4 UIA Contact' $true
    Fill '^(输入或选择事项类型|事项类型)' 'F4 UIA Type'
    Fill '^(一句话说明|事项标题)' 'F4 R1 future'
    $planned=[datetime]::Today.AddDays(3)
    R1-Date '加入日期' $planned
    Click '^保存并取号$'
    R1-Open '暂缓事项' 'F4 R1 future'
    $before=R1-CheckBusiness 'F4 R1 future' 'scheduled' $planned.ToString('yyyy-MM-dd')
    Click '^提前入队$'
    Click '^确认加入$'
    try{Click '^关闭$'}catch{}
    R1-Open '待办队列' 'F4 R1 future'
    $after=R1-CheckBusiness 'F4 R1 future' 'early'
    if($before.number -ne $after.number -or !@($after.allocations|Where-Object {$_.voided_at -and $_.queue_date -eq $planned.ToString('yyyy-MM-dd')}).Count){
      throw 'Scheduled reservation was not safely voided'
    }
    Screen 'business-future-early'
    Click '^关闭$'
    @{reservation=$before.allocations[0].daily_sequence;scheduledDate=$planned.ToString('yyyy-MM-dd');originalVoided=$true}
  }
  Step 'r1-business-merge-trash-restore' {
    try{Click '^关闭$'}catch{}
    Click '^待办队列(?:\s+\d+)?$'
    New-Task 'F4 R1 merge source'
    New-Task 'F4 R1 merge target'
    R1-Open '待办队列' 'F4 R1 merge target'
    Click '^合并$'
    R1-Select '^选择需要并入的重复事项$' ' · F4 R1 merge source$'
    Click '^确认合并$'
    [void](Get-Control '^事项已合并，相关记录已保留$')
    [void](R1-CheckBusiness 'F4 R1 merge target' 'merged' 'F4 R1 merge source')
    try{Click '^关闭$'}catch{}
    R1-Open '待办队列' 'F4 R1 merge target'
    Click '^移入回收站$'
    R1-Open '回收站' 'F4 R1 merge target'
    [void](R1-CheckBusiness 'F4 R1 merge target' 'trash')
    Click '^恢复$'
    R1-Open '待办队列' 'F4 R1 merge target'
    [void](R1-CheckBusiness 'F4 R1 merge target' 'restored')
    Screen 'business-restored'
    Click '^关闭$'
    @{merge=$true;trash=$true;restore=$true}
  }
  Step 'r1-business-restart' {
    $first=R1-CheckBusiness 'F4 R1 main' 'edited' 'edited'
    $owned=Get-Process -Id $process.Id
    if($owned.StartTime.ToUniversalTime().Ticks -ne $ticks -or $owned.Path -ne $Exe){throw 'Cannot identify original app for restart'}
    if(!$process.CloseMainWindow() -or !$process.WaitForExit(8000)){throw 'Could not close the app gracefully for restart'}
    $script:process=[Diagnostics.Process]::Start($info)
    $script:ticks=$process.StartTime.ToUniversalTime().Ticks
    $script:stdout=$process.StandardOutput.ReadToEndAsync()
    $script:stderr=$process.StandardError.ReadToEndAsync()
    $end=[datetime]::UtcNow.AddSeconds(25)
    do{
      $script:window=[InlineUiWindow]::ProductWindow([uint32]$process.Id,'In Line｜排着呢')
      if($window -ne [IntPtr]::Zero){break}
      Start-Sleep -Milliseconds 200
    }while([datetime]::UtcNow -lt $end -and !$process.HasExited)
    if($window -eq [IntPtr]::Zero -or $process.SessionId -ne $pidSession){throw 'Restarted app lacks verified visible product HWND'}
    R1-Open '待办队列' 'F4 R1 main'
    [void](Get-Control '^edited$' 'Text')
    $second=R1-CheckBusiness 'F4 R1 main' 'edited' 'edited'
    if($second.number -ne $first.number -or $second.work.Count -ne $first.work.Count){throw 'Persisted task or history changed'}
    Screen 'business-after-restart'
    @{number=$second.number;workEvents=$second.work.Count;persisted=$true}
  }
}
function R1-DateText([datetime]$a,[datetime]$b){
  "$($a.ToString('yyyy-MM-dd')) 至 $($b.ToString('yyyy-MM-dd'))"
}
function R1-Range([string]$label,[datetime]$a,[datetime]$b,[datetime]$c,[datetime]$d,[string]$comparison){
  $current=R1-DateText $a $b
  $previous=R1-DateText $c $d
  [void](Get-Control ('^'+[regex]::Escape($current)+'(?: · 周[一日]起算)?$') 'Text')
  [void](Get-Control ('^'+[regex]::Escape($comparison)+'$') 'Text')
  $end=[datetime]::UtcNow.AddSeconds(6)
  do {
    $names=@(Children|ForEach-Object {$_.Current.Name}|Where-Object {$_})
    $joined=$names -join ''
    if(($joined -replace '\s','').Contains(("对比范围：$previous" -replace '\s',''))){break}
    Start-Sleep -Milliseconds 150
  }while([datetime]::UtcNow -lt $end)
  if(!(($joined -replace '\s','').Contains(("对比范围：$previous" -replace '\s','')))){throw "Previous dates not in actual snapshot for ${label}: $previous"}
  @{label=$label;current=$current;previous=$previous;comparison=$comparison}
}
function R1-Stats {
  $today=[datetime]::Today
  $mon=$today.AddDays(-(([int]$today.DayOfWeek+6)%7))
  $sun=$today.AddDays(-[int]$today.DayOfWeek)
  $month=[datetime]::new($today.Year,$today.Month,1)
  $quarter=[datetime]::new($today.Year,1+3*[math]::Floor(($today.Month-1)/3),1)
  Step 'r1-statistics-fixture' {
    foreach($suffix in 'New A','New B','Prior'){
      Click '^待办队列(?:\s+\d+)?$'
      New-Task "F4 R1 $suffix"
      R1-Open '待办队列' "F4 R1 $suffix"
      Click '^本轮已处理$'
      [void](Get-Control '^已记录本轮处理，事项已进入暂缓队列$')
    }
    $rows=R1-Read 'stats-seed' @($today.ToString('yyyy-MM-dd'))
    ConvertTo-Json -InputObject @($rows) -Depth 5 | Set-Content -LiteralPath "$Output/statistics-fixture.json" -Encoding utf8
    @{distinctTasks=3;current=2;previous=1;syntheticHistoryInitialization=$true}
  }
  $spec=@(
    @('本周',$mon,$today,$mon.AddDays(-7),$today.AddDays(-7),'上周同期'),
    @('上周',$mon.AddDays(-7),$mon.AddDays(-1),$mon.AddDays(-14),$mon.AddDays(-8),'上上周'),
    @('本月',$month,$today,$month.AddMonths(-1),[datetime]::new($today.Year,$today.Month,1).AddDays(-1),'上月同期'),
    @('上月',$month.AddMonths(-1),$month.AddDays(-1),$month.AddMonths(-2),$month.AddMonths(-1).AddDays(-1),'再上一个月'),
    @('本季度',$quarter,$today,$quarter.AddMonths(-3),$quarter.AddDays(-1),'上季度同期'),
    @('上季度',$quarter.AddMonths(-3),$quarter.AddDays(-1),$quarter.AddMonths(-6),$quarter.AddMonths(-3).AddDays(-1),'再上一季度')
  )
  foreach($idx in 2,4){
    $e=$spec[$idx][3].AddDays(($today-$spec[$idx][1]).Days)
    if($e -lt $spec[$idx][4]){$spec[$idx][4]=$e}
  }
  foreach($row in $spec){
    Step ("r1-statistics-dates-"+$row[0]) {
      Click '^统计中心$'
      Click ('^'+$row[0]+'$')
      $period=R1-Range @row
      R1-WriteTree ("statistics-"+$row[0])
      $period
    }
  }
  Step 'r1-statistics-sunday' {
    Click '^软件设置$'
    Click '^周日$'
    Click '^统计中心$';Click '^本周$'
    $period=R1-Range '周日' $sun $today $sun.AddDays(-7) $today.AddDays(-7) '上周同期'
    [void](Get-Control ('^'+[regex]::Escape((R1-DateText $sun $today))+' · 周日起算$') 'Text')
    R1-WriteTree 'statistics-sunday'
    Click '^软件设置$';Click '^周一$';Click '^统计中心$';Click '^本周$'
    $period
  }
  Step 'r1-statistics-values-and-drill' {
    Click '^统计中心$';Click '^本周$'
    [void](R1-Range '本周' $mon $today $mon.AddDays(-7) $today.AddDays(-7) '上周同期')
    $names=@(Children|ForEach-Object {$_.Current.Name}|Where-Object {$_})
    $joined=$names -join '|'
    if($joined -notmatch '处理事项\|\+1\|2\|1'){throw 'Expected current 2 and previous 1 in the actual workload row'}
    $date=$today.ToString('yyyy-MM-dd')
    Click ("^$date（周.） 全部办理事项 2 项$")
    foreach($title in 'F4 R1 New A','F4 R1 New B'){[void](Get-Control ('^'+$title+'$'))}
    if(@(R1-Visible 'F4 R1 Prior' '').Count){throw 'Prior-period fixture leaked into current detail'}
    [void](Get-Control '^2 项$' 'Text')
    R1-WriteTree 'statistics-drill'
    Screen 'statistics-drill'
    Click '^收起明细$'
    @{current=2;previous=1;detail=2;date=$date}
  }
  Step 'r1-statistics-custom-and-state' {
    Click '^统计中心$';Click '^自定义$'
    $start=$today.AddDays(-2)
    R1-Date '开始日期' $start
    R1-Date '结束日期' $today
    $null=R1-Range '自定义' $start $today $start.AddDays(-3) $start.AddDays(-1) '前一等长周期'
    R1-Open '暂缓事项' 'F4 R1 New A'
    Click '^关闭$';Click '^统计中心$'
    $range=R1-Range '自定义' $start $today $start.AddDays(-3) $start.AddDays(-1) '前一等长周期'
    R1-WriteTree 'statistics-custom-restored'
    Click '^本周$'
    $range
  }
  Step 'r1-statistics-report-and-png-content' {
    try{Click '^关闭$'}catch{}
    Click '^统计中心$';Click '^本周$'
    $null=R1-Range '本周' $mon $today $mon.AddDays(-7) $today.AddDays(-7) '上周同期'
    Click '^生成报告$';Click '原生工作报告';Click '^预览报告$'
    [void](Get-Control '^本周工作报告$')
    $period=R1-DateText $mon $today
    [void](Get-Control ('统计期间为 '+[regex]::Escape($period)+'。本周共处理 2 项，其中完成 0 项'))
    Click '^复制报告$'
    $report=[Windows.Forms.Clipboard]::GetText()
    if(!$report.Contains("统计期间为 $period。") -or !$report.Contains('本周共处理 2 项，其中完成 0 项')){throw 'Actual work report data differs from UI and known counts'}
    $report|Set-Content -LiteralPath "$Output/statistics-report.txt" -Encoding utf8
    Screen 'statistics-report'
    Click '^关闭$'
    R1-WriteTree 'statistics-current-snapshot'
    Click '^导出图表$';Click '^导出 300 DPI PNG$'
    $png=Join-Path $lab 'r1-statistics-content.png'
    Dialog 'save' $png
    $end=[datetime]::UtcNow.AddSeconds(10)
    while(!(Test-Path -LiteralPath $png) -and [datetime]::UtcNow -lt $end){Start-Sleep -Milliseconds 200}
    $image=[Drawing.Image]::FromFile($png)
    try {
      if([Math]::Abs($image.HorizontalResolution-300) -gt 0.5 -or [Math]::Abs($image.VerticalResolution-300) -gt 0.5){throw 'Actual PNG DPI mismatches expected'}
      $metadata=@{width=$image.Width;height=$image.Height;dpi=$image.HorizontalResolution;expectedPeriod=$period;expectedPrevious='上周同期';current=2;previous=1;sha256=(Get-FileHash -LiteralPath $png).Hash}
    }finally{$image.Dispose()}
    Copy-Item -LiteralPath $png -Destination "$Output/r1-statistics-content.png"
    $metadata
  }
}
function R1-Dpi([int]$scale){
  Step ("r1-display-$scale") {
    $changed=$false
    try {
      $measurement=Set-HostedDisplayScale $scale
      $changed=$true
      $windowBounds=[Windows.Automation.AutomationElement]::FromHandle($window).Current.BoundingRectangle
      Write-Host "F4_R1_DPI scale=$scale dpi=$measurement size=$($windowBounds.Width)x$($windowBounds.Height)"
      Click '^待办队列(?:\s+\d+)?$'
      New-Task 'F4 R1 DPI'
      R1-Open '待办队列' 'F4 R1 DPI'
      [void](Get-Control '^事项时间线$' 'Text')
      R1-WriteTree "dpi-$scale-detail"
      Screen "dpi-$scale-detail"
      Click '^关闭$';Click '^统计中心$';Click '^本周$'
      [void](Get-Control '^工作量趋势$' 'Text')
      R1-WriteTree "dpi-$scale-statistics"
      Screen "dpi-$scale-statistics"
      Click '^软件设置$'
      [void](Get-Control '^数据备份$')
      Click '^导入并恢复$'
      $end=[datetime]::UtcNow.AddSeconds(5)
      do{
        $front=[InlineUiWindow]::GetForegroundWindow()
        [uint32]$owner=0
        [void][InlineUiWindow]::GetWindowThreadProcessId($front,[ref]$owner)
        if($front -ne $window -and $owner -eq $process.Id){break}
        Start-Sleep -Milliseconds 120
      }while([datetime]::UtcNow -lt $end)
      if($front -eq $window -or $owner -ne $process.Id){throw 'Owned file picker missing at real DPI'}
      $native=[Windows.Automation.AutomationElement]::FromHandle($front)
      $bounds=$native.Current.BoundingRectangle
      $bitmap=[Drawing.Bitmap]::new([int]$bounds.Width,[int]$bounds.Height)
      $graphics=[Drawing.Graphics]::FromImage($bitmap)
      try{
        $graphics.CopyFromScreen([int]$bounds.X,[int]$bounds.Y,0,0,$bitmap.Size)
        $bitmap.Save("$Output/dpi-$scale-file-picker.png",[Drawing.Imaging.ImageFormat]::Png)
      }finally{$graphics.Dispose();$bitmap.Dispose()}
      [Windows.Forms.SendKeys]::SendWait('{ESC}')
      [void](Get-Control '^导入并恢复$' 'Button')
      @{scale=$scale;actualDpi=$measurement;windowWidth=$windowBounds.Width;windowHeight=$windowBounds.Height;filePickerOwner=$owner;filePickerWidth=$bounds.Width;filePickerHeight=$bounds.Height}
    }finally{
      if($changed -or [InlineUiWindow]::GetDpiForWindow($window) -ne 96){
        $restored=Set-HostedDisplayScale 100
        Write-Host "F4_R1_DPI restored=$restored"
      }
    }
  }
}
