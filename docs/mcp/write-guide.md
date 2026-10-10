# P3 单事项写入与撤销（本地开发，待统一验收）

用户2026-10-10最新授权：各阶段分别开发提交并推送，P8统一验收；P1–P7保持待验收，不创建accepted阶段Tag，生产安装及Release另获授权。下文旧提交限制保留为历史。

2026-10-10。用户允许继续开发后统一验收；P1/P2/P3不因此视为验收通过。软件0.5.0、API1、业务schema12，新增三个工具后共10个工具。Codex自动导入/重载入口仍关闭；没有自动修改本机客户端配置、授权或生产数据。

## 权限和定位

所有调用先独立认证，沿用全局/客户端权限交集和全部部门AND类型限域。写入须常规读取+日常写入；完整读取独立控制敏感字段。未认证、暂停、撤销或越权拒绝，返回前仍复核授权。一次正常授权不逐笔弹窗；明确用户意图不是权限令牌。

`mutate_task`动作白名单为create、patch、setStatus、setUrgent、recordWorkEvent；P4的排队、改期、子任务、归档、回收站、纠错和批次尚未提供。禁止任意SQL/Tauri命令、永久删除、数据库覆盖恢复、附件及任意文件访问。只用一个target定位项：taskId、permanentNumber或精确title；标题多候选返回needs_disambiguation，由AI使用授权查询给出候选并请用户指定ID。

## 请求示例

先用query_tasks读取当前字段与fieldVersions/taskVersion。提交局部编辑：

```json
{
  "action": "patch",
  "target": {"taskId": 123},
  "fieldBase": {"title": {"value": "原标题", "version": 8}},
  "patch": {"title": "新标题"},
  "idempotencyKey": "本客户端稳定请求键",
  "intent": {"summary": "用户要求修改标题", "explicitUserRequest": true},
  "reason": "按用户提供的准确名称修改"
}
```

patch白名单：title、departments、contacts、taskType、details、internalNotes、priority、workload、requestedDeadline、requestedDeadlineLabel。省略字段保持不变；截止字段null代表清空。字段原值和独立版本必须同时匹配；其他字段已变化可以合并，同字段变化（包括改走后改回的ABA）拒绝。同一主程序数据库互斥与事务也协调GUI修改。

详情/备注优先使用`textEdits:[{field,find,replace}]`，find须在当前文字中精确且唯一；仍提供相应fieldBase，patch不要重复该字段。整段替换须intent.replaceWholeText=true。自由文本由AI按用户明确事实归类：事项内容放details，内部提醒放internalNotes，真实办理才用recordWorkEvent；不能猜联系人、截止日期或虚构已完成工作。

create接受task：title/departments/contacts/taskType必填，details/internalNotes可空，priority/workload/requestedDeadline可选。状态固定pending，其他值走现有领域校验。原同名且在授权范围的事项返回possible_duplicate；AI仍须先检索语义近似候选，由用户明确允许重复时才设置intent.allowPossibleDuplicate=true。不自动合并。部门、联系人、类型仅随明确事项新增缺失值，不提供重命名、删除、排序或合并字典的工具。

setStatus/setUrgent/recordWorkEvent须taskVersion。setUrgent参数为isUrgent、requester、urgentReason（与顶层操作reason分开）。recordWorkEvent须resultStatus/handledAt（RFC3339）/note/syncStatus，明确真实办理设置intent.confirmedRealWork=true；会产生真实办理记录的状态操作也必须确认此标记。已经是目标状态或相同加急信息时返回no_change，不产生新业务事件/统计。

## 提交、回执和重试

主程序共享五个现有GUI领域函数，MCP不另写状态/编号/队列/统计规则。领域变更、自动字典新增、AI前后审计及幂等回执处于同一SQLite事务；任何一处失败全部回滚。加急可能按原规则调整相邻顺序，所有实际影响事项也复核限域并纳入审计。

幂等键绑定客户端和完整规范化请求；同键同请求返回旧回执，同键异请求返回idempotency_conflict，不重做。回执在重启后仍存在，不自动清理或随缓存删除。权限和当前事项范围先复核；跨客户端不共享回执。回执只返回taskId/auditId/变化字段名/版本及提交、核验状态，不回显已有联系人、正文、备注或前后值。

提交后保持同一数据库互斥，再独立读取关键版本/偏好验证。commitStatus=committed与verificationStatus分开；verification_failed意味着已提交但核验异常，不能补写。传输在可能送出写请求后中断/超时返回result_unknown；没有自动重放。授权在提交后变化也可能隐藏结果，必须保持原幂等键，不能换键重做。允许有限只读核验/刷新（最多两次），权限和业务冲突不自动重试；P5在途任务与可靠恢复尚未实现。

## 审计与用户批准撤销

AI审计长期保存在业务库mcp_ai_audit，保留客户端来源、动作、理由、明确意图、时间、前后状态和撤销关联；不保存长期Token。technical诊断没有替代业务审计，纯编辑不会虚增办理量。query_task_history的audit来源只提供授权范围内摘要；原因文本需要完整读且不限域，限域客户端省略可能含其他事项信息的自由文本。

request_undo只接受本客户端auditId、reason、intent、idempotencyKey，生成needs_user_approval。AI没有approve/executeUndo工具。用户在设置的“AI操作记录与撤销申请”审阅原操作前后变化，再明确批准或拒绝。执行前在事务中检查现状和版本；同字段冲突整体停止，局部编辑撤销保留无关字段的新修改。

撤销为有据可查的补偿操作：新建事项撤销移入回收站；新增办理保留原行并作废；状态走原领域规则恢复，重新入队按既有规则重新取号，不回收旧号；未来事项重新预占，已过期计划按领域规则拒绝。加急相邻顺序也检查现状并记录恢复。原审计、状态/队列/办理历史不删除；补偿本身另记approvedUndo审计。改变首条有效办理的统计影响在软件内批准提示中披露。

## 普通偏好

manage_preferences：get返回白名单实际保存值（未设置为null）和fieldVersions；patch提交patch、fieldBase、fieldVersions、idempotencyKey、intent、reason。每项原值和版本一致才提交，多项同一事务。白名单仅ui_font_family、ui_scale、ticket_colors、show_deferred_in_queue、week_start_day、statistics_rate_mode；沿用现有值校验，不新增暂缓的主题组合。禁止安全授权、恢复、更新、路径、快捷键及自启动设置。审计及用户批准撤销同样适用。

## 升级与验收边界

schema11升级12前自动完整备份；增加事项/字段/偏好版本、AI审计、幂等回执和撤销申请，永久编号及既有数据保持。字段/事项版本由GUI和MCP共用触发器维护。权限库仍独立，业务备份恢复不激活授权；恢复合并不导入另一库的幂等键或客户端权限。

自动测试及本地构建不代表安装/升级和真实客户端验收。生产安装、旧数据实际升级、当前Codex认证与自动重载、UI审阅观感均留待用户统一验收。当前阶段提交推送、验收Tag及Release均未授权。
