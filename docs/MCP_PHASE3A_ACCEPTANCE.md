# P3-A：9 个只读 MCP 工具最小业务验收

原验收日期：2026-11-02；状态更新：2026-11-03（Asia/Shanghai）。**用户已确认 P3-A 最小只读业务验收通过，授权精确提交本轮五文件。** 9 个工具均有最小授权 L3 正向证据，不代表全部 action 通过。P3-B/P3-C/P3-D 实际执行未启动；GUI、正式 Codex 宿主及原会话自动恢复仍未验证，Phase 2 整体及完整 16 工具业务验收未完成。

下列运行 SHA、二进制摘要和业务记录保留为已验收候选的历史证据，不改写为正式提交的新身份；正式提交的标准 CI 须另以新 SHA 核对。本次只补充批准状态及快照可用性风险，不改三个测试文件、资源限制或既有断言。

## 基线、候选和运行

- 原隔离验收与本次提交的起始开发基线：`9a8a957569dfff1274dc3b58dd7b52faeca3132e`，`feat/scheduled-items`；隔离阶段未推进，现获准提交五文件。
- 独立工作树：`D:/codex-data/projects/inline/In-Line-p3a`；临时分支 `codex/p3a-readonly-acceptance-20261102`。
- 当前候选：`a344ae8dd5643f464b44bb4adb416fdb98d6cbb9`，只改 3 个既有 Rust 测试文件，没有新增 `#[test]`、测试框架、依赖、Workflow 或产品实现。
- [初次 CI 41415440373](https://github.com/bluntvoice/In-Line/actions/runs/41415440373)，SHA `40823b684c853923d0c4261813ed8e8d1fb3f02a`：失败，记录保留。
- [加入保全断言的 CI 41416168941](https://github.com/bluntvoice/In-Line/actions/runs/41416168941)，SHA `e6dba877acf1c92c1b0bb893a9a980d6fa123dd7`：失败，同一夹具资源预算问题，保全断言尚未执行到；记录保留。
- [最终 CI 41417095655](https://github.com/bluntvoice/In-Line/actions/runs/41417095655)，上述当前候选：**completed/success**，Job `125183521290`。实际镜像 `windows-2025-vs2026` / `20261021.278.1`，Windows Server 2025 `10.0.26100`、runner `2.338.2`。

最终前端 **135/135（22 文件）**、Rust **252/252，0 失败/0 忽略**；TypeScript/Vite、rustfmt、三程序离线身份、实际 16 工具契约和匿名拒绝检查均通过。没有新增测试用例来凑数量；本轮补强的是既有测试中的业务断言。

实际 sidecar 为软件 0.5.0 / MCP v1 / schema 14，能力回包中的 Commit 为 `a344ae8dd5643f464b44bb4adb416fdb98d6cbb9`；本次 debug sidecar SHA-256 为 `1f53b372aa7891b2dbe1d4ad94b55fc0706d4de306e62eda77c7b81dc9827748`。这是临时候选的 CI 构建，不是正式开发提交、安装包或正式宿主会话。

全部沿用现有 `.github/workflows/ci.yml` 的 GitHub-hosted Windows Job。没有触发 NSIS、dev-build 或 Release；没有在办公电脑启动本轮 MCP/CLI 夹具、安装软件或访问正式业务库/授权/Codex 配置。

## 先映射，再补断言

编辑前已在 `release/MCP-Phase3A-20261102/preimplementation-map.json` 保存“工具→action→原测试→层级→断言→原运行 SHA→缺口”，原运行 SHA 为正式基线及其已通过标准 CI。随后仅补精确业务值和必要的白名单记录。L1 为数据库/共享领域 API，L2 为经鉴权 service，L3 为真实 sidecar/stdio；L1/L2 不冒充 L3，GUI/正式宿主属于独立未覆盖层级。

| 工具 / action | 夹具、权限与 scope | 实际 L3 断言与独立预期 | 复用 / 补充来源 |
|---|---|---|---|
| `get_capabilities` / `{}` | P2-D 自建身份；明确设置客户端 R/F/W 全 false，全局较宽；scope 无限制 | 身份有效时返回 ok、权限三项 false，verificationScope=thisToolCall；之后普通查询 forbidden。能力查询不要求 R | 复用 `mcp_connection_acceptance.rs::p2d_d04_original_connection_enforces_live_intersection_scope_pause_and_revocation` 的原 transport，仅补这一条能力成功请求 |
| `get_report_summary` / 日期+时区 | 单事项单完成事件；全局和客户端 R=true/F=true/W=false；scope 无限制 | handled/completed/numerator/denominator 均 1，API rate=1.0；类型“功能开发”、部门“产品组”各 1；2026-08-08 趋势 1 | 扩展 `mcp_stdio.rs::stdio_server_lists_and_calls_read_only_report_tools`，不是新建同类测试 |
| `list_report_items` / 同快照明细 | 同上，同一日期、时区、filters=None 和 summary snapshot | total=1、workEvents=1；snapshot/dataVersion 与汇总相同；原有标题/办理说明、无联系人/详情/内部备注断言保留 | 同一既有 stdio 测试，补同快照及条数断言 |
| `query_tasks` / ID、scope、投影及分页 | 原 P2-D 首次接入哨兵；原活连接 A/T 限域；原 P2 stdio 两事项夹具 | 读取已知哨兵标题；A 范围只返回 A，显式 B ID 返回空；两条冻结分页在直接夹具颜色改变后仍保留 original-second 与同 dataVersion | 直接复用 P2-D D01/D03/D04 和 `mcp_queries.rs::p2_real_stdio_and_authenticated_pipe_use_bound_snapshot_and_redaction`，不重复新增请求 |
| `query_task_history` / kinds=work | 单事项、一次完成事件，R/F=true、W=false，无限域 | total=1、taskId 为初始化 ID、状态 completed、时间 2026-08-08T09:00:00+08:00、未作废 | 在既有只读 stdio 夹具补精确内容；普通成功不外推 includeVoided 或纠错链 L3 |
| `query_work_calendar` / 日期与时区 | 同一固定事件；初始化两个独立 R/F=true/W=false 客户端，scope 相同 | 2026-08-08 +480：1；2026-08-07 -120：1；2026-08-08 -120：0。计数包含 handlingRounds、completedTasks、workEvent 行及其已知 ID/状态/时间 | 同一只读夹具扩展；第二只读身份避免超过每客户端 4 快照的既有上限，不改阈值 |
| `query_changes` / current 全量→空闲增量 | 同一已初始化事项、R/F=true/W=false，无限域；观察期间无业务变更 | 首次 fullSyncComplete=true、恰好一条 current、匹配初始化 taskId 与固定标题；同条件 nextCursor 后增量 items=[] | 既有 L3 原仅检查完成标志和空增量；本轮补全量实际内容，未制造新业务修改 |
| `inspect_data` / health、trace(completed) | health 复用原组合夹具；trace 使用单事项报告的原快照，R/F=true/W=false | health.healthy=true（旧断言）；trace value=1、totalSources=1、complete=true、与报告同 snapshot | 复用 `mcp_queries.rs` health；补同报告的精确 trace L3。compare 为 L2，缓存规则与 CLI 另列；未找到 recompute action 的独立正向断言，不声称已覆盖 |
| `get_diagnostic` / 自身 errorId | 既有合成私有 stdio、R=true/F=false/W=false、A/T 限域；只对一次非法查询产生的编号读取 | 成功 ok、code=invalid_arguments、requestId 匹配、自带原始 canary 不回显 | 复用 `mcp_queries.rs` 已有成功调用及断言，仅补脱敏输出，不重复请求 |

上表九行各自的最小 L3 子项均通过，实际值与所列预期一致；完整候选 SHA、13 条新白名单事实记录、63 条复用 P2-D 元数据、具体测试名与运行日志摘录已冻结于 [P3-A checks](mcp/acceptance/MCP-PHASE3A-20261102-checks.json)。工具通过仅限表中明确 action，不代表该工具全部分支、GUI 或正式宿主通过。

## 预期值的来源

核心 stdio 夹具只初始化一条“完成 MCP 接入”合成事项：部门“产品组”、类型“功能开发”，唯一有效办理结果 completed，时间为 `2026-08-08T09:00:00+08:00`。预期统计、维度、事件数和标题来自这些固定输入，未从 summary/calendar/query 输出反算。

`09:00 +08:00 = 01:00 UTC = 前日 23:00 -02:00`，因此固定跨日成员关系为 1/1/0，与运行器本地日期无关。API completionRate 为比值 `1.0`（对应显示 100%），没有将 API 值错误断言为数值 100。

快照/游标 ID 必须从前次回包接续，这是协议操作；业务预期值仍独立固定。相同 snapshot/dataVersion 的对照用于证明一致性，不取代独立的 total=1/workEvents=1 等断言。

只读段前后直接比对合成事项、事件、偏好、业务审计和授权文件，只记录是否相等，不记录内容/散列。派生缓存、快照、游标和诊断索引可以正常生成，没有机械要求整个数据库或目录字节不变。该保护覆盖新增只读段；后续既有权限调整等回归按原用例单列，不能宣称整场 CI 的合成授权从未变化。

## 原有负向和复杂业务证明

这些原有测试由同一候选标准 CI 重跑，不新增重复组合：

| 重点 | 精确测试来源 / 层级 | 可证明内容和限制 |
|---|---|---|
| 部门/类型与多部门 | `mcp_security.rs::scope_filters_summary_denominator_trend_details_and_cross_client_calls`、`scope_all_departments_missing_departments_and_literal_names`，L2；P2-D D04 为 L3 | A/T、B/T、A+B/T、其他类型、缺部门和字面名称；A-only 不读取 A+B；分母/趋势/明细有独立硬编码计数；P2-D 原连接继续复验 |
| >100 分页、排序、版本和隔离 | `mcp::query::tests::pages_freeze_content_and_order_over_100_items_and_concurrent_writes`、`live_scope_change_expiry_revocation_and_generation_invalidate_results`、`resource_caps_and_database_generation_changes_fail_closed`，L2；`mcp_queries.rs` 两页为 L3 | 多页冻结、并发变更不混入、跨客户端游标拒绝、授权/代际变化及资源上限。>100 的完整业务规模没有额外重做 L3 |
| 完整字段、作废、回收站、历史 | `mcp::query::tests::history_projection_voided_and_trash_require_full_read`、`query_filters_are_bound_and_injection_has_no_sql_access`，L2；旧 stdio 与 P2-D 字段拒绝为 L3 | 未授 F 无 text/log/私密投影，includeVoided/includeTrash 要求 F；L3 新增只检查一条普通有效 work 事件 |
| 纠错链、撤销历史的读取基础 | `database::mcp_p4_tests::p4_future_creation_queue_real_work_and_correction_keep_linked_history`、`p4_backup_merge_translates_correction_ids_and_preserves_original_voided_history`，L1；历史投影为 L2 | 共享历史构造/关联规则。不是本轮新增 MCP 写入或 restore；未新增纠错链全量 L3 读取闭环 |
| 统计、报告/日历、溯源 | `mcp::query::tests::metric_trace_all_counts_rates_dimensions_trend_and_scope_match_gui`、`report_and_calendar_share_ui_calculations_and_snapshot_with_long_history`，L2 | 两种统计模式、各维度/趋势溯源、有效 winner、排除作废/回收站/范围外；共享 statistics 对照证明算法一致性，独立业务 oracle 由本轮固定夹具补足，不声称是 GUI 展示验收 |
| 非空增量及范围移出 | `mcp::jobs::tests::changes_capture_gui_events_leave_scope_without_leaking_body_and_bind_client`、`full_sync_pagination_generation_and_event_modes`，L2 | 全量分页、已知修改/current、removed 仅含 ID、不泄露移出标题、游标重放/代际失效。P3-A 新 L3 为固定数据的全量→空闲增量，不新增非空增量写入 |
| 自身诊断、暂停/撤销与泄露保护 | `mcp::p7_tests::diagnostics_own_only_pause_revoke_and_unhealthy_basis_are_enforced`、`forged_client_never_claims_recent_state_and_post_guard_error_keeps_audit_reference`，L2；自身诊断为 L3 | 其他客户端 not_found、暂停 paused、撤销 revoked、不含合成 Token、伪造身份不接管近期状态。诊断没有上传原始请求、凭证或正式路径 |
| 缓存、比较、重算边界 | `mcp::query::tests::report_cache_is_short_client_owned_invalidates_on_basis_auth_and_explicit_requery`、`trace_pagination_is_bound_to_metric_client_and_original_snapshot`，L2；`cli/phase5.rs` 为 CLI 辅证 | 复用缓存所有权/失效/明确重查及 compare 断言；未找到 inspect_data.recompute 的独立正向断言，标为未覆盖。get_report_summary 的 explicitRequery 不冒充 recompute endpoint；不因 health/trace 成功泛化所有 action |

原样标准 CI 包含此前已存在的合成写入回归；本轮没有新增这些请求，也不将其记为 P3-B/P3-C 验收。P3-A diff 中新增的 MCP 调用仅为上述只读工具；保全与初始化通过直接夹具 API 完成，没有新增成功事项写入、偏好 patch、命名查询保存/删除、批次 apply、撤销申请、作业导出或备份调用。

## 首轮失败、原因与处理

首轮和加入保全断言的候选将 report/history/三个 calendar 范围放在同一身份，第三个 calendar 请求要求第 5 个快照。初次运行在该请求处得到错误，原失败断言、运行与提交均保留。

源码 `QueryState::begin` 规定单客户端最多 4 个、全局最多 16 个快照；这是既有安全保护，不是应放宽的业务需求。最终 test-only 调整在读取前初始化第二个只读客户端，仅分担两个日历边界探测；原三个 1/1/0 业务断言完整保留。原客户端持有 report/history/calendar/query 四个快照后，另行申请新 calendar 快照，**仍要求实际 resource_limit**。

首轮日志只输出 isError=true，没有输出其错误码，不能冒称日志中已记录 resource_limit。最终 CI 的独立资源上限探测明确返回 **resource_limit**，同一日历业务请求在有预算的只读身份返回预期空结果；五项业务/授权保全均为 true，与超预算的源码路径相符。根因归为测试夹具编排不足；本轮没有确认需要产品修复的新缺陷。

没有修改产品、阈值、原业务断言预期或使用 continue-on-error；没有通过删掉失败场景获得通过。两轮原始失败与三个候选提交都保留在临时分支历史和 Actions 中。

## 快照资源限制：待评估的可用性风险

2026-11-03 按用户决定登记，**不是已确认的安全缺陷或立即修复任务**：

- 测试编排问题：原单身份组合测试先持有 report/history/两个 calendar 快照，再要求第五个；已通过只读兄弟身份分担探测修正。失败历史没有删除，实际使用问题不能由这一次测试编排直接推定。
- 已确认机制：`QueryState::begin` 单客户端最多 4 个快照、全局最多 16 个，TTL 为 600 秒；相同 client/query hash/数据基线可复用，分配前清理过期或授权 revision 不同的快照。不是每调用一次工具就固定消耗一个新快照，也没有新增自动释放接口。
- 待评估体验：真实用户连续使用不同查询/范围，或数据基线变化，需要新快照时，可能在 TTL 内遇到 resource_limit。尚未采集真实日常操作频率、触发比例及提示/恢复体验，不能据此承诺等待固定时间必然恢复，也不建议借新客户端绕开实际使用保护。

后续如另获授权，先以合成常用查询序列测量独立快照需求、复用与错误提示；正式环境观察须另获授权。当前不提高上限、不改 TTL、不增释放/驱逐机制、不改安全断言或产品源码。

## 修改、保全及未覆盖项目

临时分支仅修改：

1. `src-tauri/tests/mcp_stdio.rs`：补强独立业务值、同快照、历史、日历、增量、trace、资源保护及只读段保全；复用原夹具和 transport。
2. `src-tauri/tests/mcp_connection_acceptance.rs`：在原 transport 上增加 R=false 的能力查询成功断言与脱敏记录。
3. `src-tauri/tests/mcp_queries.rs`：既有成功自身诊断仅增加白名单记录，不增加协议请求。

上述三测试文件连同本报告和 checks JSON 是本次唯一批准提交的五文件。三个测试 Git blob 必须与最终候选完全一致；报告和 JSON 仅更新用户批准状态、风险登记及提交边界。原 855 文件及 16 项原未跟踪资料（12 项历史、4 份已审阅准备文档）保留；15 个 NSIS 原生 ZIP 摘要匹配，旧分支及工作树不清理。只读段的事项/事件/偏好/业务审计/授权五项相等性检查全部通过；原 checks 中的工作区审计状态是提交前快照。正式环境保护不以读取真实业务文件作散列“验证”，而是本轮无访问/操作路径、全部测试在托管 runner 的显式合成库执行。

正式 GUI 的展示/审批/交互、正式 Codex 工具发现与已加载配置/当前会话、原会话断线/跨升级自动恢复均未验证。没有真实旧库迁移、企业策略或断电验证；未新增完整纠错链、非空增量及所有 inspect_data action 的 L3 闭环。上述不能被 9 个最小成功路径覆盖掉。

阶段状态保留：P2-D 最小隔离验收及正式提交/CI 完成；P2-B 自动恢复未完成；P2-C 原生 12/13、ps-hang 未通过；R2 选 B、兼容性限制保留；Phase 2 整体未完成。P3-A 最小只读业务验收已获用户确认，但部分 inspect action、非空增量、复杂历史仍未完整覆盖，本轮未新增成功 MCP 业务写入。完整 16 工具验收未完成，P3-B/P3-C/P3-D 实际执行未启动，正式发布未授权。

本次只提交获准五文件、推送并核对正式提交的新 SHA 标准 CI；完成后只准备 P3-B 最小执行清单，再次停止等待确认。不实施 P3-B 元数据/偏好写入或取消操作，不进入 P3-C/P3-D，不修产品或正式安装/发布。
