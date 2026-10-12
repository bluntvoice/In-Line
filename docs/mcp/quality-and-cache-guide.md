# P6 数据健康、统计依据与报告缓存（开发，待P8验收）

版本0.5.0，业务schema14，15个MCP工具。独立认证、范围限制和全局暂停始终生效；Codex自动接入仍关闭。P6不提供AI业务修复。

## inspect_data

常规读取权限即可使用以下只读操作；未知参数/操作拒绝。

| action | 参数 | 返回 |
| --- | --- | --- |
| health | 可选snapshot、增量cursor | 派生索引/触发器连续性、数据代际、当前客户端快照/游标状态及缓存技术状态，不含事项正文或他客户端信息 |
| trace | report含原snapshot；metric；可选dimension、periodStart、cursor | 原报告冻结基准上的实际事项、有效办理事件ID和队列区间、指标权重与口径；每页1–100条 |
| compare | report和right两个报告定义，可含各自snapshot | 同客户端的范围、时区、条件/权限、口径、数据基准和统计差异；返回各自快照以继续查明细 |
| cache | 无额外参数 | 本客户端报告缓存空间、来源快照、计算时间及待请求校验状态 |
| recompute | report，不含snapshot/cursor/offset | 按当前权限及基准重新计算，不复用报告缓存、不修改原记录 |

示例：先get_report_summary取得meta.snapshotId，然后：

```json
{"action":"trace","metric":"completionRate","report":{"startDate":"2026-08-08","endDate":"2026-08-08","timezoneOffsetMinutes":480,"snapshot":"原报告snapshotId","limit":100}}
```

下一页只补顶层cursor，原report、metric、dimension、periodStart、limit保持一致；不要使用report.cursor/offset。分页游标按客户端、授权、原报告条件及指标绑定；原快照600秒失效或权限/索引代际变化须重新查询。正常业务修改后的旧报告仍明确代表原冻结基准，不冒称最新。

指标支持summary（可省前缀）的handledTasks、topLevelTasks、subtasks、processed、completed、waitingMaterials、waitingConfirmation、waitingCounterpartyConfirmation、rateNumerator、rateDenominator、completionRate；byTaskType/byDepartment的handledTasks/completed/pendingFollowUp需dimension精确值；trend的handledTasks/processed/completed需原报告已有periodStart。不存在的维度/趋势桶不猜测。

统计中心和溯源共用有效事件最后结果/入队资格SQL。办理率分母为范围内符合截止条件的在队事项与有效办理事项的去重并集；办结率分母为有效办理事项。趋势按原报告日/周桶独立取最后有效结果。多部门计数可重叠，归属按快照捕获时现值。来源不返回联系人、详情、内部备注、办理备注、队列关闭自由文本或范围外父事项编号。原记录缺失无法重建；complete表示计算/来源集合完整，历史完整性另标unknown，分页仍以nextCursor为准。

## 缓存与复用

get_report_summary新增explicitRequery=true显式重算。未提供时区时先解析当前本机偏移并纳入缓存/后台任务身份，后台恢复继续使用原时区。内存缓存TTL60秒、总8MiB、最多16条/每客户端4条、单条2MiB，超过单条容量仍直接返回已计算结果；请求输出仍有7MiB上限。先自动淘汰，持续容量旁路在本地轻量提示。客户端/授权/范围/查询/数据基准及统计定义完全一致且原快照有效才复用。计算时间、来源快照、校验时间/状态结构化返回。

P5已完成只读任务10分钟内可按同客户端、参数、口径、授权及当前基准复用，明确重查例外；返回sourceJobId/computedAt/sourceSnapshotId/validatedAt。7天内取原任务结果可显式代表已授权历史基准，状态标authorized_historical_basis，不作为当前完成结果复用。授权、数据UUID/代际、索引epoch及现时范围始终重查。重启不会恢复报告内存缓存。

设置→报告缓存与数据健康：查看空间/最近计算、刷新状态、清理报告缓存。清理只释放RAM报告条目，保留分页快照、任务检查点、审计和正式导出；任务自身7/30天清理另有入口。没有证据证明需要报告磁盘缓存，本阶段不启用磁盘层。既有私有检查点仅使用Windows当前用户ACL，同账户进程仍可访问；此规则不改变DPAPI凭证保护。

## 派生索引恢复边界

健康检查不写库。发现必要派生表/索引/30个预期触发器/事件计数旁证异常时，启动或本地明确点击可在独立事务重建派生索引，验证全部定义及权威UUID/代际/提交序号不变后提交；校验或DDL失败整笔回滚。只从当前水位开始新索引epoch，旧增量游标强制全量重同步；不补造缺失事件，不改业务记录、号码、AI审计、业务版本、授权及凭证。无法安全处理则保留异常，提示本地排查。

源文件和阶段证据见[PHASE-06](acceptance/PHASE-06.md)。隔离测试不等于生产安装、客户端实机接入或用户统一验收。
