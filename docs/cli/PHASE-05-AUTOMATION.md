# CLI Phase5：高级自动化能力

日期：2026-10-22。状态：**用户已独立确认阶段5通过（含季度趋势修复），授权提交推送后进入Phase6；实机安装另安排。**

## 1. 基线与授权

用户已确认阶段4通过并授权提交推送后进入阶段5。阶段4提交 `87280c9a09dd8a0d6900b375be943f93fb0c8784` 已推送至 `feat/scheduled-items`，通过 `git ls-remote` 核对一致。季度趋势日期标签修复单独留在工作区，未混入阶段4提交。

依据：原CLI任务附件、[CLI PRD](../prd/cli.md)、[Phase0架构及D1–D4确认](PHASE-00-ARCHITECTURE.md)、[命令草案](COMMANDS-DRAFT.md)。压缩后先回读这些文档及[当前检查点](CURRENT_TASK.md)。

## 2. 已接入命令

| 命令 | 服务、参数与边界 |
| --- | --- |
| `stats trace / compare` | 原 `inspect_data`；用JSON提供report、right、metric及冻结snapshot；不改统计口径或重新猜测依据 |
| `query list / get / save / delete` | 原 `manage_saved_query`；命名条件按客户端隔离，保存/删除须 `--execute`、有效write及 `explicitIntent:true`（或 `--explicit-intent`） |
| `batch preview` | 原 `batch_mutate/dryRun`，也要求有效write、原键及意图/原因；不带execute，不占号、不产生业务写入 |
| `batch execute` | 必须execute；CLI先调同一dryRun服务，携带返回basis作为expectedBasis，再发一次apply；宿主再校验基准与版本。既有默认20/硬限100不变，不能跳过预演 |
| `changes list` | 原 `query_changes`；同条件、同客户端保留cursor，先全量再增量，events/current及失效规则不变 |
| `jobs start / list / get / result` | 原 `manage_jobs`；仅既有查询、报告、日历及export作业，不调度业务写入。start须execute；结果单页1–100，使用原jobId/nextOffset显式领取，不自动等待或循环重查 |
| `jobs cancel-waiter / cancel` | 都须execute，cancel另须明确用户指令；取消等待者不等于终止共享任务，发布阶段等限制由原服务处理 |
| `data cache` | 原 `inspect_data/cache`只读信息；不增加清理、修复或重算命令 |
| `data backup / export` | 原 `manage_business_data`；须execute、write、原键及意图/原因。仅返回宿主受控产物引用，不接受目标路径，不回传整份备份或读取产物内容。export格式在JSON的format或 `--export-format json/csv`，CLI的 `--format`仍是输出格式 |
| `preferences get / patch` | 原 `manage_preferences`白名单；patch须execute、write、原键、明确意图及原字段基准/版本，不读取新版本后替换，也不修改授权配置 |
| `receipt get --kind artifact --idempotency-key ...` | 扩展CLI内部原回执只读入口，查询当前客户端的产物账本元数据；不打开、创建、补做或修复产物，不证明文件目前仍完好 |
| `report daily / weekly / monthly` | 宿主受限 `cli_report` 解析期间并调用原冻结快照报表。默认宿主今天，可用date锚定；weekly为所设周起始日至锚定日，monthly为月初至锚定日。仍支持显式from/to，date与起止日期互斥 |
| `report daily --render text` | 同一冻结快照组装四字段日报；见下节 |

复杂参数使用 `--input <UTF-8 JSON文件或->`；命令已确定的action/period/text不允许再由输入覆盖。JSON未知字段、重复flag/JSON字段、缺执行开关、非法limit及不支持的作业种类明确拒绝。Phase5高级入口不接受 `--all`：增量cursor与作业nextOffset由调用者按原协议显式续取，不能把单页结果称为完整。

原16个MCP工具名称、完整输入schema及annotations保持不变。新增 `cli_report` 仅在受保护IPC中向CLI独立授权身份开放，不作为stdio MCP工具发布。旧宿主未声明CLI Phase5时高级命令拒绝；不降级至MCP身份。

## 3. 日报口径及输出

采用用户已确认D4-A，保留标题 `YYYY-MM-DD 今日计划完成事项`，并紧随口径说明：这是指定日实际办理回顾，不是未来计划。按当日每事项最后一条有效办理事件归类，仅列最后结果已处理/已完成；父子独立，排除作废、回收站及无实际办理事件的未来事项，不以事项当前状态推测已办。

逐行四字段：`事项类型-事项标题-事项对接部门-事项对接人`。多部门/联系人用 `/` 连接，字段内换行/制表符转空格，避免一个事项伪装成多行。排序为类型、标题、ID。四字段和事件筛选均从同一个授权冻结数据库读取；跨页固定snapshot，不与live联系人字段混拼，不返回办理note。

联系人要求全局及该CLI客户端同时有效fullRead；不足明确拒绝，不静默留空。JSON结构化普通摘要仍可用regularRead。文本与 `--format json`互斥，文本成功才写stdout，失败为非0并在stderr输出Envelope。单次组装最多100页/10000项，另受现有报表/响应资源限额约束；超限不返回部分成功。

```powershell
& 'D:\Application\In Line\in-line-cli.exe' report daily --date 2026-10-22 --timezone-offset-minutes 480 --render text --profile chosen
& 'D:\Application\In Line\in-line-cli.exe' changes list --mode current --limit 100 --profile chosen --format json
& 'D:\Application\In Line\in-line-cli.exe' query save --name '合同待办' --explicit-intent --execute --input '查询条件.json' --profile chosen --format json
& 'D:\Application\In Line\in-line-cli.exe' batch preview --input '批次.json' --profile chosen --format json
& 'D:\Application\In Line\in-line-cli.exe' batch execute --execute --input '批次.json' --profile chosen --format json
```

路径为后续安装后的调用示例，当前不代表本机已安装CLI。

## 4. 结果与恢复

- 预演成功0，保留commitStatus=dry_run，不当作业务提交。
- 批量needs_user_approval为8，保留approvalId；不自动批准或模拟逐项执行以绕过整批审批。
- queued/running/paused/recovering/cancelling/publishing等后台状态为8，failed为6；取消等待完成可以为0，仍保留共享作业的真实状态。
- 普通写入须committed/no_change及verified_after_commit才为0；产物须artifact_created及verified_after_commit、已知引用才为0。
- 未知成功结构、不明提交或核验失败为9；发出作业启动/取消后丢失响应也保守为9，不自动重发。原错误code及已知提交事实保留。
- 批次/偏好保留原键查业务回执；备份/export保留原键并加kind=artifact查元数据。命名查询及作业控制沿用各自已有服务语义，不虚构幂等键或业务回执。

## 5. 验证、失败记录及未验证项

新增6项真实CLI进程集成检查：混合批次与真实MCP stdio一致性（含未来预占且无虚假办理）、预演/原子冲突/软件内待批准、预演与执行之间基准变化、已提交但响应不明后的只读回执、命名查询/偏好/增量/溯源比较/jobs/受限产物，以及101项同快照日报及全局fullRead拒绝。

新增命令与退出码单元回归，原GUI/MCP检查继续运行；合成数据库及独立测试授权，不使用生产数据库或本机既有授权。

首次测试夹具误用不存在/私有数据库方法，以及复用只适用于固定3项的旧断言，均改为现有受限服务或直接读取合成库固定表，没有扩大产品API。旧版本请求已达到目标值时原服务允许no_change，测试改为真正冲突的不同目标值，保留原规则与严格断言。产物只读适配首次插入位置不正确，在编译失败后移回对应函数。失败日志保留，不通过跳过测试或修改MCP基线放行。

完整检查全部通过：

| 检查 | 结果及证据 |
| --- | --- |
| 全部Rust | **217通过**：188库、2sidecar、27集成（真实CLI集成20，新增Phase5为6）；0失败、0忽略、0过滤。`evidence/PHASE-05-rust-complete.txt` |
| 前端 | **125通过/21文件**，含季度趋势修复回归；TS/Vite构建及Rust格式检查通过。`PHASE-05-frontend.txt`、`PHASE-05-build.txt`、`PHASE-05-format.txt` |
| MCP契约 | 实际sidecar的16个工具完整schema/annotations与原API v1基线一致，未改基线。`PHASE-05-contract.txt` |
| 资源守卫 | Rust jobs1/串行，提交内存峰值63%，最小余量15.56GiB；未触发守卫。`PHASE-05-rust-runtime.json` |
| 源码与保护 | 205个构建/产品/测试输入冻结哈希回读一致；已安装主程序/sidecar、Codex配置、授权文件及旧本地CLI产物哈希未变。`PHASE-05-protected-readback.json`、`PHASE-05-final-checks.json` |

季度趋势修复独立保留在本工作区：周视图及导出显示实际起止日期，首尾周裁至范围；此前隔离浏览器趋势、明细和Canvas导出检查通过，本阶段125项前端及构建覆盖相同源码。没有把它混进阶段4已验收提交；后续提交时需包含在用户确认的范围中。

## 6. 本地保留产物

`release/CLI-Phase5-local-20261022/in-line-cli.exe`，**5,885,952字节，debug控制台，非安装包**。SHA256：

`F36B31798C4E14D053D92BB4AE6E3D9B780EE7D86708985138C8C2FFAF69E9FC`

构建基线为87280c9、sourceDirty=true、exactCommit=null；不将此debug产物标为尚不存在的阶段5提交。实际副本版本、帮助、缺execute、text/json互斥及缺profile拒绝已回读，未使用生产身份。见 `evidence/PHASE-05-local-artifact.json`。实机安装、GUI授权/审批操作、三程序升级/旧库及当前聊天MCP认证均留Phase6；本阶段未安装、未创建NSIS/Actions安装包、Tag或Release，不声称当前聊天已接入。

阶段5完成后停止待用户独立确认，未确认前不提交推送，也不进入Phase6。


## 用户阶段确认（2026-10-22）

用户回复“A：确认阶段5通过，提交推送（包含季度趋势修复），进入阶段6；实机安装另安排（推荐）”。另指示“阶段6完成后，通过action执行测试版本安装包生成”；按开发构建规则后续生成Actions Artifact，不视为正式Release或本机安装授权。本次阶段确认另见 `evidence/PHASE-05-user-acceptance.json`，原构建时检查及未验收标记保留作历史证据。
