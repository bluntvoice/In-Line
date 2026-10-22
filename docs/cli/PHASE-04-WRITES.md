# CLI Phase4：事项写入与管理

日期：2026-10-22。状态：**用户已确认阶段4通过，授权提交推送后进入阶段5；实机验证仍留阶段6。**

## 1. 用户授权、基线及规则确认

用户在阶段3报告后回复“先推送，进入阶段4”。已先提交推送阶段3：`a6987f6bae20c718eba4b128e6520aed1f1129f4`，`git ls-remote`核对远端同名分支一致、工作区干净，再开始本阶段。分支 `feat/scheduled-items`；`origin/main`为 `1baac96252af2665274f2a955b3adacfa80519fd`，未合并主分支。

权威来源：[CLI PRD](../prd/cli.md)、[Phase0架构](PHASE-00-ARCHITECTURE.md)、[命令草案](COMMANDS-DRAFT.md)、[当前检查点](CURRENT_TASK.md)。原任务附件为 `C:/Users/KB/.codex/attachments/9d6f9e9e-144a-4c02-807b-646248f67f1b/已粘贴的文本.txt`；压缩后优先回读这些来源。

调查发现草案的恢复语义与真实领域代码不符，已列选项确认。用户选A：**沿用GUI/MCP“从回收站恢复并加入今日队列”，更正草案**；归档事项重新开启仍使用 `reopen`。此选择单独记录于 `evidence/PHASE-04-restore-decision.json`。

实机安装、原生GUI操作与当前聊天MCP接入仍留 CLI Phase6；阶段4不提前增加 Phase5 批量、作业、全文日报、偏好及备份导出命令。

## 2. 实现范围

| 命令 | 实际服务及保护 |
| --- | --- |
| `task create` | 原 `mutate_task/create`；显式提供领域必填字段，新请求先做限域标题候选检索，不猜联系人或其他业务字段 |
| `task update <id>` | 原 patch、fieldBase/fieldVersions、局部 textEdits，不读取新版本后悄悄覆盖 |
| `task status set <id> <status>` | 共享状态应用服务和领域事务；真实办理状态保留确认要求 |
| `task complete <id>` | 普通完成复用原 setStatus；原MCP父任务直接完成保护保留 |
| `task complete <id> --parent-only / --include-eligible-subtasks` | 受限内部适配，明确父任务完成范围，检查父版本及当前全部子项ID/版本/范围，在宿主一次共享领域事务中完成 |
| `task urgent set/clear <id>` | 原加急动作，设加急必须明确提出人及原因 |
| `task archive/trash/restore/reopen <id>` | 原受控归档、软删除、回收站恢复、归档重开；不提供永久删除 |
| `task reschedule <id>` | 原单事项改期与编号作废/预占规则；没有多事项改期的客户端循环替代审批 |
| `task child create <parent-id>` | 受限内部适配，在宿主按GUI规则继承缺省部门/联系人/类型/加入日期，再独立保存；未来日期正常预占，不向AI导出继承的自由文本 |
| `task parent set/clear <id> ...` | 原双方版本、两级结构/无环及范围检查 |
| `task children reorder <parent-id>` | 原完整子项集合/版本/范围；只改子项排序 |
| `task parent candidates <id>` | 固定最小字段投影、合法顶层候选、同客户端快照分页；源事项也须在授权范围内 |
| `queue enter/move <id> ...` | 原入队及上下移动，未来提前办理取号规则不变 |
| `work process-round/complete-round <id>` | 原真实办理/完成轮次，只在明确确认实际办理时执行 |
| `work record/correct/void <id>` | 原办理、关联更正与作废；历史影响须确认，旧记录保留 |
| `undo request --audit-id ...` | 只申请；退出8并返回软件内批准引用。没有approve、force、yes或自动批准入口 |
| `receipt get --idempotency-key ...` | 只读当前显式profile的原回执账本；未找到不执行原写入，不回退其他身份，不声称当前业务状态仍与旧版本相同 |

### 写入参数与输出

- 每次写入必须显式 `--execute`、`--profile`，并提供用户意图、原因、原幂等键及所需版本/字段基准。CLI不自动查询并补版本，不自动生成新键，不将接入授权当作业务指令。
- 通用 JSON 字段为 `idempotencyKey`、`reason`、`intent`；intent必须有非空summary及 `explicitUserRequest=true`。真实办理还须 `confirmedRealWork=true`。可选普通flag与JSON同字段重复仍拒绝；未知字段和动作覆盖均拒绝。
- 常用flag：`--idempotency-key`、`--reason`、`--intent-summary`、`--explicit-user-request`、`--confirmed-real-work`、`--task-version`、`--parent-version`、`--date`、`--event-id`、`--audit-id`、`--requester`、`--urgent-reason`、`--handled-at`、`--result-status`、`--note`。复杂patch、fieldBase、taskIds/taskVersions使用 `--input`。
- 命令中的目标ID/状态/动作由固定语法确定，JSON不再重复指定action/target；例如 `task status set 123 processing` 的JSON只提供版本、键、原因和意图。子任务使用task对象，只有title必填，其他缺省按宿主原规则处理。
- 父任务显式完成需提供父 `taskVersion` 及 `taskVersions` 对象，后者精确覆盖当前所有子项（含关系中的终态/回收站记录），最多99个子项；不能只提交可见子集或在客户端逐项模拟。普通完成不选择范围时仍走原MCP保护。
- 新建标题检索只用于候选提示，不作语义猜测、自动合并或自动选择目标；数据库事务内仍保留原同名保护。仅对尚无原键回执的新建请求进行候选预检，防止已有幂等回执被重复候选错误挡住。已有原键的显式再次执行仍由服务端核对完整请求指纹。
- 回执及错误附 `cliReceipt` 原键、请求类别和只读核验指引。仅 `committed/no_change + verified_after_commit` 返回0；仅撤销申请返回8；未知状态、提交后核验失败及请求发出后的连接/授权不明返回9。已知commitStatus和verificationStatus保留，不因失败丢掉已提交事实。
- CLI不自动重试任何写请求。原回执查询返回 `readOnly=true`、`ledgerStatus=recorded`、`currentStateVerification=not_reexecuted` 及原receipt；它证明账本记录，不能替代新的业务现状核验。
- 保留1MiB/10秒输入、纯UTF-8单Envelope、分页100条、同快照完整性、私有profile/DPAPI/机器用户/路径及二进制hash绑定等阶段2/3保护。内部IPC另有既有请求/响应大小及期限上限，未放宽资源保护。

## 3. 架构、安全与界面

- 保留单Rust package和原16个MCP工具。新 `application/cli.rs` 提供4个宿主内部适配名：受限父候选、显式完成、继承式子项创建、只读回执；只有已认证 `kind=cli` 身份可进入，仍受全局/客户端权限、范围、质量、限流及结果释放复验限制。stdio不注册这些名称。
- 对CLI能力单独声明内部version/phase及路由白名单，写入前验证宿主支持阶段4，避免同为0.5.0的旧只读宿主被误当成可写宿主；未改变现有MCP工具input schema或协议注解。
- GUI和内部父子完成共同调用事务内完成应用服务，使用原领域取号、队列、状态及工作事件规则。写入、版本、审计、原回执同事务提交；新增父子完成返回全部受影响版本并复验。子任务创建复用原写协调器，间接改变也检查范围。
- 父子完成撤销只能在软件内批准。补偿先按现有领域规则重新开启终态事项，再恢复状态/队列，并作废所有新增父项及子项办理事件；保留原历史、不回收旧号码，任何冲突整项停止。
- 只读回执按client+key查账本，并复用原范围检查和自由文本脱敏；不调用写工具、不插入审计、不更新号码、版本或账本。
- IPC对新增写路由同样追踪请求发出状态；响应框架损坏、连接中断或发出后的认证/授权不明保守返回result_unknown。服务端明确的业务错误仍按原类别返回。
- GUI授权可独立选择CLI写权限，沿用首次敏感权限提示及全局交集；**已有只读授权不自动升权**。导入只验证配置及连接，不执行测试业务写入。提示词说明先核对权限、写入须明确指令、原键核验及软件内批准。
- 宿主成功写入会触发原数据/审计刷新事件；新增路由纳入安全技术诊断。审计界面增加中文动作标签和父子状态变化，供用户批准撤销前审阅。原生事件与GUI点击仍需阶段6实机验收。

主要文件：`cli/write.rs`、`cli/parse.rs`、`cli/output.rs`、`application/cli.rs`、`database/cli.rs`，以及原写协调器、受保护IPC、宿主网关和GUI授权/审计的必要适配。未改数据库schema、依赖锁文件、版本、安装器或Actions配置。

## 4. 验证与证据

| 项目 | 最终结果 |
| --- | --- |
| 完整Rust suite | **209通过**：186库、2sidecar、21集成；零忽略、零筛选 |
| 真实CLI进程 | **14通过**，其中6项阶段4场景；原8项基础/只读/导入回归保留 |
| GUI/CLI/真实MCP stdio一致性 | 同一合成输入、三份独立库，比较事项、队列条目、编号序列、状态/办理事件及data_generation/commit_sequence；仅对执行时间戳进行存在性正规化，状态、号码、计划/票据日期及业务内容均保留 |
| 并发/范围 | CLI与实际stdio同旧版本并发，一方成功一方conflict；仅胜者写审计/回执；越权及缺profile拒绝，无MCP身份回退 |
| 父子/未来 | 默认继承及未来预占、父候选105条跨页/完整读取、普通父项完成保护、明确父项单独/包含子项完成、ABA与全关系版本校验、越权/父项晚失败全事务回滚 |
| 回执/不明结果 | 合成响应丢失退出9，未自动重放；只读原键核验不增加写调用；显式同键重放幂等，不同请求指纹冲突；已提交但核验失败保留事实且退出9 |
| 审批/历史 | 撤销申请退出8，软件内批准前业务不变；批准后新增父/子办理事件作废、原记录保留，复验全部受影响版本 |
| 其他写命令 | 编辑、加急、状态、轮次、入队/移动、归档重开、软删除恢复入队、办理记录/关联更正/作废及旧版本拒绝均经实际CLI验证 |
| 前端/构建 | **122通过/21文件**，TypeScript/Vite通过；新增父子撤销审阅内容检查 |
| 格式/契约 | cargo fmt、diff空白检查通过；实际sidecar 16工具名称、完整schema及注解与审定v1基线一致 |
| 匿名协议 | 16工具+3次重复均拒绝；前五次未认证，后续限流；stdout纯JSON，无授权、host或存储创建 |
| 冻结及保护 | **203份**产品/测试/构建输入回读一致；已安装GUI/sidecar、授权库、Codex配置四份SHA256与本阶段开发前一致；阶段2/3本地debug副本保持原摘要 |

完整Rust采用原仓库外target、jobs=1、test/dev debug info=0、test-threads=1。最终总用时约103秒，编译37秒；资源守卫未触发，提交额度峰值60%，最小余量16.81GiB。没有关闭用户软件或修改清理工具/分页文件来通过测试。

证据为 `evidence/PHASE-04-*`；完整主记录 `PHASE-04-rust-complete.txt`，参数/内存记录 `PHASE-04-rust-runtime.json` 和采样文件。实机、安装和当前聊天MCP验证不计入这些通过数。

### 调试与修复记录

保留首次夹具编译诊断（GUI办理列表模型不含voided_at，改为在合成库直接核对原始事件仍保留且已作废）；随后回归识别父子撤销必须先重新开启终态，已补共享领域补偿及全部受影响版本复验。测试新建夹具漏填必需联系人，补齐明确合成值，未放宽领域规则。定向186库+12进程、后续14进程以及最终209全量均通过。交付回读时再将帮助页的“只读请求JSON”更正为“命令的请求JSON”；除该帮助词和源码换行形式外功能源码与全量测试冻结记录一致，单独受资源守卫构建CLI并回读帮助/版本/缺execute拒绝，不重复无关全量测试（见help-readback证据）；没有删除断言、跳过测试或改变原业务必填规则。

## 5. 本地交付与停止点

`release/CLI-Phase4-local-20261022/in-line-cli.exe`，**5,485,568字节，debug控制台，非安装包**。SHA256：

`7C9CD90A1683B24AEED542B3F3B97A588A5B3EC1BF6B15F117BA0282F136BC61`

构建基线a6987f6、sourceDirty=true、exactCommit=null，不声明尚未存在的阶段4提交。实际副本帮助/版本/离线doctor、缺profile及缺execute拒绝已回读，未使用生产授权连接业务宿主。元数据见 `evidence/PHASE-04-local-artifact.json`。

尚未进行已安装原生GUI授权/审批点击、真实CLI部署、旧库/三程序升级及混版实机验收，均按用户决定留阶段6。未改生产业务数据/授权/配置，未安装、未生成NSIS或Actions安装包、未创建Tag/Release，也没有把独立测试服务当作当前聊天MCP认证成功。本聊天MCP未重新验证。

**历史停止点：实现交付时曾待阶段4独立验收；该停止点已由下方用户确认解除。阶段4已提交推送87280c9，现阶段进度见CURRENT_TASK。**


## 用户阶段确认（2026-10-22）

用户回复“A：确认阶段4通过，提交推送并进入阶段5（推荐）”。本确认针对阶段4已有实现及隔离检查，不将本机安装、当前聊天MCP认证或软件内人工验收记录为通过。历史构建证据保持原样，用户确认另见 `evidence/PHASE-04-user-acceptance.json`。
