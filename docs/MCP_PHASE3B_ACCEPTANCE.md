# P3-B 最小隔离业务验收报告

日期：2026-11-03（Asia/Shanghai）。用户现已确认业务验收及环境兼容性修订均通过，批准仅五文件正式提交至feat/scheduled-items。**本提交时点：P3-B验收通过，正式新SHA的标准CI尚待核验；新SHA结果以独立提交回执为准。** 下列原候选、三轮失败、四轮原业务记录及两轮门禁修订记录保留为各自SHA的历史事实，不改写成正式新SHA。

## 原业务候选冻结结果（dc6810e，已由用户验收）

- 候选：`dc6810e0d45c73cd4202d1ae8009b8379cec7758`；[CI 41588745353](https://github.com/bluntvoice/In-Line/actions/runs/41588745353)，Job `125760262009`，API 回读 completed/success，push 触发现有 ci.yml。
- Rust **254/254，0失败/0忽略**；前端 **135/135，22文件**；TypeScript/Vite、rustfmt、三程序离线身份、实际16工具 schema/注解及匿名拒绝/限流检查全部通过。
- 新增两个集成用例均通过；**47次实际工具调用、104条 expected/actual 断言全部匹配**。其中三主工具41次，辅助 capabilities 4次、query_tasks 2次；不是41个不同 action，也不代表完整16工具业务验收。
- GitHub-hosted Windows Server 2025，镜像 `win25-vs2026` / `20261030.294.1`，runner `2.338.2`。Q/J 使用同一 debug sidecar，软件0.5.0 / MCP v1 / schema14；实际能力回包 Commit 全部为当前候选。二进制 SHA-256：`eb3fca3c333f30225ee392f3ea0415b05ca26ed630b1367c78da8479f6f891de`。没有生成安装包或发布资产。
- 最终 Q：唯一命名查询完整保存/回读/等价保存/私有性/删除闭环通过；偏好 **monday(v1)→sunday(v2)→monday(v3)**，只有2条审计、2条回执，0条待批准撤销。所有负向请求均按指定错误码拒绝并通过逐次原子保全。
- 最终 J：1个 query_tasks 作业、2个独立 waiter；等价 start 复用同 job；取消 waiter-1 后其 result=waiter_cancelled，waiter-2仍取得已知单行结果；归属隔离及无意图 cancelJob 拒绝通过。**取消时作业已 completed，不是正向在途 cancelJob 证明。**
- 四条原 stdio 连接均保持各自 PID/创建时间和持有句柄身份；正常 EOF 退出均通过。原工作区856文件、64个历史ZIP逐项摘要未变，原 status 未变；正式开发分支仍为 d4c6774…，main及历史工作树 HEAD 未变。

完整逐请求、权限/scope、连接身份、预期/实得、运行 SHA、L1/L2/L3 映射及前三轮原始失败见 [冻结 checks](mcp/acceptance/MCP-PHASE3B-20261103-checks.json)。报告与 checks 仅本地交付，未混入临时测试提交或推送正式分支。

## 基线与交付身份

- 正式基线：`d4c6774f24aaf6b4999a1247da70580a05b9ffcc`。执行前两次 `git ls-remote` 均确认它仍是远端 `feat/scheduled-items` HEAD。
- P3-A 正式 [CI 41566956378](https://github.com/bluntvoice/In-Line/actions/runs/41566956378) 已独立通过只读 GitHub API 回读为 completed/success，SHA 与基线一致。
- 临时分支：`codex/p3b-isolated-acceptance-20261103`；工作树：`D:/codex-data/projects/inline/In-Line-p3b`。
- 首个候选：`ada079cdd525d5d55559a7da801773c35a9149db`；[CI 41569451851](https://github.com/bluntvoice/In-Line/actions/runs/41569451851)，Job `125698708434`：completed/failure，编译时3处 E0624（调用库内私有 mcp_basis），新增 Q/J 尚未执行。原 SHA、失败日志摘要与证据保留。
- 修正候选：`637cede3433cdfbb2ad56ec14e2c1033685c7874`；[CI 41587037936](https://github.com/bluntvoice/In-Line/actions/runs/41587037936)。仅将私有方法调用换为合成库只读表比较，并补齐 queue_number_allocations 保全；未修改产品方法可见性、错误码或业务断言。Git HTTPS 短暂连接失败，随后通过 GitHub Git Database API 按原 blob/tree/作者/时间/正文上传，逐项核对对象 SHA 并 force=false 快进临时分支，远端 SHA 与本地完全相同；没有重新发起任何 MCP 请求。最终 CI 与构建摘要以后文冻结回读为准。
- 第二轮为 completed/failure：J 全部通过；Q 查询生命周期通过，第一次 monday→sunday 收到 committed、版本2、verified_after_commit 的成功回包，但测试错误预期 verified_in_transaction，断言失败后停止，**没有执行补偿、后续偏好负向或审计核对，也不把该失败夹具记为已恢复**。原73条断言（含1条失败）、35次调用与构建身份独立保存。
- 第三候选：`0069a740405e68010f68183eb2a7194d1fc63669`；[CI 41587762407](https://github.com/bluntvoice/In-Line/actions/runs/41587762407)：completed/failure。依据既有 `mcp_write.rs::mcp_transaction`（497–545行）严格区分了两个验证阶段；首次提交及存储回执/审计核验通过，J 也通过，但同键重放后的“回包整对象相等且数据库未变”复合断言为 false。原日志未分开输出两个子项，不能仅凭此轮判断是回包形状还是数据库变化。Q 未到补偿；原76条断言（含1条失败）、36次调用和构建摘要保留。
- 当前候选：`dc6810e0d45c73cd4202d1ae8009b8379cec7758`。既有 `mcp_write.rs::redact_receipt`（831行起）在 F=false 时对 items 使用 IndexMut，即使原回执没有 items，也会补入 items:null。本候选单独严格核验该唯一附加空字段、其余所有原回执字段一致、偏好/版本/审计/回执表完全未变，并保留原整对象比较（精确预期包含该既有空字段）；不删除业务保全断言、不改产品或错误码。再次通过对象 SHA 完全一致的 GitHub API force=false 快进临时分支；只在全新 runner/新合成数据库执行，不接续前轮失败夹具。
- 只沿用 `.github/workflows/ci.yml` 的 GitHub-hosted Windows runner；没有新增 Workflow、安装包、Tag、Release、常驻服务或接管机制。
- 新测试在任何数据库、安全库或进程创建之前强制检查 `GITHUB_ACTIONS=true`、`RUNNER_ENVIRONMENT=github-hosted`、`RUNNER_OS=Windows`。本机只编辑/静态阅读、格式检查及 Git/GitHub 操作，没有启动本轮 MCP/CLI 或访问正式客户端凭证。

## 夹具与证据规则

Q/J 分别使用随机唯一临时根下的 `synthetic.db` 和显式 `security` 目录。均通过既有 `ipc::start`、实际 `in-line-mcp.exe`、私有认证 named pipe 和原始 JSON-RPC stdio 路径执行，不注入产品行为。Q 为 A/T、B/T 两条固定合成事项；J 使用独立的同样两条哨兵，且从不共享 Q 的数据库或偏好基线。

| 夹具/身份 | 全局 R/F/W | 有效 R/F/W | scope | 用途 |
|---|---|---|---|---|
| Q owner | true/false/true | true/false/true | A/T | 唯一命名查询和共享偏好修改/补偿 |
| Q other | true/false/true | true/false/false | B/T | 查询定义不可见、共享偏好可读/不可改 |
| J owner | true/false/false | true/false/false | A/T | 唯一 query_tasks 作业和两个 waiter |
| J other | true/false/false | true/false/false | A/T | 相同数据 scope、不同客户端归属对照 |

四条连接均先核对能力回包中的有效权限、scope、Commit、软件/MCP/schema 版本。每次工具调用记录原子请求序号、原 sidecar PID、FILETIME 创建时间十进制字符串，以及同一持有句柄仍存活的结果；不重连、不替换进程。结束只关闭测试自有 stdin、等待正常 EOF 退出，不强杀、不删失败数据库或检查点。

`P3B_CALL` 只输出本次明确编写的合成输入、允许的权限/scope 和状态码；job/waiter 仅输出 `job-1`、`waiter-1`、`waiter-2`。`P3B_EVIDENCE` 输出独立 expected/actual；`P3B_BUILD` 只输出候选 SHA、runner 身份和 sidecar 二进制摘要。没有输出 Token、Token 摘要、DPAPI、完整配置/数据库、作业结果正文或实际 opaque ID。预期事项 ID=1、范围外 ID=2、行数=1、固定标题来自夹具输入；日志只记 J 结果的行数/固定预期匹配布尔值与投影键。

## 三工具 action 结果矩阵

以下已执行各行均由最终 CI 确认，实得与所列独立预期一致；正向在途 cancelJob 行明确未覆盖。逐请求实际状态表附在文末，完整字段见 JSON。L1=共享数据库/领域 API；L2=鉴权 service；L3=实际 sidecar/stdio；L4=GUI/正式宿主，本轮无 L4。

| 工具/action | 身份 | 独立预期与核心断言 | 证据来源 |
|---|---|---|---|
| saved_query/list 初始 | Q owner | names=[] | 新 Q / L3 |
| saved_query/save | Q owner | 唯一名称 p3b-only-query，explicitIntent=true；saved=true | 新 Q / L3 |
| saved_query/list/get 回读 | Q owner | 仅一个名称；A/T 条件、[id,title] 投影和原名称 | 新 Q / L3 |
| saved_query/save 等价重复 | Q owner | 同名称同参数仍仅一个定义，条件/投影一致 | 新 Q / L3 |
| saved_query/list/get 私有性 | Q other | names=[]、get=not_found | 新 Q / L3 |
| saved_query/delete、删除后 list/get | Q owner | deleted=true；names=[]；get=not_found | 新 Q / L3 |
| 定义不扩 scope/不修改业务 | Q owner | A/T 恰一哨兵，范围外 ID=2 返回0；业务、偏好/回执/审计、授权不变 | 新 Q / L3+L1；范围外过滤仍受 scope 的完整规则复用 L2 |
| preferences/get 初始 | Q owner | 明确初始化 monday；读取版本 v | 新 Q / L3 |
| preferences/patch 第一次 | Q owner | monday→sunday；v→v+1；committed；回包 verified_after_commit，存储回执 verified_in_transaction；changedFields 仅 week_start_day | 新 Q / L3+L1 |
| preferences/patch 同键同参 | Q owner | replayed=true、原 auditId 不变；既有 F=false 脱敏仅附加 items:null，其余原回执字段严格一致；数据库偏好/版本/审计/回执不变 | 新 Q / L3+L1 |
| preferences/patch 同键异参 | Q owner | idempotency_conflict；无部分提交 | 新 Q / L3+L1 |
| preferences/patch 旧 base | Q owner | conflict；无部分提交 | 新 Q / L3+L1 |
| preferences/patch 旧 version | Q owner | conflict；无部分提交 | 新 Q / L3+L1 |
| preferences/patch 非白名单混合 payload | Q owner | launch_at_login + week_start_day 被 invalid_arguments 拒绝；无部分提交 | 新 Q / L3+L1 |
| preferences/patch 缺明确意图 | Q owner | explicitUserRequest=false→explicit_intent_required；无部分提交 | 新 Q / L3+L1 |
| preferences/patch 无 W | Q other | forbidden；无部分提交 | 新 Q / L3+L1 |
| preferences/get 已提交共享配置 | Q owner/other | 均读取 sunday/v+1，与事项部门 scope 无关 | 新 Q / L3 |
| preferences/patch 第二次独立补偿 | Q owner | 首次全部核验后 sunday→monday；新独立键/intent/reason；v+1→v+2；不同审计 | 新 Q / L3+L1 |
| preferences/get 补偿后 | Q owner/other | 均为 monday/v+2；两次审计/回执仍在，第一次记录原样；没有 undo | 新 Q / L3+L1 |
| jobs/start | J owner | 只创建一个 query_tasks 作业，reused=false | 新 J / L3；既有 L3 生命周期直接保留 |
| jobs/get | J owner | 同一 job 正常到 completed | 新 J / L3 |
| jobs/start 等价重复 | J owner | 同数据/配置/授权基线复用 job-1，reused=true；独立 waiter-2，与 waiter-1 不同 | 新 J / L3 |
| jobs/list | J owner/other | owner 仅 job-1；other 为空 | 新 J / L3 |
| jobs/get/result/cancelWaiter/cancelJob 归属隔离 | J other | 均 not_found；result 带 owner waiter-2 也不能冒用；cancelJob 不带明确意图 | 新 J / L3 |
| jobs/cancelJob 缺意图 | J owner | explicit_intent_required；前后完整 job view 不变 | 新 J / L3 |
| jobs/cancelWaiter | J owner | 仅一次取消 waiter-1；waiting=false；操作时 job 已 completed | 新 J / L3 |
| jobs/result 已取消 waiter | J owner | waiter-1→waiter_cancelled | 新 J / L3 |
| jobs/result 存活 waiter | J owner | waiter-2→已知 A/T 单行、[id,title]、complete=true；job 仍 completed | 新 J / L3 |
| jobs/cancelJob 正向在途 | 无 | 未授权、未执行、未覆盖 | 不由已完成作业取消或旧 local_cancel 证明替代 |

命名查询协议没有 idempotencyKey/auditId，本轮没有构造这些字段。等价保存通过 owner/name 回读核验，不换名称。偏好本轮两个稳定键为 `p3b-Q-monday-to-sunday` 与 `p3b-Q-sunday-to-monday`；负向请求各使用独立、固定测试键，失败时断言无回执/审计写入。没有结果不确定后换键重试或盲目补偿。

## 既有与新增证据映射

| 精确测试来源 | 层级 | 已有断言/本轮用途 |
|---|---|---|
| src-tauri/src/mcp/query_tests.rs::saved_queries_are_client_isolated_persistent_and_reauthorized | L2 | 保存、私有 list、重开后 get、降权投影拒绝、delete 后空及定义无 Token；原样复用，不再新建持久化框架 |
| 同文件::saved_definition_write_requires_effective_write_and_current_scope | L2 | 无有效 W 拒绝；保存范围外过滤条件不授予对应数据访问；原样复用 |
| src-tauri/src/database/mcp_write_tests.rs::p3_preference_whitelist_atomic_and_user_approved_undo | L1 | 白名单及失败原子性；旧 undo 仅作历史回归，不作为 Q 清理手段 |
| 同文件::p3_preference_aba_and_cross_client_undo_do_not_bypass_permissions | L1 | 旧版本/ABA 冲突、跨客户端审批隔离；原样复用 |
| src-tauri/src/mcp/jobs_tests.rs::private_jobs_dedup_waiter_cancel_result_and_revocation | L2 | 相同 owner 去重、单 waiter 取消、另一个取结果、他人隔离及撤权；原样复用 |
| 同文件::limited_jobs_cancel_and_cleanup_preserves_running_checkpoints | L2/内部 API | 缺意图拒绝；后续 local_cancel 为软件端 API，不能冒充 MCP 正向在途取消 |
| src-tauri/tests/mcp_queries.rs::p2_real_stdio_and_authenticated_pipe_use_bound_snapshot_and_redaction | L3 | 既有 start/get/result 完成及脱敏；原测试、断言不变 |
| src-tauri/tests/cli/phase5.rs::phase5_saved_queries_preferences_incremental_inspection_and_jobs_use_original_services | CLI 辅证 | 旧命名查询/偏好/作业生命周期；CLI 不冒充 MCP stdio |
| src-tauri/tests/mcp_queries/p3b.rs::p3b_q_saved_query_and_shared_preference_compensation_over_original_stdio | 新 L3+L1 | 补齐 Q 的唯一查询完整生命周期、两次偏好提交、回执/审计、原子拒绝及保全 |
| 同文件::p3b_j_one_read_job_two_waiters_and_owner_isolation_over_original_stdio | 新 L3+L1 | 补齐 J 的双 waiter、归属隔离、无意图 cancelJob 拒绝及已完成作业的 waiter 取消 |

标准 CI 原样执行旧合成回归，含旧事项写入、撤销、批次及文件相关测试。本轮没有新增这些调用，不将旧回归称为 P3-C 授权执行或验收。新增 Q/J 的工具仅三工具及必要 get_capabilities/query_tasks 辅助读取。

## 写入、补偿与失败恢复核对

最终完整运行中，每次偏好成功提交后，直接用只读 SQLite 连接核对 owner+原键的 mcp_receipts：存储回执为 verified_in_transaction、实际回包为 verified_after_commit（由提交后独立回读决定），仅该验证阶段字段不同，其余全部回执字段严格一致。并经共享 mcp_audit_state 核对审计动作 preferences、owner 关联、前后值/版本、reason、intent、undoOf=null。补偿后再次核验第一次回执/审计未被修改。实得仅2条业务审计、2条回执、0条待批准撤销；settings 恢复初值，week_start_day 版本增加2，其余偏好版本不变。同键重放实得唯一附加 items:null，其他全部原回执字段和数据库状态保持一致。

四轮使用相互独立的新鲜 runner。首轮编译失败，没有执行新增动作；第二、三轮各执行1次首次偏好提交，均因测试断言失败停止、没有补偿；最终轮完成2次提交和补偿。**跨全部运行累计收到3次正向提交、1次补偿成功回包，涉及3个独立 Q 数据库；只有最终 Q 的补偿完成可被本报告确认。** J在后三轮各有1个独立只读作业/2个waiter/1次owner取消，累计3个合成作业；每轮内部均满足夹具上限，不能合并成同一个作业历史。没有在失败夹具上换键重试或清理；未下载这些数据库，runner生命周期结束后的状态不能继续回读。本轮没有重放或清理正式环境资料。

旧 base 与旧 version 分开验证。非白名单请求携带一个有效白名单目标和一个非法键，整体拒绝；同键冲突、权限拒绝及明确意图拒绝均逐次对比 settings、偏好版本、审计、回执、undo 表，不能部分提交。Q 的业务表和授权在失败前后也保持一致。

没有人为注入本轮写入回包丢失/崩溃，不能声称本轮验证了未知结果恢复。旧同键恢复测试保留为既有回归；若真实出现未知结果，应按原 owner/name 或原幂等键先核验并停止依赖操作。本轮不新增清理或恢复机制。

J 在第二次等价 start 前通过只读连接核对基线输入表 mcp_data_basis、mcp_change_index、schema_meta 完全相等；涵盖 database UUID、数据代际、提交序号、索引 epoch、schema 版本，query/statistics 版本常量由同一源码 SHA 固定。Q/J 数据根独立。只取消已完成 job-1 的 waiter-1，waiter-2 取回已知结果；因此证明等待者取消与共享作业隔离，**不证明在途计算取消、共享任务中断或正向 cancelJob**。

## 保全、完整文件清单与风险

合成业务保全直接比对 tasks、task_queue_entries、daily_sequences、queue_number_allocations、task_logs、status_history、task_work_events、urgent_records、master_values、mcp_task_versions、mcp_field_versions 的完整行值，覆盖事项、队列、编号、事件、日志、紧急记录、字典和事项版本。授权 DPAPI 仅夹具内逐字节相等，不输出内容或散列。Q 允许且只允许预期偏好/审计/回执变化；J 的偏好/审计/回执/授权与基线输入表也全部不变。派生查询快照、作业检查点与诊断元数据正常生成，不要求整个数据库字节相同。

候选仅提交并推送：

1. `src-tauri/tests/mcp_queries.rs`：仅增加 test-only 子模块声明。
2. `src-tauri/tests/mcp_queries/p3b.rs`：两个集成用例及其私有夹具/脱敏证据辅助函数；无新依赖、无产品 hook。

本地另交付本报告及 `docs/mcp/acceptance/MCP-PHASE3B-20261103-checks.json`，均不混入测试提交、不推送正式分支。被忽略的 `release/MCP-Phase3B-20261103/` 保存本轮本地只读 GitHub 回读脚本、脱敏 CI proof 和保全清单，不含业务数据库/凭证/结果正文；运行日志本身不下载保存，只提取允许的记录及测试/runner 元数据。

执行前对原工作区856个跟踪/未跟踪文件冻结摘要与原 status，对原 release 下64个历史ZIP另存摘要；最终逐项均匹配，原 status 完全相同。远端 feat/scheduled-items 仍为 `d4c6774f24aaf6b4999a1247da70580a05b9ffcc`，main仍为 `1baac96252af2665274f2a955b3adacfa80519fd`；P2-D工作树仍为 `b997b3fe78a37c27363c433d9eda39d34f7a02a2`，P3-A工作树仍为 `a344ae8dd5643f464b44bb4adb416fdb98d6cbb9`。没有清理历史路径、NSIS包或覆盖原未跟踪资料。正式安装、数据库及当前Codex客户端凭证未访问；这属于本轮操作边界，不是一次正式环境GUI或宿主验收。

保留限制：P2-B 自动恢复未完成；P2-C 原生生命周期12/13，ps-hang 未通过；快照每客户端4、全局16、TTL600秒仍是待评估可用性风险，本轮未改上限/TTL/作业资源限制。正式 GUI、Codex 宿主及真实旧库迁移未完整验证。Phase 2、完整 Phase 3 和发布均未完成。

原候选 dc6810e 的两个用例在本机直接运行会在夹具创建前因环境断言失败，因此用户暂缓正式提交。本轮仅修订这一测试兼容性限制，运行约定与验证结果见文末；原业务候选的 CI 通过不等于办公电脑本地全套已实测。

## 后续 P3-C 的最小建议（仅建议，未执行）

P3-B 经用户验收后，先逐 action 映射既有 mutate_task、request_undo、batch_mutate、manage_business_data 的 L1/L2/L3 证据。仅对缺口提出独立合成夹具：最小单事项修改及稳定键回读/冲突、一次用户批准的撤销请求及软件端审批模拟、最小批次预演/拒绝原子性；文件创建若确需 export/backup，单列内容、数量、位置和读取边界后再授权。优先复用已有测试，不默认扩展完整事项 action 矩阵或正向在途 cancelJob，不加入真实 GUI/正式宿主/旧库迁移/发布。

原 P3-B 业务验收已由用户确认；当前停止点为环境兼容性修订复核。未经本轮复核及后续明确指令，不将任何测试提交或推送至 feat/scheduled-items，不启动 P3-C/P3-D。

## 最终逐请求实际状态表

以下41次主工具调用全部为当前候选的 L3；每行预期/实得均来自实际冻结记录。业务值、版本、审计/回执和保全的独立断言见上表及JSON的104条facts。权限/scope按前述身份表，具体PID、FILETIME和请求序号保存在JSON。

| 工具/action | case | 身份 | 预期状态 | 实得状态 | 原连接序号 |
|---|---|---|---|---|---|
| manage_jobs/start | start_job | J-owner | ok | ok | 3 |
| manage_saved_query/list | initial_list | Q-owner | ok | ok | 3 |
| manage_jobs/get | poll_original_job | J-owner | ok | ok | 4 |
| manage_saved_query/save | save | Q-owner | ok | ok | 4 |
| manage_jobs/start | equivalent_start | J-owner | ok | ok | 5 |
| manage_saved_query/list | save_list | Q-owner | ok | ok | 5 |
| manage_jobs/list | owner_list | J-owner | ok | ok | 6 |
| manage_saved_query/get | save_get | Q-owner | ok | ok | 6 |
| manage_jobs/list | other_list | J-other | ok | ok | 3 |
| manage_saved_query/save | equivalent_save | Q-owner | ok | ok | 7 |
| manage_jobs/get | other_get | J-other | not_found | not_found | 4 |
| manage_jobs/result | other_result_impersonate | J-other | not_found | not_found | 5 |
| manage_saved_query/list | equivalent_save_list | Q-owner | ok | ok | 8 |
| manage_jobs/cancelWaiter | other_cancel_waiter | J-other | not_found | not_found | 6 |
| manage_saved_query/get | equivalent_save_get | Q-owner | ok | ok | 9 |
| manage_jobs/cancelJob | other_cancel_job_no_intent | J-other | not_found | not_found | 7 |
| manage_jobs/get | before_cancel_intent_denial | J-owner | ok | ok | 7 |
| manage_saved_query/list | private_list | Q-other | ok | ok | 3 |
| manage_jobs/cancelJob | cancel_job_without_intent | J-owner | explicit_intent_required | explicit_intent_required | 8 |
| manage_saved_query/get | private_get | Q-other | not_found | not_found | 4 |
| manage_jobs/get | after_cancel_intent_denial | J-owner | ok | ok | 9 |
| manage_jobs/cancelWaiter | cancel_waiter_1 | J-owner | ok | ok | 10 |
| manage_jobs/result | cancelled_waiter_result | J-owner | waiter_cancelled | waiter_cancelled | 11 |
| manage_saved_query/delete | delete | Q-owner | ok | ok | 12 |
| manage_jobs/result | surviving_waiter_result | J-owner | ok | ok | 12 |
| manage_saved_query/get | deleted_get | Q-owner | not_found | not_found | 13 |
| manage_saved_query/list | deleted_list | Q-owner | ok | ok | 14 |
| manage_preferences/get | initialized_get | Q-owner | ok | ok | 15 |
| manage_preferences/patch | patch_forward | Q-owner | ok | ok | 16 |
| manage_preferences/patch | same_key_same_parameters | Q-owner | ok | ok | 17 |
| manage_preferences/patch | same_key_different_parameters | Q-owner | idempotency_conflict | idempotency_conflict | 18 |
| manage_preferences/patch | stale_base | Q-owner | conflict | conflict | 19 |
| manage_preferences/patch | stale_version | Q-owner | conflict | conflict | 20 |
| manage_preferences/patch | not_whitelisted | Q-owner | invalid_arguments | invalid_arguments | 21 |
| manage_preferences/patch | missing_explicit_intent | Q-owner | explicit_intent_required | explicit_intent_required | 22 |
| manage_preferences/patch | no_write_permission | Q-other | forbidden | forbidden | 5 |
| manage_preferences/get | owner_committed_get | Q-owner | ok | ok | 23 |
| manage_preferences/get | shared_committed_get | Q-other | ok | ok | 6 |
| manage_preferences/patch | patch_compensation | Q-owner | ok | ok | 24 |
| manage_preferences/get | compensated_shared_get | Q-owner | ok | ok | 25 |
| manage_preferences/get | compensated_shared_get | Q-other | ok | ok | 7 |

## 本地证据文件完整清单

以下均位于被忽略的 release/MCP-Phase3B-20261103/，仅用于本轮证据和回读，不提交/推送：

- `ci-41569451851-proof.json`
- `ci-41587037936-proof.json`
- `ci-41587762407-proof.json`
- `ci-41588745353-proof.json`
- `freeze-evidence.cjs`
- `github-push-exact.cjs`
- `github-readback.cjs`
- `original-files-before.json`
- `original-status-before.txt`
- `original-zips-before.json`
- `preservation-proof.json`
- `remote-refs-proof.json`

本报告与checks为新增未提交文档；测试提交仅包含前述两个Rust测试路径。前三轮失败与最终运行的proof分别保留，没有把不同SHA的事实/二进制摘要拼成同一次运行。

## 测试运行环境兼容性最小修订（本轮复核材料）

用户确认 dc6810e 业务验收通过，但暂缓正式提交。本轮只改 test-only 环境判断和验证，继续使用 `codex/p3b-isolated-acceptance-20261103`；正式基线仍为 `d4c6774f24aaf6b4999a1247da70580a05b9ffcc`。不更改产品、权限、协议、数据库实现、Workflow、依赖、资源限制或原业务断言，不进入 P3-C/P3-D，不创建 Tag/Release 或合并 main。

### 精确源码差异

相对已验收候选 dc6810e，仅2个 test-only 路径，137行增加、8行删除：

1. `src-tauri/tests/mcp_queries/p3b.rs`：`Fixture::new` 改为返回 `Option<Self>`。缺少 GITHUB_ACTIONS 时，先输出 `P3B_NOT_RUN`（fixture、not_run、non_github_environment、businessExecuted=false），立即返回 None；判断发生在随机根、目录/数据库、安全库、客户端或sidecar创建之前。两个用例仅在入口接收 None 后提前返回，其他正文保持一致。
2. 同文件保留全部3项环境断言，并补充明确失败消息。只要 GITHUB_ACTIONS 存在（包括值为false或非预期值），就不能走“非执行”分支；GITHUB_ACTIONS不为true、RUNNER_ENVIRONMENT不为github-hosted、RUNNER_OS不为Windows均明确失败。
3. 同文件新增1个门禁回归用例，通过同一编译测试二进制的子测试进程，分别模拟 local_Q、local_J、无效GitHub声明、self-hosted和非Windows声明。仅对子进程设置环境，不修改全局环境，不与真实Q/J产生环境竞态；从不模拟“被批准的环境”来发起额外业务动作。
4. `src-tauri/tests/p3b_unsupported_platform.rs`：14行独立非Windows守卫。因为原stdio测试crate整体cfg(windows)，该守卫避免在非Windows GitHub上仅因0个stdio测试而静默跳过。GitHub声明存在即失败；非GitHub输出Q/J未执行标记。没有夹具、数据库、客户端或sidecar创建逻辑。

Rust默认测试框架不提供本方案所需的动态ignored状态，因此非GitHub的Q/J函数会正常返回，harness可能将其计入“passed”。**该统计只能表示测试函数未失败；明确的P3B_NOT_RUN表示业务用例未运行，不能用本地harness成功数声称P3-B专项通过。** 只有批准的托管Windows实际产生P3B_CALL、P3B_EVIDENCE和P3B_BUILD时，才计入专项业务证据。

两个原业务用例正文（除夹具入口提前返回）、record函数和原104条业务断言保持不变；不删除原失败验证，不更换错误码，不增加路径/凭证或release绕过参数。精确patch保存于本地被忽略目录 `release/MCP-Phase3B-Gate-20261103/environment-gate.patch`，原报告/JSON字节快照及旧12份证据文件摘要也保存于该目录。

### 候选与验证边界

- 首个门禁候选：`25571e390eb6eff21aaa1b3dfb3626fdacd12e3e`；[CI 41596981018](https://github.com/bluntvoice/In-Line/actions/runs/41596981018)。该候选随后补入独立非Windows守卫，没有基于业务失败更改断言。
- 完整修订候选：`dfed2252c880da572bc67e233f1a0c27554cb01c`；[CI 41597338585](https://github.com/bluntvoice/In-Line/actions/runs/41597338585)。API回读completed/success；精确结果如下。
- 办公电脑仅执行源码对比、格式检查和Git/GitHub操作，未运行本轮cargo test、MCP/CLI、合成写入或门禁子测试进程。模拟非GitHub检查由GitHub-hosted Windows运行同一测试二进制完成；这不等同于本机全量回归实测，也不声称已在真实Linux/macOS上编译运行守卫。
- 门禁回归分别要求：本地Q/J子进程成功退出且各有1条P3B_NOT_RUN；3种不支持的CI声明失败并匹配对应原因；5个探测根始终不存在；全部子进程均无P3B_BUILD/CALL/EVIDENCE。没有下载或输出子进程原始响应、配置、数据库或凭证。

### 原记录、语义与停止点

此前四轮原始proof、候选SHA、sidecar摘要及三轮失败保留，不重新生成或改写。原最终Q补偿成功只属于dc6810e对应夹具；前两个失败Q未执行补偿的区别保留。`verified_in_transaction`是事务内存储回执阶段，`verified_after_commit`是提交后独立回读阶段；F=false同键重放附加`items:null`的既有表现仍严格验证。正向在途cancelJob仍未授权、未执行、未覆盖，J仅证明已完成作业的waiter取消。

P2-B自动恢复、P2-C原生12/13与ps-hang未通过、快照可用性风险、正式GUI/Codex宿主/真实旧库迁移的未完成状态均保留；Phase 2、完整Phase 3及发布未完成。完成本轮冻结后停止等待用户复核，复核前不正式提交P3-B测试或资料，不提前执行P3-C的新业务写入、审批模拟或文件生成。

### 本轮冻结回读结果

完整候选 `dfed2252c880da572bc67e233f1a0c27554cb01c` 的 [CI 41597338585](https://github.com/bluntvoice/In-Line/actions/runs/41597338585) / Job `125787653866` 为 completed/success。Rust **255/255，0失败/0忽略**（比原候选增加1个门禁回归用例），前端 **135/135，22文件**。TypeScript/Vite、rustfmt、三程序离线身份、实际16工具schema/注解及匿名拒绝/限流步骤均通过。

Q/J两个P3-B业务用例均真实执行并通过；**47次实际调用、104条原业务断言保持匹配**。与dc6810e逐事实/逐请求比较，仅当前准确构建SHA、PID/FILETIME、二进制摘要和runner时间等运行元数据不同，业务预期/实得、输入、权限、scope、请求顺序及错误码相同。两个业务用例正文除夹具入口外保持一致。

实际GitHub-hosted Windows镜像 `win25-vs2026` / `20261030.294.1`，Q/J使用同一debug sidecar，软件0.5.0 / MCP v1 / schema14，能力回包Commit均为新候选。新sidecar SHA-256：`8e7da6adc89c01d6fcb769a3849bd6292b51f2f49232151b5e84afc8f673e08a`；原候选二进制摘要仍保留在原冻结记录，未改写为新摘要。

| 门禁模拟 | 预期 | 实得 | 资源/业务证据 |
|---|---|---|---|
| local_Q | 正常返回；Q/J未执行 | 退出码0；P3B_NOT_RUN | 探测根不存在；无BUILD/CALL/EVIDENCE |
| local_J | 正常返回；Q/J未执行 | 退出码0；P3B_NOT_RUN | 探测根不存在；无BUILD/CALL/EVIDENCE |
| invalid_ci_declaration | 明确拒绝；Q/J未执行 | 退出码101；拒绝原因匹配 | 探测根不存在；无BUILD/CALL/EVIDENCE |
| unsupported_runner | 明确拒绝；Q/J未执行 | 退出码101；拒绝原因匹配 | 探测根不存在；无BUILD/CALL/EVIDENCE |
| unsupported_os | 明确拒绝；Q/J未执行 | 退出码101；拒绝原因匹配 | 探测根不存在；无BUILD/CALL/EVIDENCE |

**本地测试未执行**：上述非GitHub结果来自托管Windows的同一测试二进制子进程模拟，办公电脑仅作源码/格式验证。普通本地cargo test将不会仅因新增Q/J缺少GitHub变量而失败，但不能将本地harness的正常返回统计当成专项通过。真实Linux/macOS平台未编译运行；独立非Windows守卫为源码级核对，RUNNER_OS不支持的拒绝路径已通过托管Windows模拟。

**GitHub专项测试通过**：Q偏好仍为monday(v1)→sunday(v2)→monday(v3)，2条回执和2条审计完整，原第一次审计/回执未删除或倒退；同键重放仍只附加既有items:null，业务状态不变。J仅取消已completed作业的waiter，正向在途cancelJob仍未覆盖。P3-A只读stdio回归、P2-D四项连接回归、原stdio/CLI及其他历史回归均在同一新SHA标准CI通过，其源码未改。

两次新增候选CI均为独立新鲜Q/J实例，每轮各2次成功偏好提交（含1次补偿）、1个只读job、2个waiter和1次owner waiter取消；每轮47次业务调用，不将门禁子进程计为业务执行。没有新增业务失败，也没有以重新运行成功替代失败调查。此前三轮失败及第四轮已验收成功记录保持原样。

保全回读：原工作区**856文件**、历史**64个ZIP**、此前证据目录**12文件**逐项摘要不变，原status不变；当前测试工作树除两项获准测试变更及本报告/JSON外，无其他既有文件变化。feat/scheduled-items仍为d4c6774…，main和P2-D/P3-A历史工作树HEAD不变，未访问正式安装/库/客户端凭证。

本轮修改清单：提交并推送的2个测试路径见“精确源码差异”；本地更新仅本报告和既有checks，checks的environmentCompatibilityRevision单独记录新候选/运行/门禁/业务证据，原根部104事实、47动作、三轮失败及四轮运行不替换。辅助回读、原文档备份、精确patch、源码对比、保全清单和新CI proof仅存于被忽略的release/MCP-Phase3B-Gate-20261103/。文档未提交或推送正式分支。

本轮到此停止，等待用户复核。复核前不正式提交至feat/scheduled-items，不启动P3-C/P3-D，不创建Tag/Release或合并main。

## 正式提交批准与使用说明（2026-11-03）

用户已复核通过完整修订候选 `dfed2252c880da572bc67e233f1a0c27554cb01c`，批准三个测试按对应Git blob原样提交，并提交本报告和脱敏checks。正式父基线为 `d4c6774f24aaf6b4999a1247da70580a05b9ffcc`；此刻P3-A验收/提交/CI完成，P3-B业务验收和门禁复核通过，正式新SHA CI待核验后收尾。三个工具的既有结论不扩大：命名查询生命周期、偏好两次提交/版本/幂等/审计/补偿/原子拒绝通过；作业仅只读、双waiter、归属及已完成作业waiter取消通过，正向在途cancelJob未覆盖。

以下旧文中“等待复核/暂缓正式提交”的表述是原阶段快照，由本次用户批准更新；原SHA、预期/实得、失败状态、构建摘要和未补偿事实不被覆盖。正式提交只能包含用户指定五文件，原release证据目录不提交。新SHA构建身份和CI回执另行交付，不冒充候选CI。

### 查看非执行业务标记

在普通非GitHub环境查看两个用例及门禁回归的完整输出，可用：

```powershell
cargo test --locked --manifest-path src-tauri/Cargo.toml --test mcp_queries p3b:: -- --nocapture
```

默认测试输出可能隐藏标记，应查看完整输出中的 `P3B_NOT_RUN`、`status=not_run` 和 `businessExecuted=false`。harness显示passed只表示函数正常返回，不能当作P3-B业务通过；只有批准的GitHub-hosted Windows的实际47次调用/104事实才是专项业务证据。保持环境真实，不伪造GitHub变量来触发本地写入。本轮未在办公电脑执行该命令。

非Windows守卫尚未在真实Linux/macOS运行器编译执行，仅做源码核对和Windows上不支持OS声明的模拟，限制继续保留。P2-B自动恢复未完成，P2-C原生12/13、ps-hang未通过，快照限制为待评估可用性风险；正式GUI、Codex宿主及真实旧库迁移未完整验证。完整Phase 2、Phase 3和发布均未完成。

P3-B正式CI通过后，仅准备四工具P3-C逐action证据和最小执行清单；不新增业务写入、审批模拟或文件生成。不合并main，不创建Tag/Release。
