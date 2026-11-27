function R1-WriteTree([string]$label){
  $items=foreach($node in (Children)){
    try{
      $v=$node.Current
      if($v.Name){@{name=$v.Name;type=$v.ControlType.ProgrammaticName;id=$v.AutomationId;visible=!$v.IsOffscreen}}
    }catch [Windows.Automation.ElementNotAvailableException]{}
  }
  ConvertTo-Json -InputObject @($items) -Depth 4 | Set-Content -LiteralPath "$Output/$label.json" -Encoding utf8
}
function R1-Read([string]$mode,[string[]]$inputs){
  $value=& python -X utf8 (Join-Path $PSScriptRoot 'f4-r1-fixtures.py') $db $mode @inputs
  if($LASTEXITCODE){throw "Synthetic SQLite check $mode failed with exit $LASTEXITCODE"}
  if($value){ConvertFrom-Json -InputObject ($value -join [Environment]::NewLine)}
}
function R1-Visible([string]$name,[string]$role,[switch]$IncludeScrolled){
  @(Children|Where-Object {
    try{
      $n=$_.Current
      $n.Name -ceq $name -and (!$role -or $n.ControlType.ProgrammaticName -eq "ControlType.$role") -and ($IncludeScrolled -or !$n.IsOffscreen)
    }catch [Windows.Automation.ElementNotAvailableException]{$false}
  })
}
function R1-One([string]$name,[string]$role){
  [void](Get-Control ('^'+[regex]::Escape($name)+'$') $role)
  $found=@(R1-Visible $name $role -IncludeScrolled)
  if($found.Count -ne 1){throw "Expected one visible $role named $name, found $($found.Count)"}
  $found[0]
}
function R1-Raw([bool]$expanded){
  $until=[datetime]::UtcNow.AddSeconds(5)
  do{
    $current=@(R1-Visible '原始记录' '' -IncludeScrolled | Where-Object {$_.Current.ControlType.ProgrammaticName -in 'ControlType.List','ControlType.Group'})
    if(($expanded -and $current.Count -eq 1) -or (!$expanded -and !$current.Count)){return}
    Start-Sleep -Milliseconds 150
  }while([datetime]::UtcNow -lt $until)
  throw "Raw log list visibility differs from expected expanded=$expanded"
}
function R1-KeyToggle([string]$label){
  $item=R1-One $label 'Button'
  Set-InputFocus $item
  $focused=[Windows.Automation.AutomationElement]::FocusedElement
  if(!$focused.Equals($item)){throw 'Target summary did not receive keyboard focus'}
  [Windows.Forms.SendKeys]::SendWait('{ENTER}')
}
function R1-Confirm([string]$message){
  $end=[datetime]::UtcNow.AddSeconds(7)
  do{
    $front=[InlineUiWindow]::GetForegroundWindow()
    [uint32]$owner=0
    [void][InlineUiWindow]::GetWindowThreadProcessId($front,[ref]$owner)
    if($owner -eq $process.Id){
      $scope=[Windows.Automation.AutomationElement]::FromHandle($front)
      $all=$scope.FindAll([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.Condition]::TrueCondition)
      if(@($all|Where-Object {$_.Current.Name -eq $message}).Count){
        $ok=@($all|Where-Object {$_.Current.ControlType -eq [Windows.Automation.ControlType]::Button -and $_.Current.Name -match '^(确定|OK|Yes|是)$' -and !$_.Current.IsOffscreen})
        if($ok.Count -ne 1){throw "Expected one confirmation button, found $($ok.Count)"}
        $ok[0].GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern).Invoke()
        return
      }
    }
    Start-Sleep -Milliseconds 150
  }while([datetime]::UtcNow -lt $end)
  throw "Owned confirmation unavailable: $message"
}
function R1-Select([string]$control,[string]$option){
  $combo=Get-Control $control 'ComboBox'
  Set-InputFocus $combo
  [Windows.Forms.SendKeys]::SendWait('%{DOWN}')
  $parent=[Windows.Automation.AutomationElement]::FromHandle([InlineUiWindow]::GetForegroundWindow())
  $list=$parent.FindAll([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.Condition]::TrueCondition)
  $entry=@($list|Where-Object {$_.Current.Name -match $option -and -not $_.Current.IsOffscreen -and $_.Current.ControlType.ProgrammaticName -in 'ControlType.ListItem','ControlType.MenuItem'})
  if($entry.Count -ne 1){throw "Expected one option $option, found $($entry.Count)"}
  $entries=@($list|Where-Object {!$_.Current.IsOffscreen -and $_.Current.ControlType.ProgrammaticName -in 'ControlType.ListItem','ControlType.MenuItem'})
  $position=-1
  for($i=0;$i -lt $entries.Count;$i++){
    if($entries[$i].Equals($entry[0])){$position=$i;break}
  }
  if($position -lt 0){throw 'Cannot resolve exact popup option position'}
  [Windows.Forms.SendKeys]::SendWait('{HOME}')
  if($position){[Windows.Forms.SendKeys]::SendWait("{DOWN $position}")}
  [Windows.Forms.SendKeys]::SendWait('{ENTER}')
}
function R1-Date([string]$name,[datetime]$date){
  $stem=[regex]::Escape($name)
  foreach($field in @(@('Month|月',$date.ToString('MM')),@('Day|日',$date.ToString('dd')),@('Year|年',$date.ToString('yyyy')))){
    $target=Get-Control ("^(?:"+$field[0]+") "+$stem+"(?:\s*\*)?$") 'Spinner'
    Set-InputFocus $target
    [Windows.Forms.SendKeys]::SendWait($field[1])
  }
}
function R1-TaskForm([string]$title,[string]$note=''){
  Click '^新增取号'
  $contact=$title -replace '^F4 R1 ','R1 '
  foreach($pair in @(@('^(输入或选择部门|部门 / 团队)','部门 / 团队','R1 Team'),@('^(输入或选择对接人|对接人)','对接人',$contact))){
    $chip='^'+[regex]::Escape(('移除'+$pair[1]+'“'+$pair[2]+'”'))+'$'
    try{Fill $pair[0] $pair[2] $true}
    catch{[void](Get-Control $chip 'Button' 2)}
    [void](Get-Control $chip 'Button')
  }
  Fill '^(输入或选择事项类型|事项类型)' 'F4 UIA Type'
  Fill '^(一句话说明|事项标题)' $title
  if($note){Fill '^(记录判断|内部备注)' $note}
}
function R1-NewTask([string]$title,[string]$note=''){
  R1-TaskForm $title $note
  Click '^保存并取号$'
  [void](Get-Control ('^'+[regex]::Escape($title)+'$'))
}
function R1-Open([string]$tab,[string]$title){
  Click ('^'+[regex]::Escape($tab)+'(?:\s+\d+)?$')
  Click ('^'+[regex]::Escape($title)+'$') ''
  [void](Get-Control ('^复制事项标题：'+[regex]::Escape($title)+'$') 'Button')
}
function R1-Timeline {
  Step 'r1-timeline-fixture' {
    R1-NewTask 'F4 R1 timeline A'
    R1-NewTask 'F4 R1 timeline B'
    R1-Open '待办队列' 'F4 R1 timeline A'
    Click '^本轮已处理$'
    R1-Open '暂缓事项' 'F4 R1 timeline A'
    Click '^关闭$'
    R1-Read 'timeline-seed' @()
    R1-Open '暂缓事项' 'F4 R1 timeline A'
    [void](Get-Control '^R1 独立业务审计$' 'Text')
    Fill '^补充一条普通处理备注$' 'F4 R1 memo'
    Click '^添加$'
    [void](R1-One 'F4 R1 memo' 'Text')
    $rows=R1-Read 'timeline-read' @('F4 R1 memo')
    ConvertTo-Json -InputObject @($rows) -Depth 6 | Set-Content -LiteralPath "$Output/timeline-db-before.json" -Encoding utf8
    R1-WriteTree 'timeline-collapsed'
    @{sqlRecords=$rows.Count;syntheticHistory='v1, v2, audit';guiNote=$true}
  }
  Step 'r1-timeline-keyboard-and-raw' {
    $action='已记录本轮处理，事项进入暂缓队列'
    foreach($label in @($action,'R1 v1 主动作','R1 v2 主动作')){
      [void](R1-One $label 'Button')
    }
    [void](R1-One 'R1 独立业务审计' 'Text')
    if(@(R1-Visible 'R1 独立业务审计' 'Button').Count){throw 'Audit became an automatic group'}
    R1-Raw $false
    R1-KeyToggle $action
    R1-Raw $true
    [void](Get-Control '^退出 \d{4}-\d{2}-\d{2} 队列：\d+（本轮已处理）$' 'Text')
    R1-WriteTree 'timeline-v2-real-open'
    Screen 'timeline-v2-real-open'
    R1-KeyToggle $action
    R1-Raw $false
    foreach($v in 1,2){
      R1-KeyToggle "R1 v$v 主动作"
      R1-Raw $true
      [void](Get-Control "^R1 v$v 队列原文$" 'Text')
      [void](Get-Control '05[/月].*06.*12:32' 'Text')
      R1-WriteTree "timeline-v$v-legacy-open"
      Screen "timeline-v$v-legacy-open"
      R1-KeyToggle "R1 v$v 主动作"
      R1-Raw $false
    }
    @{action=$action;detailListsPerExpandedGroup=1;fixtureVersions=@(1,2);keyboard='focus-checked Enter'}
  }
  Step 'r1-timeline-edit-then-delete' {
    $text=R1-One 'F4 R1 memo' 'Text'
    $walker=[Windows.Automation.TreeWalker]::ControlViewWalker
    $node=$text
    $found=$false
    for($level=0;$level -lt 5 -and $node;$level++){
      $node=$walker.GetParent($node)
      if(!$node){break}
      $actions=$node.FindAll([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.Condition]::TrueCondition)
      $edit=@($actions|Where-Object {$_.Current.Name -eq '编辑' -and $_.Current.ControlType.ProgrammaticName -eq 'ControlType.Button'})
      $delete=@($actions|Where-Object {$_.Current.Name -eq '删除' -and $_.Current.ControlType.ProgrammaticName -eq 'ControlType.Button'})
      if($edit.Count -eq 1 -and $delete.Count -eq 1){$found=$true;break}
    }
    if(!$found){throw 'Could not uniquely scope actions to the note'}
    $edit[0].GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern).Invoke()
    $editors=@(Children | Where-Object {
      $value=$null
      $_.Current.ControlType.ProgrammaticName -eq 'ControlType.Edit' -and
        $_.TryGetCurrentPattern([Windows.Automation.ValuePattern]::Pattern,[ref]$value) -and $value.Current.Value -ceq 'F4 R1 memo'
    })
    if($editors.Count -ne 1){throw 'Note editor is not unique'}
    Set-InputFocus $editors[0]
    [Windows.Forms.Clipboard]::SetText('F4 R1 memo edited')
    [Windows.Forms.SendKeys]::SendWait('^a')
    [Windows.Forms.SendKeys]::SendWait('^v')
    Click '^保存$'
    [void](R1-One 'F4 R1 memo edited' 'Text')
    [void](R1-Read 'timeline-read' @('F4 R1 memo edited'))
    Screen 'timeline-note-edited'
    $visibleDelete=R1-One '删除' 'Button'
    $visibleDelete.GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern).Invoke()
    R1-Confirm '删除这条普通处理备注？'
    $end=[datetime]::UtcNow.AddSeconds(5)
    while(@(R1-Visible 'F4 R1 memo edited' 'Text').Count -and [datetime]::UtcNow -lt $end){Start-Sleep -Milliseconds 150}
    $rows=R1-Read 'timeline-read' @()
    if(@(R1-Visible 'F4 R1 memo edited' 'Text').Count){throw 'Deleted note remains visible'}
    ConvertTo-Json -InputObject @($rows) -Depth 6 | Set-Content -LiteralPath "$Output/timeline-db-after.json" -Encoding utf8
    @{edited=$true;deleted=$true;otherRecordsPreserved=$rows.Count}
  }
  Step 'r1-timeline-switch' {
    try{Click '^关闭$'}catch{}
    R1-Open '待办队列' 'F4 R1 timeline B'
    [void](Get-Control '^事项时间线$' 'Text')
    if(@(R1-Visible 'R1 独立业务审计' 'Text').Count -or @(R1-Visible '已记录本轮处理，事项进入暂缓队列' 'Button').Count){throw 'Prior task history is visible after switching'}
    R1-WriteTree 'timeline-other-task'
    Click '^关闭$'
    @{checkedTitle='F4 R1 timeline B';previousLogVisible=$false}
  }
}
