# CLI 完整命令体系草案

日期：2026-10-16。状态：**用户已确认Phase0及D1–D4全部选A，全部命令尚未实现**。架构、权限、阶段门禁见 [Phase0报告](PHASE-00-ARCHITECTURE.md)。本文件为已确认方案下的命令设计草案，具体实现需逐阶段验收，不自动进入开发。

## 1. 名称和调用方式

用户已通过D2-A确认 binary 为 `in-line-cli.exe`，文中简写 `in-line-cli`。现有 `in-line.exe` 继续是GUI，`in-line-mcp.exe` 继续是stdio服务；三者不互相通过shell包装业务。

本机 PowerShell `Get-Command inline,in-line,in-line-cli -All` 未发现当前PATH下同名命令；不代表其他电脑或注册表不存在冲突。外部官方包索引已有 [PyPI inline](https://pypi.org/project/inline/) 项目，说明通用命名有重名风险，但**包名重名不等于已经证实其安装了同名exe**。检索还见 [npm inline-js](https://www.npmjs.com/package/inline-js)，其实际命令为inlinejs，不能据此误称inline.exe冲突。

因此采用更明确的 `in-line-cli.exe`，首版通过安装目录/完整路径调用，不自动写系统或用户PATH、不覆盖已有别名；该命名及PATH策略已按D2-A确认。没有对所有包管理器或商标做全面排重，不声称全球唯一。

PowerShell中文空格路径示例（未来实现后）：

```powershell
& 'D:\Application\In Line\in-line-cli.exe' task list --format json --limit 100
```

## 2. 通用参数与输入

| 参数 | 提议语义 |
| --- | --- |
| `--help` / `--version` | 只读本地元数据；不启动服务、不需要业务授权 |
| `--format table\|json` | 默认table；Agent显式json。UTF-8；JSON模式stdout恰好一个Envelope，日志/进度进stderr |
| `--no-color` | 禁止ANSI/终端装饰；JSON始终无颜色 |
| `--profile <name>` | 显式选已授权的本地CLI身份；不得在找不到时回退到其他更大权限身份 |
| `--input <file\|->` | 复杂业务JSON；`-`从stdin读；拒绝未知字段、超限体积和非法UTF-8，Windows BOM有明确兼容测试；不解释表达式/命令 |
| `--limit 1..100` | 一页最多100；超限报参数错误，不静默截断 |
| `--cursor` / `--snapshot` | 完全采用已有绑定/失效规则；cursor不打印到装饰性提示中 |
| `--all` | 明确请求逐页读取，保持同一快照及授权；初版有资源上限，超限或失效不能把部分结果说成完整 |
| `--execute` | 写命令必须明确执行；不省略版本、幂等、意图或服务端审批；无参数不得写入 |
| `--idempotency-key` / `--reason` | 写命令必填且保存到外部回执；客户端超时不能自动换键 |

普通查询提供常用flag与等价JSON输入；同一字段同时两处指定时拒绝，不能悄悄覆盖。常用过滤包括ids、永久编号、text、状态、部门、类型、优先级、工作量、加急、未来、活动队列、parent/structure/archive及各类日期；具体白名单来自QueryFilters。联系人/内部文本不得为“方便搜索”越过fullRead。

初版不提供通用`--sort`、任意SQL、任意数据库路径、`--force`或可执行脚本扩展。父子排序和队列move是领域操作，不是查询页内客户端重排。自动化不弹CLI确认对话，不读取无期限交互输入；必要批准留软件内。

## 3. 完整命令清单及服务映射

阶段数字指CLI Phase。表内“新增”是需要开发的适配/共享只读能力；不表示已有同名工具。业务传输只经主程序，不从CLI调用GUI自动化。

| 命令 | 底层能力/语义 | 阶段 |
| --- | --- | --- |
| `--help`、`--version` | 新增本地帮助和构建元数据 | 2 |
| `doctor` | 新增只读环境/安装构建/配置存在性检查；不读Token、不修复、不启动或迁移业务库 | 2 |
| `status` | 主程序/IPC状态及已授权get_capabilities；未认证仅提供安全本地状态并返回非0，不能显示“已连接” | 2 |
| `capabilities` | get_capabilities，有效客户端身份/权限/范围及能力；不报告其他客户端 | 2–3 |
| `client import --package <id>` | 新CLI用途一次性包；ID不是长期Token，禁止授权/增权；需要GUI先明确授权 | 2–3 |
| `client status` | 只读当前显式profile；不列出其他身份或Token | 2–3 |
| `task list` | query_tasks，默认授权范围，默认不含回收站 | 3 |
| `task get <id>` | 精确ID查询；无权/不存在按现有不泄露策略；不猜同名目标 | 3 |
| `task search <text>` | query_tasks白名单检索；同条件分页，不用本地扫DB | 3 |
| `task history <id>` | query_task_history；事件种类/作废/回收站权限沿用 | 3 |
| `task ticket <id>` | query_tasks中现有编号/计划/队列/轮次字段的只读投影，不导出图片 | 3 |
| `task children <parent-id>` | query_tasks parentTaskId过滤 | 3 |
| `task parent candidates <id>` | 受限域合法父任务候选查询适配，不能直接暴露GUI无范围查询 | 4 |
| `task create` | mutate_task/Create；不猜部门/联系人/截止时间；近似重复返回需消歧 | 4 |
| `task update <id>` | Patch；fieldBase/fieldVersions及局部文本替换；不全对象覆盖 | 4 |
| `task status set <id> <status>` | SetStatus；合法状态与事件副作用来自domain | 4 |
| `task complete <id>` | 普通事项复用SetStatus；父任务显式完成范围需新增受权应用服务适配，保留现有MCP直接完成保护；不能客户端分步模拟原子组完成 | 4 |
| `task urgent set\|clear <id>` | SetUrgent；设加急的提出人/原因按既有校验 | 4 |
| `task archive <id>` | Archive受控归档；不改回GUI按钮，不与complete混用 | 4 |
| `task trash <id>` | Trash软删除；父子范围明确；没有permanent-delete命令 | 4 |
| `task restore <id>` | Restore只恢复回收站目标；不会自动重开归档/加入队列 | 4 |
| `task reopen <id>` | Queue/reopen；归档/终态重开，按已有取号及截止时间规则 | 4 |
| `task reschedule <id> --date` | Reschedule；版本与编号作废规则；多事项改期仍软件内整批确认 | 4 |
| `task child create <parent-id>` | CreateSubtask；属性默认继承后独立；未来日期必预占 | 4 |
| `task parent set <id> <parent-id>` | SetParent；双方版本、两级/无环/合法父任务及范围 | 4 |
| `task parent clear <id>` | SetParent/null；不改队列/状态/统计 | 4 |
| `task children reorder <parent-id>` | ReorderSubtasks；完整范围及版本，不改队列顺序 | 4 |
| `queue list` | 当前有效活动队列；跨日未完成队列仍按现有GUI规则，不只看计划日 | 3 |
| `queue future` | query_tasks scheduled过滤/计划范围；未来未到期不算办理 | 3 |
| `queue history <id>` | query_task_history队列种类 | 3 |
| `queue enter <id>` | Queue/enqueue；未来提前入队使用现有领域函数，不自造编号 | 4 |
| `queue move <id> up\|down` | Queue/moveUp/moveDown，版本与范围保护 | 4 |
| `work process-round <id>` | Queue/processRound；仅真实办理明确确认 | 4 |
| `work complete-round <id>` | Queue/completeRound；仅真实完成确认 | 4 |
| `work record <id>` | RecordWorkEvent；时间/结果/说明，不猜真实事实 | 4 |
| `work correct <id> --event-id` | CorrectWorkEvent；历史影响确认，作废关联新事件 | 4 |
| `work void <id> --event-id` | VoidWorkEvent；保留审计及统计影响确认 | 4 |
| `calendar list --from --to` | query_work_calendar；真实办理而非未来排程 | 3 |
| `stats summary --from --to` | get_report_summary；原口径与时区 | 3 |
| `report daily\|weekly\|monthly` | 日期便捷入口，服务端统一范围，结构化摘要/明细；非新统计算法 | 3基础/5完整 |
| `report items --from --to` | list_report_items；原快照/100条限制，显式完整性 | 3 |
| `report daily --render text` | 四字段日报组装/渲染新增；标题/口径由D4确认，含联系人需fullRead | 5 |
| `stats trace`、`stats compare` | inspect_data trace/compare，权限和快照基准不变 | 5 |
| `query list\|get\|save\|delete` | manage_saved_query；保存/删除须写权限及明确指令 | 5 |
| `batch preview --input` | batch_mutate/dryRun；不占号、不写业务 | 5 |
| `batch execute --input --execute` | batch_mutate/execute；先预演、带expectedBasis重新核对，默认20/硬限100；审批可能pending | 5 |
| `changes list` | query_changes；先全量再增量；events/current语义不变 | 5 |
| `jobs start\|list\|get\|result` | manage_jobs；仅已有查询/导出任务种类，不调度业务写入 | 5 |
| `jobs cancel-waiter`、`jobs cancel` | cancelWaiter/cancelJob；取消等待不等于终止共享任务，按现有权限/明确意图 | 5 |
| `data health` | inspect_data/health；不修改业务原数据 | 3 |
| `data cache` | inspect_data/cache，只读状态 | 5 |
| `data backup`、`data export` | manage_business_data；服务器受控产物引用，授权/字段/范围和明确意图，不读整个备份回传给AI | 5 |
| `preferences get\|patch` | manage_preferences现有白名单；patch版本/幂等；不修改授权或任意应用设置 | 5 |
| `undo request --audit-id` | request_undo，仅申请；无approve命令 | 4 |
| `receipt get --idempotency-key` | **新增只读服务能力**：仅当前客户端原回执；不存在不触发执行，不能调用原写命令假装“只查” | 4 |
| `diagnostic get <error-id>` | get_diagnostic，只读自己的脱敏诊断 | 3 |

单页查询可用table；嵌套对象table显示关键字段，不丢失JSON内容语义。长任务使用现有jobs结果领取机制，不无界流式堆到stdout。首版不加NDJSON；若以后确需大流式输出，先另定完成/失败及截断协议。

批次默认先预演是在CLI适配器强制调用同一共享预演服务，并携带基准执行；后端仍重新验证。首版不提供跳过预演选项，不把本地预览文件或`--execute`当作服务器批准。

## 4. JSON 和退出码草案

复用现有Envelope语义：`apiVersion/status/requestId/data/error`。公共业务错误code与MCP一致；CLI额外的命令语法诊断只放本地错误detail，不重写服务器错误code。保持MCP原返回结构不变；CLI若增加本地元数据，在自己的文档/版本中定义。

```json
{"apiVersion":1,"status":"ok","requestId":"<id>","data":{"items":[],"nextCursor":null,"meta":{"complete":true}},"error":null}
```

这是结构示意，实际字段以复用的查询响应契约为准；不能在实现中为凑示例自行删改原有业务字段。

| 退出码（提议） | 分类与例子 |
| --- | --- |
| 0 | 已完成且成功；只读查询、预演成功也为0，但保留dry_run；写入需已提交且后置核验成功 |
| 2 | 参数/输入错误：invalid_arguments、非法JSON、未知字段、互斥参数 |
| 3 | 认证/权限：unauthenticated、forbidden、revoked、paused、explicit_intent_required、sensitive_notice_required |
| 4 | 当前授权范围内not_found；不得为区分无权和不存在进行额外越权查询 |
| 5 | conflict、idempotency_conflict、business_rule、possible_duplicate、needs_disambiguation；不自动消歧 |
| 6 | 环境/连接/版本/资源及查询恢复问题：host/security unavailable、incompatible、rate_limited、快照/增量失效、任务期限/结果过期等；error.code保留具体差别 |
| 7 | 未预期内部错误；不得泄露堆栈/SQL/凭证；支持关联脱敏errorId |
| 8 | 请求已收，但仍pending、需要软件内批准或后台未完成；不声称业务完成；可保持status=ok并在data标状态 |
| 9 | 结果不确定：result_unknown、写入期间authorization_changed、提交后核验失败、发出写请求后连接中断；保留原key核验，禁止当普通“可重试连接错误”处理 |

仅在确认写请求尚未发出时，连接失败才归6；发出后无法确定提交结果归9，即使底层传输错误也不能鼓励重新执行。退出码不是完整业务协议，Agent必须同时读error.code、commitStatus和verificationStatus。未识别的成功/提交状态按失败保守处理，不能默认0。

帮助/版本为人类文本；若显式要求JSON则也使用稳定封装。`--render text`与`--format json`互斥，避免一个命令输出两种格式。未知/未实现命令返回参数或unsupported错误，不输出模拟成功。

## 5. 首版不提供的命令

- 永久删除、清空回收站、覆盖恢复、附件、任意SQL/任意路径读取、shell执行、任意HTTP服务。
- 任何授权grant/增权/撤销恢复、软件内审批批准、关闭审计/绕过幂等、自动重放未知写入。
- UI截图/字体下载/托盘/窗口控制、软件更新安装等GUI管理能力；不用CLI架构任务重新扩大范围。
- 事项合并和单事项编号色写入先延后：有GUI能力不意味着已有受权MCP领域动作；若后续需要，先补范围、回执和共享服务方案。
- `receipt get`、四字段日报、CLI授权及命名/打包目前都是待开发能力，不能直接执行此文档中的示例。

## 6. 确认记录及停止点

用户已确认Phase0方案及D1–D4全部选A。当前仅记录并提交推送文档；尚未授权进入CLI Phase1，不新增CLI binary、依赖、命令解析、数据库迁移或MCP协议变更。
