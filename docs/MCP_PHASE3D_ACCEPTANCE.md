# P3-D 证据汇总审查报告（待用户验收）

审查开始：2026-11-04；完成：2026-11-05，Asia/Shanghai。保留用户指定的 `20261104` 索引文件名。基线：`1a2a7cd542f8bd61e3101652982f78bd27a07941`，`feat/scheduled-items`。开始时经 GitHub API 核对远端 HEAD 与该基线一致；Git HTTPS 查询超时后使用只读 API 回读，没有合并、覆盖或强推。

本报告执行已批准的纯只读证据审查，沿用 [P3-D 准备方案](MCP_PHASE3D_PREPARATION_PLAN.md) 的 61 行分组，不新增验收框架。只读取代码、Git 对象、冻结材料及既有 GitHub Actions 日志，没有运行新的业务请求、测试、审批、批次、备份或导出。三份交付文件仅保留本地，未 Commit/Push。

## 1. 五类结论分别记录

| 对象 | 结论 | 边界 |
|---|---|---|
| P3-D 证据汇总 | 只读审查和资料整理完成，待用户验收 | 不构成新增业务运行或用户接受风险 |
| Phase 3 已完成的最小业务覆盖 | P3-A/B/C 各自已验收、正式提交且正式 SHA CI 已核验；16 工具均有至少一条授权成功 L3 子路径 | 不是全部 action、参数组合、权限组合均通过 |
| 完整 Phase 3 | 尚无足够证据宣布完整通过 | 直接 L3 缺口、审批/宿主环境、G00 安全决策及其他未关闭门禁仍存在 |
| Phase 2 | 整体未完成 | P2-B 当前 Codex 会话自动恢复未完成；P2-C 原生 12/13，`ps-hang` 未通过 |
| 正式发布准备 | 尚未完成，未获发布授权 | G00/G08/G06 等需单独决策或补验；没有 main 合并、Tag/Release、安装或正式环境变更 |

本轮没有新增运行复现的产品缺陷；发现一项**已确认的源码入口白名单差异 S01**，尚未运行复现，不能据此声称产品无缺陷，也不能将它归入“只有测试不足”。详见第 7 节及 G07。

## 2. 证据等级、索引和计数规则

完整矩阵存于 [61 行脱敏证据索引](mcp/acceptance/MCP-PHASE3D-20261104-evidence-index.json)。`rows[D01..D61]` 保持准备方案顺序，每行分别列出 L1、L2、CLI 辅证、L3 成功、L3 拒绝、L4 状态、关键断言、缺口及源码支持。`sourceCatalog` 将代号映射到精确测试函数、源文件、行号、Git blob、各正式运行 SHA/CI 和实际 `test ... ok` 记录。空数组表示未定位该层独立证据，不由共用代码补作通过。

- L1：领域/数据库规则/纯函数，手工 Authorization 或内部状态操作不证明完整端点鉴权。
- L2：有效客户端经过 service/认证 IPC 的测试。CLI 路径单列，不能代替 stdio。
- L3：真实 sidecar JSON-RPC/stdio。测试文件放在 `tests/cli` 下不降低其中真实 stdio 的等级；普通 CLI 成功也不会因此升为 L3。
- L4：正式 GUI、正式 Codex 宿主、安装和真实旧库等适用环境；目前未完整验收。共享 GUI 领域 API 和本地软件批准模拟不等于 GUI 操作通过。

核对冻结 `docs/mcp/contracts/tools-v1.json` 与 `contract.rs` 的 16 个名称一致。61 个工具/action 分组键唯一，未发现该约定分组下的遗漏或重复：15 个 mutate 顶层 action 中 queue 展开为 6 mode，形成 20 行；jobs 有 6 action，start 展开 5 kind；inspect 有 5 action；saved query 有 4 action；preferences 有 get/patch；business 有 backup/export，export 分 JSON/CSV。`list/get` 等合并行在 `branchMembers` 中展开，取消缺意图和在途成功是同一 action 的不同分支，不算两个功能。

61 行不是所有参数的笛卡尔组合，也不是 61 个新增功能。34 行有选定子路径的直接 L3 成功，其中 D08、D13 明确为部分覆盖；D34 仅有直接 L3 拒绝；其余 26 行没有直接 L3 正向。因此无直接 L3 正向共 27 行。**不计算功能通过率**，任何行均不声称全部组合验收完成。

公共回归在 A/B/C 中重复通过只算复验，不算新增功能。A 的 13 条事实、B 的 104 条记录、C 的 107 条记录是不同证据族，包含连接、身份、保全等事实；不能相加为业务操作数。P2-D 的 63 条辅助元数据也不计作 63 个新成功 action。

## 3. 三个正式运行的独立账本

本轮重新只读取得三个既有正式 CI 的元数据及日志，核对对应 SHA、测试名称、结构化断言和构建身份，没有重新触发 CI。

| 阶段 | 正式 SHA | 正式 CI | 实际回归和专项 |
|---|---|---|---|
| P3-A | `d4c6774f24aaf6b4999a1247da70580a05b9ffcc` | [41566956378](https://github.com/bluntvoice/In-Line/actions/runs/41566956378) | Rust 252，前端 135；A 13 事实、P2-D 63 辅助记录；9 工具最小只读 L3 |
| P3-B | `b6312a9dad8d25f0d8f4d6332182e153ab8b3a42` | [41604479696](https://github.com/bluntvoice/In-Line/actions/runs/41604479696) | Rust 255，前端 135；B 47 次调用、104 条匹配记录、5 类门禁；A 回归仍通过 |
| P3-C | `1a2a7cd542f8bd61e3101652982f78bd27a07941` | [41790307395](https://github.com/bluntvoice/In-Line/actions/runs/41790307395) | Rust 257，前端 135；C 18 次选定调用、107 条匹配记录、5 类门禁；B 47/104 及门禁、A 回归仍通过 |

三个正式运行均有三程序离线构建身份匹配、实际 16 工具完整契约及 annotations、匿名拒绝和限流检查通过记录。该离线检查不证明已安装正式程序或当前 Codex 会话已恢复。索引 `formalLedger` 分别保存 runner、job、SHA 和实际 sidecar SHA256，不移用候选的身份：

| 正式阶段 | 本次回读对应 sidecar SHA256 |
|---|---|
| A | `431bb07f59118aa9d08a3a032a420bef84087a13de8e44cae741d8da9548af10` |
| B | `810ea8b9fce0592866e824ba2a953c023a9f76cab5a8946992d6adce394ad7c0` |
| C | `030bf44979366daa4f806d27e492e0b398f8c93f2a5b79366d47bef1fc36895b` |

A 的 sidecar hash 同时对照原正式冻结证明，B/C 对照各自实际 Q/J 构建记录。二进制 hash 是测试构建身份，不是 Token hash。每行选用最新正式 C 运行支持当前断言；原 A/B 身份仍在 `sourceCatalog.runtime` 和账本中独立保留。

### 3.1 候选及失败历史不覆盖

`runLedger` 保留并重新核对 17 个历史运行的 SHA/CI/status，不混合为一次运行。原报告与冻结 JSON 均未改写。

| 历史组 | CI | 事实和边界 |
|---|---|---|
| A 两次早期失败 | 41415440373、41416168941 | 快照预算编排问题；原失败日志未输出具体错误码，不能倒填 `resource_limit`。后续显式第五快照探测才有该码 |
| A 成功候选 | 41417095655 | `a344ae8dd5643f464b44bb4adb416fdb98d6cbb9`；不同于正式 A |
| B 编译失败 | 41569451851 | E0624，Q/J 业务未执行 |
| B 第一次业务失败 | 41587037936 | Q 第一提交 sunday/v2 已由 wire 确认，完整回执/审计核对及补偿未到达；J 通过 |
| B 第二次业务失败 | 41587762407 | 第一提交及回执/审计已核对，旧重放复合断言失败，原日志不能区分其两个子项；未补偿；J 通过 |
| B 业务成功及门禁修订 | 41588745353、41596981018、41597338585 | 原业务候选 `dc6810e0d45c73cd4202d1ae8009b8379cec7758`；最终门禁候选 `dfed2252c880da572bc67e233f1a0c27554cb01c`；成功 Q 完成独立补偿 |
| C 四轮失败 | 41758681633、41759681082、41759756218、41760035756 | 分别为 JSON 宏编译、格式、两轮 CommandExt 编译问题；都未进入 C/B 业务测试，无该轮业务写入或补偿 |
| C 成功候选 | 41787792742 | `30927f5683faa590e2d91e9a7d10991895ffaa80`；不同于正式 C |

NOT_RUN 是非执行标记。Rust 默认输出捕获可能隐藏 `P3B_NOT_RUN`/`P3C_NOT_RUN`；仅在适合的非 GitHub 环境查看时，使用标准测试的 `-- --nocapture` 可见标记，具体筛选名称见 `sourceCatalog.BQ/BJ` 和原门禁报告。**本轮没有运行该命令**。普通本地 harness 的成功统计不能作为 P3-B/C 业务通过；五类门禁的模拟非 GitHub 检查已有 Windows CI 证据，真实 Linux/macOS 守卫仍未实测。

## 4. 工具级最小 L3 与 action 深度

以下只列最小成功范围，详细成功/拒绝断言及非直接 L3 的层级以 61 行 JSON 为准。A/B/C 是首次验收来源，不是以候选取代正式 CI。

| 工具（与冻结契约一致） | 最小授权 L3 依据 | 已通过的边界 / 未据此外推 |
|---|---|---|
| get_capabilities | D01，D04/BQ/BJ | 有效身份权限/scope/构建身份，无 R 仍可自述；非正式宿主整体能力验收 |
| get_report_summary | D02，T1 | 单事项独立 1/0 统计、维度/趋势、同快照；explicitRequery 专属 L3 尚缺 |
| list_report_items | D04，T1 | 已知事项/事件、同 snapshot/dataVersion；非 >100 全规模 L3 |
| query_tasks | D05，S/D01/D03/D04 | A/T 与 B/T 隔离、已知投影、两页冻结；复杂 filters 仍部分 |
| query_task_history | D07/D08，T1/S | work 已知单事件；audit 补偿后至少两条；其他 kinds/完整纠错链不全 |
| query_work_calendar | D09，T1 | 固定时区已知 1/1/0 成员与快照；第五快照限制如实保留 |
| query_changes | D10/D11/D13，T1/S | 显式 current 初始/空增量及默认 events 初始/空增量；非空更新、removed、逐事件 L3 尚缺 |
| inspect_data | D14/D15，S/T1 | health、已知 trace；compare/cache 未直接 L3，recompute 未定位独立正向 |
| get_diagnostic | D19，S | 自有 errorId、requestId 关联和不回显私密 canary；非正式恢复因果证明 |
| manage_saved_query | D20–D23，BQ | 唯一 `p3b-only-query` 保存/list/get/等价无重复/他人不可见/删除；无 idempotencyKey |
| manage_preferences | D24/D25，BQ | 共享 week_start_day 两提交、回放/异参拒绝、版本/审计/补偿；不批准其他字段写入 |
| manage_jobs | D26/D31–D34，BJ/S | query_tasks、双 waiter、归属、completed 时取消 waiter、缺意图 cancelJob 拒绝；非正向在途取消 |
| mutate_task | D36–D38/D44–D46，M/S/R | create、标题 patch、processing、queue enqueue/processRound/completeRound；14 行复杂 action/mode 未直接 L3 |
| request_undo | D56，S | MCP 申请 needs_user_approval、N1 他人审计 not_found；一次本地软件批准另列，不是 MCP action 或真实 GUI |
| batch_mutate | D57/D58，S/B3 | dryRun 不写、apply/重放/expectedBasis、混合 not_found 全表原子性；审批闭环不全 |
| manage_business_data | D59/D60，S | 一 DB 五合成事项完整备份、一 JSON 四范围内事项、路径/ACL/审计/无 partial；CSV 和后台 export 未直接 L3 |

尚无直接 L3 正向的行：D03、D06、D12、D16–D18、D27–D30、D34–D35、D39–D43、D47–D55、D61。D34 有拒绝证据，D08 和 D13 则是同一归并行中仅部分子路径有成功证据。L1/L2/CLI 已有证据不能补作这些分支的 L3。

### 4.1 写入、补偿、审计和失败保全

- B/Q：monday v1 → sunday v2 → monday v3；两条独立幂等键、两回执、两审计，第二次是独立获授权补偿，不删除首次审计或回退版本。偏好是实例共享配置，对照身份可读不可改。旧 base/version、异参重用、非白名单、无 W、缺意图均按原契约拒绝，无部分业务提交。
- B 的 `verified_in_transaction` 是存储回执的验证阶段，wire `verified_after_commit` 是提交后核验阶段，不能强行要求相等。F=false 的同键回放只增加 `items:null` 的既有表现保留，不称第二次修改；失败 Q 与最终已补偿 Q 分开。
- C/S：标题版本 0→1→2，原 patch 幂等重放无新业务/审计/回执；request_undo 仅建待批申请，之后一次软件端自有批准才恢复标题并追加 approvedUndo/undo_of 审计。队列业务字段不变，但 save_task_on 会刷新其 updated_at，与事项时间一致；不能声称所有列完全不变。
- batch 原 apply 单 create，根与 operation 各一回执/审计；重放无第二次创建。原根存储回执不含 verificationStatus（null），不将其改写为 wire 值。N2 为合法 create + 不存在事项 setStatus 的真实 apply，全表含 sqlite_sequence 不变；其他拒绝 N1/N3/N4 的授权、表和文件均保全。
- N1/N2=`not_found`；N3/N4=`forbidden`，不以其他错误码替换。N3 是无 W 的 backup，N4 是 F=false 的 contacts 导出投影。
- 两份成功文件生成时是五条合成事项（含 B/T）和四条范围内导出；B/T 哨兵插入位置在原分页/编号与批次重放之后。后续原有不确定批次恢复新增第六条，不能用源库最终六条否定先前备份五条，也不能重新生成文件补证。
- 原 result_unknown create/batch/artifact 恢复保留原键；没有换键盲目重试。文件内容只在旧隔离夹具内核验，不进入本次交付材料。

## 5. 对准备方案的审查纠正（保留原文件）

1. JCUR 测试名含 events，但输入实际全为 current；不支持非空 events 验收。S 未传 mode，源码默认 events，所以已有默认 events 初始全量→空闲增量直接 L3，不能继续写成“完全没有 events L3”。其非空逐事件内容仍未覆盖。
2. JCH 的移出范围 removed 验证走默认 events；current 非空更新与 removed 是不同 oracle，不能相互替代。
3. QT 的 compare 是旧快照对新数据，差异包含 dataVersion；同 report 的 `differences=[]` 来自 C5F CLI，不属于 QT 或直接 stdio。
4. JC 的 execute 手工 Authorization、local_cancel/local_cleanup 是内部 API，降回 L1，不称完整 service L2 或在途 MCP cancelJob。
5. JRET 的 7/30 天检查是内部清理与内存状态核验；service 权限变化和 scope 失效另有测试，不能声称 MCP `result_expired` 请求已实测。
6. 同步 CSV 的 FEXPORT 手工 Authorization 属 L1；公式前置单引号有断言，不外推所有引号/换行组合。异步 JX 为另一工具路径的 L2。

这些更正写入新索引，不改写原冻结报告、业务结果或历史 CI。

## 6. G00～G10 发布门禁

[发布门禁与待决策清单](MCP_PHASE3D_RELEASE_GATES.md) 是唯一详细问题账本。优先顺序为 G00 完整备份权限、G08 正式宿主/安装/迁移和 Phase 2 遗留、G06 真实审批。其他项按本版本实际承诺范围分级；文档声明功能存在不等于用户已批准正式发布范围。

**所有分级均为建议，没有一项风险已由用户接受或被本轮关闭。** G00 为用户明确指定的上线前安全决策；G08/G06 建议保留相应发布门禁；G01–G07 其他路径若属于版本承诺则是对应能力的条件性阻断；G09 需调查典型可用性；G10 可作为增强候选，须用户先确认范围，不能默认不阻断。

G00 现状：有效 R+W、F=false、scope=A/T 的客户端，经明确意图创建含 B/T 的完整业务数据库备份。已有 ACL 证明仅限操作系统用户边界，不能证明同一 OS 用户下不同 MCP 客户端相互隔离。是否允许该行为用于正式环境尚未决策；本轮未改权限、API 或备份功能。

## 7. 缺陷、证据不足与后续最小授权

S01：`src-tauri/src/cli/advanced.rs` jobs/start 白名单包含 `list_report_items` 而不含 `query_task_history`；`src-tauri/src/mcp/jobs.rs` 实际分派相反。源码差异可独立核对，可能导致 CLI 支持范围与宿主不一致；但本轮没有发请求，外围参数合法时的错误路径仍是推断。需先确认 CLI 支持意图，之后单独批准最小纯参数/隔离只读复现。本轮未修改实现或追加测试。

其他直接 L3 缺口目前是证据不足，不能据此确认产品缺陷；历史 A/B/C 失败也不被重新包装为产品缺陷。宿主入口限制、正式 GUI、安装器生命周期和真实旧库环境缺口分属 G06/G08，与合成业务通过隔离。

如后续需要补测，最小授权应按 G 项选择，不整体重跑成功写入：

| 补证组 | 最小建议 | 必须单独授权的影响 |
|---|---|---|
| E1/G03 | 复用单事项固定报告，分别 inspect compare/cache/recompute，最多三条只读 action；提前核算快照预算 | 合成环境只读请求；不允许 repair 或改变上限 |
| E2/G02 | 复用已有已授权变更的模式专属 cursor，追加变更前后读取，核验非空成员/版本；removed/events 单列 oracle | 若需重建变更或移出 scope，必须另批写入/授权变更，不能借只读补证执行 |
| E3/G04 | 独立文件夹具的一次同步 CSV、一份输出；后台 export 另列请求和文件预算 | 文件生成及确切格式/字段/范围；不在原两文件夹具偷偷增加第三份 |
| E4/G05 | 一只读 job，先真实观察 queued/running，再一次自有 explicitUserRequest=true cancelJob，有限状态/结果核验 | 必须先找到现有合法可控点；不伪造在途、不靠睡眠竞态、新钩子或资源修改 |
| E5/G01/G06 | 按版本范围选择缺失 action 或审批，复用领域预期，增加直接 stdio 与独立状态核对 | action、字段、编号、幂等键、真实办理时间/意图、整批范围、批准次数逐项授权 |
| E6/G08 | 正式宿主、GUI、原生生命周期、安装与旧库副本分别安排 | 真实环境和恢复资料另行批准；不以合成测试替代 |
| S01/G07 | 先确认接口意图，再最小参数层/隔离只读复现 | 本轮未运行；修复若需要，另走产品变更授权 |

## 8. 对最终验收问题的直接回答

1. **16 工具最小 L3 全部成立吗？** 是，限第 4 节选定授权子路径，三阶段正式 CI 独立支持。
2. **哪些达到要求深度？** A 的九工具最小只读场景；B 的定义生命周期、偏好两提交补偿/幂等审计/原子拒绝、只读双 waiter；C 的选定事项路径、撤销申请与一次软件模拟批准、批次/原子拒绝、备份与 JSON 两文件内容/路径/ACL。深度限各原批准最小范围。
3. **哪些仍部分？** D08/D13 混合行、27 行无直接 L3 正向，以及已通过行的未列参数组合；完整明细见 JSON 与 G00～G10。
4. **独立运行复现的产品缺陷？** 本轮没有新增运行复现；S01 是待复现的源码差异，不能被“无缺陷”结论掩盖。
5. **哪些仅证据不足？** 未取得对应直接 stdio 的复杂 action、非空增量、inspect 分支、CSV/其他 job kind/在途取消等；L1/L2/CLI 已有不等于 L3，缺证也不证明实现错误。
6. **环境缺口？** G06 真实 GUI 审批、G08 正式 Codex/自动恢复、原生 ps-hang、安装与旧库迁移，另有真实 Linux/macOS 守卫未实测。
7. **发布前用户必须决定什么？** 首先 G00 权限模型及文件规则；还须确认版本承诺与各条件门禁的关闭/限定范围、G08/G06 安排和 G09 可用性门槛。本报告不代为接受风险。
8. **需要补测吗？** 对已完成的 A/B/C 最小范围无需机械重跑；若目标是完整 action 或正式发布，需按第 7 节和门禁清单逐项授权必要补证。
9. **A/B/C 最小业务范围完成了吗？** 是，依据用户既有阶段验收、正式提交和正式 CI；不包括本轮新增业务运行。
10. **完整 Phase 3 通过了吗？** 否。缺少约定完整范围的直接证据、适用 GUI/宿主验收、G00 安全决策和其他门禁关闭依据；Phase 2 与正式发布也未完成。

## 9. 交付、保全和停止点

本轮交付仅为本文件、61 行 JSON 和 RELEASE_GATES；原准备方案、A/B/C 报告/冻结 JSON、失败运行和临时分支均保留。最终只读核对：5 个工作树的 4,241 个既有文件、64 个 ZIP、190 个旧证据及另 35 个阶段证据文件 hash 全部匹配；工作树 HEAD、清单、既有内容不变，tracked diff 与暂存区为空，只新增获批三份未跟踪交付文件。远端开发 HEAD 完成时仍为指定基线。详细结果已登记索引 `preservation`。

JSON 结构、61 行唯一键、16 名称、60 项来源 Git blob、184 条断言的测试/运行关联、三个正式 CI 和 17 个历史运行身份、门禁记录、文档链接及脱敏静态检查均通过。新索引不含凭证、Token/Token hash、DPAPI、完整配置、数据库/备份正文、作业结果正文或原始完整日志；仅保留合成标签、测试名、断言摘要、Git/构建身份与 CI 链接。作业和 waiter 不保存真实 opaque ID。静态扫描与人工内容审查的具体边界记录于 `staticValidation`，并不宣称运行安全扫描或正式环境验收。

本轮产品、Workflow、依赖、协议、权限、资源限制变更数均为零；新 MCP/CLI 业务请求、业务测试、审批、业务文件生成、Commit/Push 均为零。文档结构/索引/脱敏检查是静态资料检查，不计作业务测试。完成后停止，等待用户验收 P3-D 汇总及单独授权正式提交。
