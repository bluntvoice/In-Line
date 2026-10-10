# P2 授权查询指南（本地实现，待验收）

基线76ef34c，软件0.5.0，API v1新增能力，业务schema11。仅stdio，沿用独立客户端授权、全局权限交集、全部部门AND类型限域和返回前授权复核。Codex自动接入仍关闭，本指南不表示当前聊天已认证。

## 真实工具

| 工具 | 参数和结果 |
| --- | --- |
| get_capabilities | 七个真实工具，schema11，限制及当前身份/有效权限；不声明业务写入 |
| query_tasks | filters/projection/limit/cursor/snapshot；稳定ID顺序；每页1—100 |
| query_task_history | taskId/kinds/includeVoided/includeTrash/limit/cursor/snapshot；状态、排队及结束、办理、加急、原日志 |
| get_report_summary | startDate/endDate/filters/snapshot/timezoneOffsetMinutes；结构化统计、范围和口径 |
| list_report_items | 保留startDate/endDate/limit/offset，新增filters/cursor/snapshot/timezoneOffsetMinutes；报告事项及有效办理记录 |
| query_work_calendar | startDate/endDate/filters/timezoneOffsetMinutes/limit/cursor/snapshot；queueTask（区间）及workEvent（实际办理）两种分页记录，汇总直接复用UI计算 |
| manage_saved_query | action=list/get/save/delete、name/filters/projection/explicitIntent；仅本客户端命名查询定义，不保存权限或业务关系 |

## 条件和投影

filters白名单：ids/permanentNumbers/text/statuses/departments/taskTypes/priorities/workloads/urgent/scheduled/activeQueue/structure/parentTaskId/archive/includeTrash/plannedFrom/plannedTo/createdFrom/createdTo/deadlineFrom/deadlineTo/handledFrom/handledTo。同字段候选OR、不同字段AND；部门查询候选不扩大服务端全所属部门范围。所有列表≤100，文本≤400字节，不接受SQL、任意列名或未知参数。parentTaskId查询须父项也在授权范围。

structure=all/topLevel/subtask；archive=all/active/archived，active排除已完成/已取消/显式归档。默认全部有效事项，排除回收站。plannedFrom/To是含两端的YYYY-MM-DD；created/deadline/handled是显式RFC3339半开区间。text常规读取只检索标题和固定编号，完整读取增加正文、联系人和内部备注。

常规读取提供ID、编号、标题、所属部门/类型、状态、工作量/优先级、日期、排队和受权父子基础信息。完整读取额外允许联系人、正文、内部备注、加急申请及办理/日志自由文本。明确请求无权限字段返回forbidden。未请求时敏感字段省略并列入redactedFields，不用空字符串代表原值。未授权父ID不回显，统计仍保留真实子任务分类。

回收站和作废详细读取须includeTrash/includeVoided明确请求及完整读取；普通报表/工作日历始终排除回收站及作废办理。有效归档历史保留。时间线提供已有源记录，不能补造历史业务审计；自由文本日志不转换为结构化事实。

旧relation/merge日志含其他事项标题但没有可核验的历史关联ID。有部门或类型限域的客户端，即便完整读取已开，也省略这类日志text，保留来源/时间和redactedFields；不借日志回显范围外父项或合并来源。不限域完整读取仍可读原文本。

## 快照分页

首次调用返回meta.snapshotId、dataVersion和nextCursor。后页原条件、projection、时区及limit必须保持一致，带cursor；不透明token绑定客户端/查询/权限修订/内存快照位置，拒绝猜测、篡改、串用或更换参数。可重读原snapshot。报告summary返回的snapshot可直接供同条件list_report_items使用，统计及明细处于同一基准。

旧offset首次0或省略可用；offset>0必须附原snapshot，cursor与offset不能混用，offset不再对变化中的生产库分页。这是有意兼容变化。旧客户端须读取新snapshot，否则后页明确invalid_arguments，禁止拼接不同数据基准。

冻结内容和顺序保存在受限内存SQLite映像中；生产复制结束即释放数据库协调锁，后页不保持生产事务。相同查询仅在当前业务变化序号/身份/代际及授权完全相同时复用SQLite映像，不复用已计算的统计值；数据变化后新建快照。之后普通更新不混入旧页；授权变化、撤销、暂停、数据库身份/代际变化，或已选事项/依赖父项离开当前限域，结果拒绝释放。服务重启或10分钟到期需重新开始；不跨进程持久化查询快照。

schema11新增mcp_data_basis（UUID/代际/变化序号），既有GUI/领域/导入的九张数据表由事务内触发器共同维护。commitSequence表示已提交行变化的单调序号，**不是事务个数**；同一事务可增加多个序号，回滚一并回滚。代际初值1；新数据库UUID不同。P5再扩充变更日志和恢复基准，这里不声称已有增量事件。

范围限制：自然日结束含当天；timezoneOffsetMinutes可指定-840—840，未指定采用本机当前偏移，结果回显。跨页固定该偏移；需要历史夏令时分段时应分别查询并保留时区依据。报告/日历范围最多36,501自然日，完整事项和时间线通过分页获取，不再统一截断371天。

资源限制：源库≤32MiB；快照及已序列化结果计量合计≤64MiB（实际对象和复制瞬时内存另有开销），单结果≤16MiB/100,000条、单页响应≤7MiB；每客户端≤4/全局≤16个活动快照，cursor≤4096/快照。超过上限报resource_limit，不静默截断。过期及权限修订失效快照在下次新查询清理；同一快照报告明细可重复读取，无需新建。

meta.complete=false披露旧历史覆盖缺口，不将现存记录齐全等同于所有过去行为齐全；knownGaps说明早于事件跟踪的缺口。快照不复用统计缓存，cache.reused=false。P6再补健康、指标溯源和更细数据覆盖验证。

## 命名查询和语义分析

list/get仅当前客户端，save/delete需常规读+有效日常写权限、explicitIntent=true且用户确有明确要求；该参数不授予新权限。保存在独立安全目录saved-queries.json，仅当前用户ACL、≤1MiB、每客户端≤100个名称，原子替换。与业务备份分离，无凭证及快照内容。get再次校验字段权限，执行返回的filters/projection仍走query_tasks实时鉴权；权限收窄后模板不能绕过限域。

AI可在上述授权数据上比较关联事项和组合统计；analysisPolicy要求事实/推断分开，不保存永久关系。本阶段没有事项修改或调度写工具。

## 升级及剩余验收

生产schema10升级11前按现有路径备份；不改永久编号、队列、历史、配色。旧schema10程序不能直接打开11；程序回退与数据恢复须分别评估，不自动回退数据库。此次仅本地构建及隔离验证；生产安装、用户实际数据升级、当前Codex认证和重载仍待实机验收。
