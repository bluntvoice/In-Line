function R1-CheckBusiness([string]$title,[string]$state,[string]$expected='-'){
  $row=R1-Read 'business-read' @($title,$state,$expected)
  ConvertTo-Json -InputObject $row -Depth 9 | Set-Content -LiteralPath "$Output/business-$state.json" -Encoding utf8
  $row
}
function R1-Business {
  Step 'r1-business-new-edit-defer-queue' {
    R1-NewTask 'F4 R1 main' 'original'
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
    R1-NewTask 'F4 R1 complete'
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
    R1-NewTask 'F4 R1 parent' 'parent attribute'
    R1-NewTask 'F4 R1 parent two' 'second parent attribute'
    R1-NewTask 'F4 R1 child' 'child attribute'
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
    R1-NewTask 'F4 R1 urgent'
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
    R1-TaskForm 'F4 R1 future'
    $planned=[datetime]::Today.AddDays(3)
    R1-Date '加入日期' $planned
    Click '^保存并取号$'
    R1-Open '暂缓事项' 'F4 R1 future' (' 未来事项 · '+$planned.ToString('yyyy-MM-dd'))
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
    R1-NewTask 'F4 R1 merge source'
    R1-NewTask 'F4 R1 merge target'
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
    try{Click '^关闭$'}catch{}
    Click '^待办队列(?:\s+\d+)?$'
    R1-NewTask 'F4 R1 persistent' 'original'
    R1-Open '待办队列' 'F4 R1 persistent'
    Click '^编辑$';Fill '^(记录判断|内部备注)' 'edited';Click '^保存修改$'
    $first=R1-CheckBusiness 'F4 R1 persistent' 'edited' 'edited'
    $owned=Get-Process -Id $process.Id
    if($owned.StartTime.ToUniversalTime().Ticks -ne $ticks -or $owned.Path -ne $Exe){throw 'Cannot identify original app for restart'}
    $process.Kill($true)
    if(!$process.WaitForExit(10000)){throw 'Verified test app did not exit for controlled restart'}
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
    R1-Open '待办队列' 'F4 R1 persistent'
    [void](Get-Control '^edited$' 'Text')
    $second=R1-CheckBusiness 'F4 R1 persistent' 'edited' 'edited'
    if($second.number -ne $first.number -or $second.work.Count -ne $first.work.Count){throw 'Persisted task or history changed'}
    Screen 'business-after-restart'
    @{number=$second.number;workEvents=$second.work.Count;persisted=$true;shutdown='bounded termination of exact test PID'}
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
    Click '^待办队列(?:\s+\d+)?$'
    R1-NewTask 'F4 R1 New A'
    R1-Open '待办队列' 'F4 R1 New A'
    Click '^本轮已处理$'
    [void](Get-Control '^已记录本轮处理，事项已进入暂缓队列$')
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
    [void](Get-Control '^\s*· 周日起算$' 'Text')
    R1-WriteTree 'statistics-sunday'
    Click '^软件设置$';Click '^周一$';Click '^统计中心$';Click '^本周$'
    $period
  }
  Step 'r1-statistics-values-and-drill' {
    Click '^统计中心$';Click '^本周$'
    [void](R1-Range '本周' $mon $today $mon.AddDays(-7) $today.AddDays(-7) '上周同期')
    $names=@(Children|ForEach-Object {$_.Current.Name}|Where-Object {$_})
    $joined=$names -join '|'
    if($joined -notmatch '处理事项\|\+100%\|2\|1'){throw 'Expected current 2 and previous 1 in the actual workload row'}
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
  Step 'r1-statistics-report-and-png-content' {

    Click '^统计中心$';Click '^本周$'
    $null=R1-Range '本周' $mon $today $mon.AddDays(-7) $today.AddDays(-7) '上周同期'
    Click '^生成报告$';Click '原生工作报告';Click '^预览报告$'
    [void](Get-Control '^本周工作报告$')
    $period=R1-DateText $mon $today
    Click '^复制报告$'
    $deadline=[datetime]::UtcNow.AddSeconds(5)
    do{
      $report=[Windows.Forms.Clipboard]::GetText()
      if($report.Contains("统计期间为 $period。")){break}
      Start-Sleep -Milliseconds 150
    }while([datetime]::UtcNow -lt $deadline)
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
function R1-StatsState {
  Step 'r1-statistics-custom-and-state' {
    Click '^待办队列(?:\s+\d+)?$'
    R1-NewTask 'F4 R1 state detail'
    $today=[datetime]::Today
    Click '^统计中心$';Click '^自定义$'
    $start=$today.AddDays(-2)
    R1-Date '开始日期' $start
    R1-Date '结束日期' $today
    $range=R1-Range '自定义' $start $today $start.AddDays(-3) $start.AddDays(-1) '前一等长周期'
    R1-WriteTree 'statistics-custom-before'
    R1-Open '待办队列' 'F4 R1 state detail'
    Click '^关闭$';Click '^统计中心$'
    [void](R1-Range '自定义' $start $today $start.AddDays(-3) $start.AddDays(-1) '前一等长周期')
    R1-WriteTree 'statistics-custom-restored'
    Screen 'statistics-custom-restored'
    $range
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
      R1-NewTask 'F4 R1 DPI'
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

function R1-DpiExtended {
  Step 'r1-dpi125-apply' {
    $value=Set-HostedDisplayScale 125
    $rect=[Windows.Automation.AutomationElement]::FromHandle($window).Current.BoundingRectangle
    if($value -ne 120 -or $rect.IsEmpty){throw '125 percent was not measured on the actual app'}
    @{scale=125;windowDpi=$value;size="$($rect.Width)x$($rect.Height)"}
  }
  if($records[$records.Count-1].status -ne 'PASS'){return}
  try{
    Step 'r1-dpi125-timeline-and-statistics' {
      R1-NewTask 'F4 R1 DPI work'
      R1-Open '待办队列' 'F4 R1 DPI work'
      Click '^本轮已处理$'
      R1-Open '暂缓事项' 'F4 R1 DPI work'
      $label='已记录本轮处理，事项进入暂缓队列'
      R1-KeyToggle $label
      [void](Get-Control '退出 \d{4}-\d{2}-\d{2} 队列：\d+（本轮已处理）' 'Text')
      R1-WriteTree 'dpi125-timeline'
      Screen 'dpi125-timeline'
      R1-KeyToggle $label
      Click '^关闭$';Click '^统计中心$'
      foreach($preset in '本周','上周','本月','上月','本季度','上季度','自定义'){Click ('^'+$preset+'$')}
      Click '^本周$'
      [void](Get-Control '^工作量趋势$' 'Text')
      R1-WriteTree 'dpi125-statistics-complete-controls'
      Screen 'dpi125-statistics-complete-controls'
      @{timelineExpanded=$true;sevenStatisticsControls=$true;windowDpi=[InlineUiWindow]::GetDpiForWindow($window)}
    }
    Step 'r1-dpi125-backup-conflict' {
      try{Click '^关闭$'}catch{}
      Click '^待办队列(?:\s+\d+)?$'
      R1-NewTask 'F4 DPI conflict source' 'before'
      Click '^软件设置$';Click '^立即备份$'
      [void](Get-Control '^备份完成：InLine-backup-.*-manual\.db$')
      $backupDir=Join-Path (Split-Path $db) 'backups'
      $items=@(Get-ChildItem -LiteralPath $backupDir -Filter '*-manual.db' -File | Where-Object Length -GE 4096)
      if($items.Count -ne 1){throw 'Expected exactly one completed manual backup'}
      $source=Join-Path $lab 'dpi125-source.db'
      Copy-Item -LiteralPath $items[0].FullName -Destination $source
      $backupHash=(Get-FileHash -LiteralPath $source).Hash
      R1-Open '待办队列' 'F4 DPI conflict source'
      Click '^编辑$';Fill '^(记录判断|内部备注)' 'after';Click '^保存修改$'
      try{Click '^关闭$'}catch{}
      Click '^软件设置$';Click '^导入并恢复$'
      Dialog 'open' $source
      Confirm-RestoreDialog
      [void](Get-Control '^发现 1 项导入冲突$')
      [void](Get-Control '^F4 DPI conflict source（冲突） ' 'Button')
      Screen 'dpi125-conflict-dialog'
      R1-WriteTree 'dpi125-conflict-dialog'
      Click '^知道了，稍后处理$'
      [void](Get-Control '^备份数据已成功导入$')
      if((Get-FileHash -LiteralPath $source).Hash -ne $backupHash){throw 'Source backup changed'}
      Click '^关闭通知$'
      @{nativeFilePicker=$true;conflictReview=$true;sourceBackupSha256=$backupHash}
    }
    Step 'r1-dpi125-approval-controls' {
      Click '^软件设置$'
      Invoke-Approval 'seed'
      Invoke-Approval 'approve'
      $section=Get-Control '^批量操作' 'Button'
      $expand=$null
      if(!$section.TryGetCurrentPattern([Windows.Automation.ExpandCollapsePattern]::Pattern,[ref]$expand) -or
         $expand.Current.ExpandCollapseState -ne [Windows.Automation.ExpandCollapseState]::Expanded){Click '^批量操作'}
      Click '^查看整批影响$'
      [void](Get-Control '2030-01-0[34]')
      [void](Get-Control '^审阅并确认$' 'Button')
      [void](Get-Control '^拒绝$' 'Button')
      Screen 'dpi125-approval'
      R1-WriteTree 'dpi125-approval'
      Click '^审阅并确认$'
      [void](Get-Control '^确认整批执行$' 'Button')
      Screen 'dpi125-approval-confirmation'
      @{approvalDetails=$true;approveRejectButtons=$true;confirmationControl=$true;decision='left pending in disposable runner'}
    }
  }finally{
    Step 'r1-dpi125-restore-system' {
      $reset=Set-HostedDisplayScale 100
      if($reset -ne 96){throw '100 percent was not restored'}
      @{restoredWindowDpi=$reset}
    }
  }
}
