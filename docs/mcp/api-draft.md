# MCP API：P1实际契约与后续草案

> 当前进度：P0已验收并推送固定Tag，C1–C4均A；P1实际实现及未执行项以[PHASE-01](acceptance/PHASE-01.md)和[接入指南](connection-guide.md)为准。下文的P0现状表和未来草案是历史设计，不能视为当前工具清单或已实现能力。P1尚未用户验收。


基线：0.5.0 / `41383aa`。P0设计已冻结，后续工具草案不代表程序已经提供；P1真实接口以本文末节及接入指南为准。既有两只读工具的业务含义保留，新认证/限域/100条上限需要迁移说明。

## 1. 连接与发现

```json
{
  "command": "<已安装路径>/in-line-mcp.exe",
  "args": [],
  "env": {
    "IN_LINE_MCP_CLIENT_ID": "<软件内创建的独立客户端ID>",
    "IN_LINE_MCP_TOKEN": "<仅创建或轮换时显示的独立凭证>"
  }
}
```

软件只生成/复制配置，不修改第三方客户端文件。示例没有真实密钥。stdio初始化从环境取得身份并与主服务认证；不要新增让AI自主授权的 `authorize_client` 工具。所有工具（含能力发现/健康/诊断/两旧工具）在执行时检查身份，认证失败时无业务数据。协议级初始化信息只说明程序/协议及安全错误，不能包含数据库路径、客户端授权列表或数据计数。

能力结果包括软件/MCP/协议/schema版本、真实可得commit或null、安装实例非秘密标识、支持功能、当前有效权限与最大页/批次、权限错误分类、破坏变化说明。MCP API版本与软件版本分别标记；不将尚未开发的P3工具声明可调用。

## 2. 工具分组

| 工具草案 | 主要参数/动作 | 权限与返回 | 阶段 |
| --- | --- | --- | --- |
| `get_capabilities` | 无业务参数 | 已认证；自身有效能力/权限，不暴露其他客户端 | P1 |
| `query_tasks` | filters、projection、limit、cursor、includeTrash显式条件 | 常规/完整读投影；事项列表或稳定ID明细，组合条件白名单，无SQL | P2 |
| `query_task_history` | taskId、kinds、limit、cursor、includeVoided | 授权时间线/队列/办理/审计；自由文本和作废详细内容需完整读；检查现状限域 | P2 |
| `get_report_summary` | 保留startDate/endDate，新增可选filters/snapshot | 授权结构化统计、范围/口径/完整性元数据；旧业务含义 | P1–P2/P6 |
| `list_report_items` | 保留startDate/endDate/offset/limit，新增cursor/snapshot/projection | 单页≤100，授权明细；offset仅在受控快照内解释 | P1–P2/P6 |
| `query_work_calendar` | range、limit、cursor | 授权真实历史，只读；不提供创建日历事件 | P2 |
| `manage_saved_query` | list/get/save/delete、name、filter | 查询需读权限；save/delete需明确意图及日常写入授权，不保存扩大权限 | P2 |
| `mutate_task` | action判别联合、target、fieldBase、idempotencyKey、intent/reason | 日常写入；仅白名单领域动作，返回授权过滤后的差异/提交状态 | P3–P4 |
| `preview_batch` | 同mutate_batch请求但无提交 | 已认证及对应写操作/范围授权；只读预演，不分配生产号码/创建业务记录 | P4 |
| `mutate_batch` | operations、stableTargets、planId可选、idempotencyKey | 日常写入；实际受影响事项默认≤20，原子提交，歧义整批暂停 | P4 |
| `manage_preferences` | get/patch、白名单key/value | get按读权限，patch日常写入及明确意图；现有字体/缩放/编号色；禁安全/恢复/更新 | P3 |
| `create_local_backup` | intent、idempotencyKey | 日常写入+明确用户要求；返回非秘密receipt，不返回数据库内容、下载地址或任意文件路径 | P4 |
| `export_business_data` | filter、format、projection、snapshot | 按读权限过滤；明确用户请求；只返回受控任务/结果，不写任意路径、不导出全库 | P4–P5 |
| `query_changes` | mode=events/current、filters、limit、cursor | 授权读，可靠进度、必要无内容脱离标记；单页≤100 | P5 |
| `manage_jobs` | get/list/cancelWaiter/cancelJob、jobId/waiterId | 仅自身任务；取消不撤销已提交写入；领取结果时再鉴权 | P5 |
| `get_data_health` | cursor/snapshot可选 | 已认证、自身授权下只读健康/已知缺口；不能触发业务修库 | P6 |
| `trace_report_metric` | reportId、metric、limit、cursor | 按原快照及当前授权分页溯源；口径/公式/有效事件ID | P6 |
| `compare_report_basis` | 两个reportId/查询定义 | 同客户端授权数据；比较范围/口径/版本/明细，不改数据 | P6 |
| `request_undo` | auditId、reason、idempotencyKey | 日常写入授权，仅创建申请；状态needs_user_approval，不能执行逆向写入 | P3 |
| `diagnose_error` | errorId | 已认证，仅自身脱敏诊断；禁止读取原始任意日志 | P7 |

约20个工具按领域分组，动作使用严格判别联合JSON Schema，避免一个万能执行器或每个字段一个工具。能力清单按实际实施阶段开放；各动作逐次服务端检查权限，不能只依赖tools/list隐藏。

## 3. 统一业务结果壳

```json
{
  "apiVersion": "<实际版本>",
  "requestId": "req_...",
  "executionId": "exec_...",
  "status": "ok",
  "data": {},
  "meta": {
    "dataVersion": {"databaseUuid": "...", "generation": 1, "commitSequence": 42},
    "snapshotId": "snap_...",
    "authorizationRevision": 3,
    "complete": true,
    "coverage": {},
    "knownGaps": [],
    "nextCursor": null,
    "cache": {"reused": false, "sourceJobId": null, "computedAt": null}
  },
  "error": null
}
```

JSON-RPC协议格式/参数解析错误遵循MCP；业务错误返回结构化code/retryable/errorId/recommendedAction，并设合适的工具isError。不把部分结果作为complete=true；异步返回accepted+jobId。敏感字段未授权明确说明projection omitted，不用空字符串冒充原值。

写结果增加 `commitStatus`（not_committed/committed_verified/committed_verification_failed/unknown）、transactionId、auditId、逐项授权差异、noChange原因。已提交但界面通知失败必须保留已提交结论。

## 4. 查询契约

- 白名单过滤：稳定ID/永久编号、文本检索、当前状态、部门/类型、优先级/工作量/加急、截止范围、创建/计划/办理时间、父子结构/父ID、归档/回收站/未来状态；复杂组合受深度/条件数量/成本限额，不执行任意SQL。
- 默认排除回收站、作废办理；明确请求且获得完整读取时可查详细历史，常规汇总按有效事件计算。归档历史有效事件继续计入，不按当前归档状态抹去过去办理。
- 单页统一≤100；超限返回明确PAGE_LIMIT_EXCEEDED及迁移指引，不静默用500或未经说明截断。全历史通过分页/后台任务，不受旧报告371天规则统一截断。
- 日期使用显式时区/半开范围规范化；报告自然日endDate含当日。统计与UI共用计算函数；区间最后有效结果、趋势桶去重、多部门可重复计入、当前类型/部门归属须返回口径。
- 稳定分页cursor绑定client/filters/projection/授权/快照/代际，拒绝跨客户端、参数变更、篡改、过期及历史缺口。offset与cursor不能混用；旧offset首次创建快照且后页必须带snapshot，否则明确需重新查询。
- 单任务定位可提供受限target候选检索，但最终写入必须精确ID或可验证唯一标识；多个候选返回needs_disambiguation。语义分析由AI在授权数据上完成，API不把推测保存为业务关联。

## 5. 写入契约

```json
{
  "idempotencyKey": "<本客户端稳定请求键>",
  "action": "patch",
  "target": {"taskId": 123},
  "fieldBase": {"title": "原标题", "titleVersion": 8},
  "patch": {"title": "新标题"},
  "intent": {"kind": "explicit_user_request", "summary": "用户要求修改标题"},
  "reason": "按用户提供的准确名称修改"
}
```

- patch只提交明确变更字段，省略与清空不同；详情/备注优先指定片段及版本，整体替换需明确意图。自由文本解释不是授权令牌。
- action白名单：create/patch/setStatus/setUrgent/recordWorkEvent/requestUndo，P4补queue/replan/createSubtask/setParent/detachParent/reorderSubtasks/archive/trash/restore/correctWorkEvent/voidWorkEvent。每种动作独立Schema、精确作用范围、域校验，不接受调用任意Tauri命令。
- 请求字段及派生结果均按权限过滤。仅有写权限却无完整读权限时，不能通过Dry Run/audit/beforeValue回显已有正文或联系人；客户端可写其明确提供的字段，但只返回允许读的投影和变化字段名称。
- 状态切换按现有规则真实产生办理/队列事件；纯备注不生成办理量；新增工作活动需用户明确真实发生时间/结果，不能由AI虚构工作。
- 事件纠错不覆盖原事件：作废原事件＋新更正事件＋correctionOf/解释/审计，遵守首次有效事件统计保护；精确事件ID，不允许静默改历史。已有void_work_event确认逻辑保留。
- 归档、回收站、改期、批量目标及子任务范围须有明确意图；批次任何歧义、超范围、字段冲突全批不提交。改期是否允许批量及二次确认渠道已按C2=A冻结：整批由应用内确认。
- 批量新建子任务可使用批内localRef，但先解析所有关系并校验两级，计入实际受影响数量；提交前全批重新校验版本/授权/预演基准。
- backup只创建本地完整受保护业务备份，不读取/恢复/覆盖/删除；备份不是事项批次中的一个可原子混合操作，单独执行并提供幂等receipt，不声称文件和SQLite事务自动原子。
- 永久删除、清空回收站、恢复/覆盖库、自动授权/升级/下载附件、任意文件读写没有工具或动作，枚举和分发双重拒绝。

## 6. 主要错误类别

| code示例 | 行为 |
| --- | --- |
| AUTH_REQUIRED / INVALID_CREDENTIAL / CLIENT_REVOKED / CREDENTIAL_ROTATED | 无业务数据，指引用户配置/轮换；AI不能自行解封 |
| PERMISSION_DENIED / SCOPE_DENIED / MCP_PAUSED | 不执行，停止任务相关读取，重新取结果仍鉴权 |
| INCOMPATIBLE_VERSION / PAGE_LIMIT_EXCEEDED / UNSUPPORTED_ACTION | 版本/迁移/限制明确；不接受旧参数绕过 |
| NEEDS_DISAMBIGUATION / EXPLICIT_INTENT_REQUIRED / USER_APPROVAL_REQUIRED | 等待明确目标/意图/软件内批准，整批不写 |
| FIELD_CONFLICT / DOMAIN_RULE_VIOLATION / PLAN_CHANGED | 返回允许查看的差异及规则，不能自动改用户决议 |
| IDEMPOTENCY_KEY_REUSED / OUTCOME_UNKNOWN / COMMITTED_VERIFICATION_FAILED | 查账本或只读核验，禁止盲目补写 |
| RATE_LIMITED / RESOURCE_LIMIT / CANCEL_PENDING / JOB_EXPIRED | 返回retryAfter/期限，守30分钟/24小时边界 |
| SNAPSHOT_EXPIRED / CURSOR_INVALID / DATA_GENERATION_CHANGED / HISTORY_GAP | 终止旧基准或重同步，只读完整重查最多一次 |
| BUSY / TEMPORARY_IO / STORAGE_FULL / SECURITY_STORE_UNAVAILABLE | 有界低风险恢复；安全存储不可用fail closed，不碰业务修复 |

上述枚举在P1规范化并逐阶段补全。绝不把错误建议当作AI可以执行危险恢复的授权。

## P1实际接口（覆盖上文同名草案）

仅有`get_capabilities`、`get_report_summary`、`list_report_items`，每次认证。参数仅现有startDate/endDate/offset/limit，不接收filters/snapshot/cursor。返回`{apiVersion:1,requestId,status,data,error}`；没有executionId/meta/快照完整性声明。API v1错误用小写code及message/retryable/retryAfterSeconds；JSON-RPC/schema错误由rmcp处理。业务错误同时设置工具isError=true，成功isError=false；structuredContent和文本均含同一结果壳。P2前维持371天范围和非稳定offset分页。

能力结果含softwareVersion、mcpVersion、schemaVersion、commit（没有可信构建SHA时null）、transport、自身permissions/scope/authorizationRevision、实际tools/writeTools、unsupported、breakingChanges、pageLimit、reportMaxDays。不会暴露他人客户端列表。所有新工具草案仍未实现；完整执行/快照/诊断元数据逐所属阶段补齐。
