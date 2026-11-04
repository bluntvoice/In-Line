# P3-C 最小隔离业务验收报告

日期：2026-11-04（Asia/Shanghai）。状态：**用户已验收 P3-C 最小隔离业务范围并批准正式提交；正式提交及新 SHA CI 核验完成后方可收尾。**

## 0. 原始失败与修订记录

| 候选 SHA | 运行 | 实际失败原因 | 业务/补偿状态 |
|---|---|---|---|
| `45d8555ff00229009afb218d18578947805165bb` | [#41758681633](https://github.com/bluntvoice/In-Line/actions/runs/41758681633) | 新增json!数组表达式缺括号，编译失败 | P3-C和P3-B业务均未执行；无该轮成功写入或补偿 |
| `f9a0e914763c324b89b5487635118daece39b03c` | [#41759681082](https://github.com/bluntvoice/In-Line/actions/runs/41759681082) | 文本传输多加1个文件末尾空行，rustfmt失败 | P3-C和P3-B业务均未执行；无该轮成功写入或补偿 |
| `a10c37cc14ed80a6c41b762d4d661aab70b7b9a7` | [#41759756218](https://github.com/bluntvoice/In-Line/actions/runs/41759756218) | 新增std Command.creation_flags缺CommandExt，E0599 | P3-C和P3-B业务均未执行；无该轮成功写入或补偿 |
| `72df9743a9528a625af6997993d42d221124c43b` | [#41760035756](https://github.com/bluntvoice/In-Line/actions/runs/41760035756) | 同一E0599；读取上一轮失败时本候选已在运行 | P3-C和P3-B业务均未执行；无该轮成功写入或补偿 |

源码级修订还将原新写oracle中的队列updated_at字节不变要求改为逐业务列保全并精确验证该元数据，将成功create的字典全表不变要求改为既有部门/类型/联系人usage_count各+1且其他列完全一致；两项修订有既有domain.rs和bump_master_use明确依据，发生在成功业务验收之前，不删除原业务断言、改变请求/拒绝码或修改产品。失败运行及各候选Git对象均保留，不通过重跑同一源码碰运气放行。

## 1. 精确范围与身份

- 正式基线：`b6312a9dad8d25f0d8f4d6332182e153ab8b3a42`，执行前远端 HEAD 相同。正式分支不接收本轮修改。
- 临时分支：`codex/p3c-isolated-acceptance-20261104`；工作树：`D:/codex-data/projects/inline/In-Line-p3c`。
- 当前候选：`30927f5683faa590e2d91e9a7d10991895ffaa80`；[标准 CI #41787792742](https://github.com/bluntvoice/In-Line/actions/runs/41787792742)。completed/success，已逐项回读核验。
- 首个远端候选：`45d8555ff00229009afb218d18578947805165bb`；[标准 CI #41758681633](https://github.com/bluntvoice/In-Line/actions/runs/41758681633)。
- Git HTTPS 两次推送连接失败；改用 GitHub 连接器上传相同三文件的 Git blob/tree/commit 并创建临时分支。远端 tree 与原本地提交 `0c6da26e93b02409ce95ad2d172e6dedbc480a33` 完全一致；原本地提交保留在 `codex/p3c-local-transport-20261104`，未丢弃。随后以 SHA-1 核对完整 commit 对象导入本地，候选本地/远端一致。此传输差异不代表测试业务失败。

本轮仅在 GitHub-hosted Windows 的 S 旧夹具中启用新增部分；保持原成功请求、顺序、键及一次自有 patch 撤销批准模拟。新增实际工具请求仅 N1～N4。其他既有全套回归按既有 CI 执行，不能把其原有写入或审批模拟统计成本轮新增授权。

## 2. 专项运行环境

新增段的 enabled 门禁在 S 创建资源之前判断。未设置 GITHUB_ACTIONS 时输出 P3C_NOT_RUN，Probe 不创建，因此不授予新增 control、不插入 B 哨兵、不调用 N1～N4、不运行新增审批/文件 oracle。原 S 旧回归保留原行为；不把此标记误解为整个 S 旧用例未运行。

GitHub 环境要求 GITHUB_ACTIONS=true、RUNNER_ENVIRONMENT=github-hosted、RUNNER_OS=Windows，否则明确失败。新增纯门禁测试以独立子测试进程模拟 local、false_github、self_hosted、linux_label、missing_runner，TMP/TEMP 指向不存在的哨兵路径；不执行 S 旧业务，不创建 DB、授权或 sidecar。真实 Linux/macOS 运行器未验证；独立 cfg(not(windows)) 守卫只完成源码核对，Windows 模拟 Linux 标签不能代替原生运行。

查看未执行标记：在非 GitHub 环境使用 `cargo test --locked --manifest-path src-tauri/Cargo.toml --test mcp_queries p3c::p3c_gate_entry_without_resources -- --exact --nocapture`，仅运行无资源门禁入口。查看 S 的专项标记则可对原 S 加 --nocapture，但原 S 本身仍含已有写入回归，办公电脑本轮未运行。默认 harness 可能隐藏标记；passed 不能证明 P3-C 业务已执行，P3C_NOT_RUN 明确表示新增专项未执行。本轮办公电脑只做源码、格式、Git 与脱敏核对，未运行 cargo test 或实际 MCP/CLI。

## 3. 本轮请求与独立预期

| 场景 | 工具/action、身份及键 | 精确预期与核对 | 状态 |
|---|---|---|---|
| 原 patch | mutate_task/patch；S-owner，id1；p3-real-patch | original-first→MCP changed；title version 0→1；taskVersion前进；无关任务字段/字段版本、永久编号、分配表、队列、办理事件、字典、偏好与授权保全；事务回执/审计关联 | 通过（本候选实际执行） |
| 原键重放 | 相同 patch 原键 | 原审计、回执及所有数据库表不变，不产生第二次修改 | 通过（本候选实际执行） |
| 原 undo | request_undo；S-owner；p3-real-undo | needs_user_approval；批准前 title 保持 MCP changed；请求/回执分别增1，业务不变 | 通过（本候选实际执行） |
| 原软件批准 | 仅 db.mcp_resolve_undo(true)，一次自有 title patch | title 恢复 original-first，title version=2；原审计保留，新增1个 approvedUndo 且 undo_of 关联原 patch；原回执不删除，编号/队列/办理事件/字典/授权保全 | 通过（本候选实际执行） |
| 原 batch | batch_mutate/dryRun→apply→原键重放；p4-wire | 预演所有表不变；apply仅新增 p4-stdio-created、独立预期下一个编号和单次分配、版本/回执/审计关联；重放所有表不变 | 通过（本候选实际执行） |
| N1，id801 | request_undo；同A/T的有效R/W control，F=false；p3c-foreign-undo；owner patch审计 | not_found，全部表/授权/文件不变；不增加撤销申请、回执或补偿 | 通过（本候选实际执行） |
| N2，id802 | batch_mutate/apply；owner有效R/W，F=false；p3c-mixed-invalid | 合法完整create p3c-never-created + 已核实不存在的9999 setStatus(version0,pending)；not_found；全部表（含sqlite_sequence）、授权与文件不变 | 通过（本候选实际执行） |
| N3，id803 | manage_business_data/backup；提升W之前owner，R=true/W=false/F=false；p3c-backup-denied | 合法明确意图，forbidden；全部表、授权与文件不变 | 通过（本候选实际执行） |
| N4，id804 | manage_business_data/export JSON，projection=[id,contacts]；owner R/W=true,F=false；p3c-export-private-field | forbidden；全部表、授权与文件不变 | 通过（本候选实际执行） |
| 原 backup | p4-wire-backup；有效R/W，F=false，scope A/T | 仅1个.db，固定合成backups/mcp-created根内；只读SQLite内部核对5条及1条B/T、业务表等于生成前状态、无token/credential/authorization表 | 通过（本候选实际执行） |
| 原 export | p4-wire-export，format=json，原默认普通投影 | 最终共2文件（1DB+1JSON），无partial；JSON4条已知A事项，完整，B/T缺席，私密字段/值缺席；审计及artifact元数据关联、业务与授权不变 | 通过（本候选实际执行） |

N2是实际 stdio apply 请求。现实现会先在 apply 端点内进行回滚模拟和整体校验；此内部行为不等同另发 dryRun，也不声称已到达成功提交后再回滚。

B/T 哨兵仅在 S 原有分页/ID检查、原uncertain-create恢复、原p4-wire重放之后由夹具直接初始化。ID1/2不变；哨兵ID5；文件创建时全库5条、可见4条。其后旧S原有 uncertain batch 仍按原键执行，所以不能把最终源库行数误写成文件快照行数。

## 4. 审计、失败恢复及日志边界

标题保存/撤销会由既有save_task_on刷新对应队列updated_at；本轮精确核对它等于事项本次updated_at，所有队列业务列与编号仍不变。成功单create精确核对新增1条事项/队列/分配、2条审计/2条回执（operation及batch根），3个既有字典项usage_count各+1；失败和预演不允许这些增量。文件还通过现有Descriptor.verify只读检查根目录和最终文件的私有ACL。

原 patch 的 wire verificationStatus=verified_after_commit，存储回执为 verified_in_transaction；batch根回执在外层读回前尚无verificationStatus（null），不能强填与patch相同值。P3-B原偏好语义及同键回放 items:null 的既有表现继续保留。只比较幂等业务状态和真实回执，不以整个复合回包字节相同作为新契约。

每次 N 请求在原 stdio 连接记录工具、action、请求ID、脱敏输入、有效权限/scope、身份别名、PID和创建时间。N1使用独立control stdio，其他请求保持owner原进程。审计ID只显示patch-audit别名；完整行、授权DPAPI字节仅夹具内部比较，不输出原值/哈希。文件真实opaque名称仅内部传递；日志仅输出数量、范围和内容谓词，不上传数据库、导出正文或临时配置。

既有S模拟回包丢失沿用 original-uncertain-key 与 p4-original-uncertain，保留 result_unknown→原键恢复与仅1次create的原断言；不换键盲重试。本轮没有增加清理或恢复覆盖路径。若失败则保留原运行与最小复现，先分清测试实现错误与产品行为，不改产品、不删断言或改错误码放行。

## 5. 逐 action 既有证据与本轮补充矩阵

下列来源与批准清单逐一核对，表中待补的最小项由第3节本轮结果覆盖；复杂action直接stdio缺口保持未覆盖。来源表中的b6312a9运行身份是历史正式证据，不能替换为本轮候选；本轮同名旧回归是否通过另按候选日志逐项回读。

### 5.1 证据等级、运行身份与来源索引

- L1：共享数据库/领域API，含直接构造Authorization的领域测试。直接传Authorization并不证明完整客户端鉴权。
- L2：有效客户端经认证service/IPC的实际路径。CLI调用同一MCP工具可作L2路径辅证，但不冒充MCP JSON-RPC/stdio L3。没有独立service单测的地方明确说明。
- L3：真实in-line-mcp子进程的stdio工具调用。文件名在cli目录中的测试，如确实用rmcp启动sidecar并call_tool，也属于该段的真实L3；同文件其他CLI调用仍单列。
- L4：正式GUI/正式Codex宿主。本清单没有L4证据；测试名含gui只代表调用共享GUI领域API。

以下为准备阶段保留的既有证据索引：下列测试均在b6312a9的标准CI日志中有对应 `... ok`，源码和断言按同一SHA核对。准备清单阶段没有重新运行测试；本轮候选标准CI重新执行的结果见第8节。脱敏日志索引见本地 `release/MCP-Phase3B-Commit-20261103/formal-ci-proof.json` 的summaries；不能把“该测试通过”泛化成测试未断言的action/分支也通过。

| 代号 | 精确文件 / 测试 | 层级与可用断言 |
|---|---|---|
| S | src-tauri/tests/mcp_queries.rs::p2_real_stdio_and_authenticated_pipe_use_bound_snapshot_and_redaction | L3：patch成功/原键重放、undo pending→本地批准→标题恢复、create回包丢失与原键恢复、batch dryRun/apply/重放、backup和JSON export成功；禁止私密canary回显 |
| M | src-tauri/tests/cli/phase4.rs::gui_cli_and_actual_mcp_stdio_share_task_queue_number_event_and_status_rules | L3：create、setStatus(processing)、queue(processRound/enqueue/completeRound)；CLI/MCP/共享GUI领域的事项、队列、编号和事件快照一致。不是GUI界面验收 |
| R | 同文件::cli_and_actual_mcp_concurrent_versions_conflict_and_profiles_cannot_expand_scope | L3+CLI：并发同版本只能一方成功、另一方conflict，总审计1；具体赢家由时序决定，不宣称MCP一定是赢家 |
| C4 | 同文件::cli_real_parent_children_future_scope_completion_and_local_undo_approval | CLI/认证IPC：日期、父子关系、单子排序、undo pending及本地批准；CLI child create走私有cli_create_subtask，不能冒充mutate_task.createSubtask |
| C4W | 同文件::cli_task_edit_urgency_queue_real_work_archive_trash_restore_and_reopen_use_versions | CLI→认证IPC→mutate_task：title、urgent、queue、archive/trash/restore、record/correct/void；逐调用exit=0，末态标题/队列及旧版本conflict。部分action只有成功回码，不能冒充逐步独立状态oracle |
| B3 | src-tauri/tests/cli/phase5.rs::phase5_actual_mcp_and_cli_batch_share_future_numbers_transactions_and_audit_rules | L3：两个operation的dryRun→带expectedBasis的apply、verified_after_commit、编号/业务快照与CLI一致；未来事项不产生办理事件 |
| C5B | 同文件::phase5_batch_forces_preview_no_number_consumption_atomic_conflict_and_local_approval | CLI/L2路径：预演不变、实际apply成功、陈旧预演拒绝、多事项日期返回needs_user_approval、超限resource_limit；拒绝停在CLI预演，不能冒充真实stdio混合apply失败 |
| C5F | 同文件::phase5_saved_queries_preferences_incremental_inspection_and_jobs_use_original_services | CLI/L2路径：export/backup成功、无items正文、只读原键回执不改业务；作业部分不作为本阶段扩展授权 |
| W | src-tauri/src/database/mcp_write_tests.rs | L1：p3_receipts_survive_reopen_concurrent_retry_and_do_not_leak（1事项/1审计、异参idempotency_conflict、权限/scope拒绝）；p3_unrelated_fields_merge_same_field_and_aba_conflict；p3_atomic_failure_rolls_back_business_dictionary_versions_and_receipt；p3_duplicates_disambiguation_whole_text_and_scope_are_explicit；p3_status_no_change_and_gui_share_domain_rules；p3_patch_undo_is_gui_only_preserves_unrelated_and_rejects_conflict；p3_explicit_real_work_and_future_undo_preserve_history_and_reservations |
| P | src-tauri/src/database/mcp_p4_tests.rs | L1：p4_dry_run_and_mixed_failure_preserve_every_business_number_version_dictionary_and_receipt；p4_batch_refs_two_level_structure_and_group_undo_are_atomic；p4_future_creation_queue_real_work_and_correction_keep_linked_history；p4_link_sort_trash_restore_and_indirect_scope_roll_back_as_a_group；p4_scope_all_affected_batch_receipts_and_permission_downgrade_are_enforced；p4_bulk_date_approval_is_once_rechecks_revocation_conflicts_and_preserves_number_history |
| F | 同文件::p4_backup_is_local_full_business_only_durable_idempotent_and_unreadable_via_api / p4_business_export_filters_scope_fields_and_csv_formulas_without_backup_access / p4_artifact_unknown_result_never_regenerates_and_permission_change_rejects_old_receipt | L1/工具纯函数：backup固定根内1文件、业务内容、无token/credential表、同键不增文件；export排除范围外/私密字段、JSON1行、CSV公式加引号前缀保护、私密projection forbidden；保留结果未知与参数/权限变更后的原键拒绝 |
| A | src-tauri/tests/mcp_security.rs::scope_filters_summary_denominator_trend_details_and_cross_client_calls；src-tauri/tests/mcp_connection_acceptance.rs::p2d_d04_original_connection_enforces_live_intersection_scope_pause_and_revocation | L2读/权限交集及L3无有效W的mutate_task.create拒绝。不能把mutate拒绝外推为其他三个工具全部有身份负向覆盖 |

公共路由依据：`src-tauri/src/mcp/service.rs`，四工具均要求有效身份、R及W，进入质量/鉴权检查后再分派；`src-tauri/src/cli/write.rs`及advanced.rs确认CLI实际工具映射。源码规则与运行断言分开，源码本身不算新增L2通过。匿名16工具烟测只证明未认证/限流门禁，不替代有身份的W、scope和归属验证。

### 5.2 mutate_task 逐action映射

TaskAction共有15个顶层variant；queue的6种mode另分行。L2列的“CLI同工具”指已核对CLI映射后的认证服务路径，不是独立service单测。所有“未定位”仅指本次检索到的测试，不推定产品不支持。

| action / mode | 已有L1成功/拒绝 | 已有L2或CLI路径 | 已有L3成功/拒绝与待补边界 | 本轮是否补充 / 状态 |
|---|---|---|---|---|
| create | W：成功、重开/并发重放、重复possible_duplicate、scope/无W forbidden、审计失败整体回滚 | M、S经service；A有拒绝 | M正常create成功并对照业务/编号；S为result_unknown后同键恢复且create审计仅1；A无W拒绝。无需再造成功创建 | 仅复用原S恢复链与M成功，无新增成功create |
| patch | W：独立字段合并、同字段/ABA conflict、全文替换明确意图、scope拒绝；undo后不影响其他字段 | S、R、C4W | S标题MCP changed→批准撤销后original-first、auditId复用；补精确字段版本、回执/审计关联及编号/事件保全，不新增成功patch | 新增S独立版本/审计/回执/保全oracle；通过（本候选实际执行） |
| setStatus | W：no_change不产事件；processed有事件，补偿保留历史 | M、R、C4W | M processing成功；R并发赢家/冲突及总审计1。不是所有目标状态和真实办理分支完整L3 | 原M/R路径直接复用，本轮不加成功操作 |
| setUrgent | W::p3_gui_changes_invalidate_versions_urgent_neighbour_scope_and_undo_are_checked：成功、撤销与排序保全 | C4W同工具set/clear | 未定位直接stdio成功；不将CLI当L3。最小阶段不默认追加催办/义务字段 | 不新增该action的直接stdio；原证据与缺口保留 |
| recordWorkEvent | W/P：真实办理意图不足explicit_intent_required；成功及未来预约/撤销保全 | C4W同工具，固定时间事件并查到note | 未定位直接stdio成功，后续若纳入必须单独授权事件、时间、resultStatus、note及confirmedRealWork，不因合成自动推定真实办理 | 不新增该action的直接stdio；原证据与缺口保留 |
| createSubtask | P::p4_batch_refs_two_level_structure_and_group_undo_are_atomic：父子创建、第三层拒绝、幂等/组撤销 | C4的child命令为私有CLI路由，只作共享领域辅证 | 未定位mutate_task该action的直接L3，不能由CLI私有路由或batch的operation代替 | 不新增该action的直接stdio；原证据与缺口保留 |
| setParent | P::p4_link_sort_trash_restore_and_indirect_scope_roll_back_as_a_group：两子链接、间接scope回滚 | C4同工具set/clear | 未定位直接L3；旧/新父及间接受影响scope需单列，不默认全量补 | 不新增该action的直接stdio；原证据与缺口保留 |
| reorderSubtasks | P同测试：两子顺序实际比较 | C4同工具单子排序成功 | 未定位直接L3；单子成功也不证明多子排序全部边界 | 不新增该action的直接stdio；原证据与缺口保留 |
| queue/enqueue | P/W共享队列/预约保全 | M、C4W | M直接L3成功及跨入口业务/编号快照一致 | 原M/R路径直接复用，本轮不加成功操作 |
| queue/processRound | P未来事项无真实意图拒绝；领域办理历史 | M、C4W | M直接L3成功；不能外推未明确授权时也允许 | 原M/R路径直接复用，本轮不加成功操作 |
| queue/completeRound | W/P事件、补偿/队列规则 | M、C4W | M直接L3成功及跨入口一致 | 原M/R路径直接复用，本轮不加成功操作 |
| queue/reopen | 共享领域经C4W，未单列独立L1场景 | C4W同工具成功 | 未定位直接L3 | 不新增该action的直接stdio；原证据与缺口保留 |
| queue/moveUp | 共享领域经C4W，未单列多项排序oracle | C4W同工具up成功 | 未定位直接L3，不宣称相对顺序完整验证 | 不新增该action的直接stdio；原证据与缺口保留 |
| queue/moveDown | 当前源码支持；本次未定位直接专项成功断言 | 未定位本次CLI用例调用down | 未定位直接L3；本最小建议不补此mode | 不新增该action的直接stdio；原证据与缺口保留 |
| reschedule | P::p4_bulk_date_approval_is_once_rechecks_revocation_conflicts_and_preserves_number_history：多事项本地批准、撤权/冲突、不改永久编号 | C4同工具单事项；C5B多事项pending | 未定位直接mutate_task.reschedule L3；不能将批次日期审批当单事项相同路径 | 不新增该action的直接stdio；原证据与缺口保留 |
| archive | 共享领域经C4W；该用例未独立断言每步归档flag | C4W同工具exit0 | 未定位直接L3；父子联动参数不默认纳入 | 不新增该action的直接stdio；原证据与缺口保留 |
| trash | P间接scope批次整体拒绝、整组trash成功 | C4W同工具成功及旧版本conflict；R的范围外trash为CLI拒绝 | 未定位直接stdio trash成功/拒绝；不将所有权not_found与scope forbidden互换 | 不新增该action的直接stdio；原证据与缺口保留 |
| restore | P父restore后子仍在trash的实际状态断言 | C4W同工具成功，最终deleted_at为空 | 未定位直接L3；是恢复单事项，不是恢复覆盖数据库 | 不新增该action的直接stdio；原证据与缺口保留 |
| correctWorkEvent | P未来创建/纠错链：旧事件void、新事件correction_of关联、撤销回到原办理事实 | C4W同工具并查到纠正note | 未定位直接L3；真实事件/历史影响确认需另批 | 不新增该action的直接stdio；原证据与缺口保留 |
| voidWorkEvent | CLI同工具成功；当前检索未单列该action独立L1专项 | C4W同工具exit0及后续版本冲突 | 未定位直接L3；不能由correctWorkEvent使旧事件void代替该端点 | 不新增该action的直接stdio；原证据与缺口保留 |

上表既有L3路径通过不代表15个顶层action全部完成。最小P3-C建议复用create/patch/setStatus及三个queue mode的L3，复杂父子、归档/回收站、日期、催办和纠错链的直接L3缺口继续登记，若用户要求补齐应另开精确范围，不静默加入本轮建议。

### 5.3 request_undo、batch_mutate、manage_business_data 映射

| 工具 / action或分支 | L1成功/拒绝 | L2 / CLI辅证 | L3及真正缺口 | 本轮是否补充 / 状态 |
|---|---|---|---|---|
| request_undo（无action字段）/申请 | W：pending不改业务、同字段冲突阻止批准、跨client申请not_found；P组撤销、backup/export的undo unsupported | C4调用同工具pending，子项状态在批准前不变 | S实际stdio返回needs_user_approval，标题仍MCP changed；已有成功申请。缺另一个有效写客户端冒用patch审计的直接L3拒绝 | 原S申请补强；新增N1有效同scope归属拒绝及全表保全；通过（本候选实际执行） |
| 软件端批准撤销 | W/P：mcp_resolve_undo(true)成功补偿、版本/历史/编号保全，冲突不部分提交 | C4本地API批准成功，补偿verified_after_commit | S：stdio申请后直接db.mcp_resolve_undo(true)，标题恢复；这是L3申请+L1软件API模拟，不是GUI按钮，也不是MCP approve action | 原S自有title仅1次批准，新增关联/版本oracle；通过（本候选实际执行） |
| 软件端拒绝撤销 | 源码存在mcp_resolve_undo(false)；本次未定位独立成功测试 | 未定位专项 | 不是MCP action；最小建议不增加一次拒绝审批，不标通过 | 未补充该分支，不能由最小验收改判覆盖 |
| batch_mutate/dryRun | P：整库业务/编号/版本/字典/回执不变；超限/歧义拒绝 | C5B有完整预演快照不变 | S直接L3 dry_run且目标事项未创建；B3实际L3预演basis后续绑定apply。可加强原段全量业务保全，不新增同类预演 | 原S全表保全补强；B3 expectedBasis原样复用；通过（本候选实际执行） |
| batch_mutate/apply | P：成功/refs、同键重放、混合失败not_found整体不变、超限/间接scope拒绝 | C5B成功；CLI的预演阻止apply不能冒充apply端点失败；C5未知回包只取原回执 | S实际L3创建1条、同键重放仍仅1条；S回包丢失result_unknown后原键恢复仅1条；B3带expectedBasis成功、编号一致。缺真实stdio混合apply拒绝及独立全表不变断言 | 原S单create/重放补强；新增N2实际apply拒绝；通过（本候选实际执行） |
| 多事项日期/截止调整审批 | P：needs_user_approval、软件mcp_resolve_batch(true)重查撤权/版本、重复批准不重做 | C5B通过CLI仅pending，业务不变 | 本次未定位直接L3 pending→软件批准闭环。最小P3-C不包含多事项改期/截止调整或批次审批模拟 | 未补充该分支，不能由最小验收改判覆盖 |
| manage_business_data/backup | F：1个固定根内.db、同键不重复、可读业务标题、无token/credential表；undo unsupported；未知/变参原键拒绝 | C5F实际同工具成功并确认.db且无items | S真实L3 artifact_created、backup列表存在该名称、回包无私密canary。缺同一stdio请求实际文件数量/内容/隔离范围的精确独立oracle；有身份无W拒绝未单独证明 | 原S 5条全量快照及ACL检查；新增N3无W拒绝；通过（本候选实际执行） |
| manage_business_data/export JSON | F：已知1行、complete、排除范围外/私密字段；私密projection forbidden，业务不变 | C5F成功，无items正文，CLI只读receipt不改业务 | S真实L3 artifact_created、无私密canary，但未读取文件核对内容。缺范围外哨兵和真实stdio私密projection拒绝 | 原S 4条导出及私密排除/ACL检查；新增N4；通过（本候选实际执行） |
| manage_business_data/export CSV | F：公式值前置单引号、私密字段/scope限制 | 本次该C5F执行JSON，不把它记作CSV | 未定位直接L3 CSV。最小建议只JSON，不再生成第三个文件 | 未补充该分支，不能由最小验收改判覆盖 |

backup/export为该工具仅有的两个action；没有任意路径、read/download/restore/remove action。`request_undo`仅申请，不能构造不存在的approve字段或把工具成功回包当成已经撤销。


## 6. 修改清单及保护

候选代码只改三个测试文件：src-tauri/tests/mcp_queries.rs（只增加门禁/专项hook，原调用与业务断言保留）、src-tauri/tests/mcp_queries/p3c.rs（新增隔离oracle与4拒绝探测）、src-tauri/tests/p3c_unsupported_platform.rs（非Windows守卫）。原P3-B模块不变，产品、NSIS、Workflow、依赖、协议、快照/TTL/作业限制均无修改。

本报告及 docs/mcp/acceptance/MCP-PHASE3C-20261104-checks.json 为待验收材料，保持在临时工作树供复核；未经验收不提交正式开发分支。64个历史ZIP、四个历史工作树现存文件、190个旧MCP证据文件已按前置哈希独立回读，全部一致（详见第8节）。

## 7. 风险、阶段状态与 P3-D 建议

- backup现有模型：有效R+W+明确意图即可创建完整业务库快照，不按事项scope裁剪，也不额外要求F。本次仅明确批准的合成A/B数据用于验证；登记为正式上线前待独立确认的安全决策，不据此允许真实客户端获取范围外数据，也不在本轮改权限或直接认定为漏洞。
- 复杂mutate_task action的直接stdio、软件端拒绝撤销、真实GUI审批、多事项改期/截止审批闭环、CSV直接stdio仍未覆盖。
- P2-B自动恢复未完成；P2-C原生12/13、ps-hang未通过；快照限制待评估可用性风险；正向在途cancelJob未完成；正式GUI、Codex宿主、真实旧库迁移未完整验证。
- P3-A与P3-B已正式完成。本次P3-C最小范围自动检查通过，待用户验收；完整Phase2/Phase3/正式发布均未完成。
- 后续P3-D建议只做已有证据身份/工具action/断言/阶段边界的汇总和缺口去重；将backup模型决策单列为上线前必要决策，不用汇总文档或CI绿色替代该决定。真实宿主、GUI、旧库、恢复和在途取消等缺口需另定授权；本轮不执行P3-D、Tag/Release、安装、main合并或正式库操作。

## 8. 最终 CI 独立回读结果

- 候选：`30927f5683faa590e2d91e9a7d10991895ffaa80`，[CI #41787792742](https://github.com/bluntvoice/In-Line/actions/runs/41787792742)，push，既有ci.yml，completed/success。Rust **257/257**，失败0/ignored0；前端 **135/135、22文件**，前端构建、Rust格式、三程序离线身份、MCP16工具完整契约及16匿名拒绝+3次重复/限流均通过。
- 三程序in-line、in-line-mcp、in-line-cli实际探针由未改的verify-cli-package验证exactCommit/baselineCommit均为本候选、sourceDirty=false及版本一致；不把本地构建常量当作运行身份。S/Q/J使用同一实际sidecar，SHA-256：`0d08b8012b1eeaf56ab9c81dcaa3b930500c0b3960bcc69ba3c03202c2ad628b`。
- GitHub-hosted Windows Server 2025；runner2.338.2，镜像win25-vs2026 / 20261030.294.1。S原owner stdio PID 6100、创建时间134382487989340307，所有owner选定请求同一进程；N1独立有效control进程。该PID仅该次runner有效。
- **18次选定四工具调用＝14次原S复用＋4次N拒绝**；107条P3C_EVIDENCE expected/actual记录全部匹配（包含连接事实，不能当作107次业务操作或整个CI调用总数）。5类P3-C门禁检查单列；1次原S软件端批准模拟，非GUI验收。N1/N2=not_found，N3/N4=forbidden，isError=true，拒绝后所有数据库表/授权/文件均不变。
- 标题字段版本实得0→1→2；原patch回执和审计保留，1条approvedUndo补偿正确关联。dryRun与重放全表不变；apply只新增预期1事项/1队列/1分配、operation+batch共2审计/2回执、三字典使用计数各+1。已知编号匹配预设日期格式及预状态next sequence；未从回包反推期望。
- backup-1.db / export-1.json仅为证据别名：实得1DB含5条/1B哨兵，1JSON含4条已知A事项、不含B及私密字段/值，complete=true。固定根/文件私有ACL、最终数量2、无partial及审计关联均通过。文件正文留在夹具内部，没有作为日志或Artifact上传。
- P3-B的**47次调用/104条原业务断言**与b6312a9正式CI逐项匹配（只将精确SHA事实归一为当次SHA），权限/scope/参数/状态保持；原5门禁也通过。Q继续monday v1→sunday v2→monday v3、两条回执/审计及补偿完整，J仍是已完成作业双waiter与waiter取消，在途cancelJob没有改判通过。
- S、M、R、B3、C4、C4W、C5B、C5F及W/P/F领域测试在本候选日志均为ok。源码矩阵原b6312a9证据身份保留；本候选通过仅支持各行明确已有断言，不扩大成所有复杂action的L3通过。
- 四个历史工作树共865/837/839/845个既有文件分别全部保全；HEAD和状态均未改变，64个历史ZIP及190个旧证据文件哈希全部一致。正式远端仍b6312a9；main仍1baac96252af2665274f2a955b3adacfa80519fd。

本轮交付修改共5文件：3个测试文件已提交到临时分支；本报告和脱敏JSON为本地新增待验收材料，未提交或推送正式分支。候选相对正式基线只有3个测试文件差异，原S源码仅追加hook，0行原源码删除；P3-B模块Git blob不变。原4次失败、所有候选和历史P3-B三轮失败/补偿差异均保留，未替换原始证据。

到此停止，等待P3-C验收；不执行P3-D，不提交feat/scheduled-items，不创建Tag/Release、合并main或安装软件。

## 9. 正式提交授权记录

2026-11-04（Asia/Shanghai），用户已通过GitHub核对候选30927f5683faa590e2d91e9a7d10991895ffaa80及原始CI #41787792742，确认P3-C最小隔离业务验收通过，并仅批准本报告、对应脱敏JSON及三个测试文件正式提交至feat/scheduled-items。前三个测试文件保持候选Git blob完全一致；上文各候选SHA、四轮失败、成功运行、P3-B证据、107条专项证据及未覆盖范围均为冻结历史记录，其原“待验收/未正式提交”文字描述当时停止点。

本次从正式基线b6312a9dad8d25f0d8f4d6332182e153ab8b3a42新建独立五文件提交，不合并临时分支。提交本身不代表新SHA CI已经通过；其运行身份、结果和收尾状态另见docs/MCP_PHASE3C_COMMIT_RECEIPT.md，防止把候选二进制身份移用到正式提交。

backup现有R+W+明确意图、全量不裁scope且不额外要求F的模型，未被本次验收批准用于正式环境。上线前必须独立决定：哪些客户端角色可触发完整备份、是否需要FullRead或更高权限、完整备份的落盘访问/保留/清理/审计规则，以及其他路径访问范围外数据的风险。未改权限模型；这些决策不得由CI通过代替。

后续仅准备P3-D证据汇总方案，不开展新的业务测试、批准模拟、文件生成或正式宿主操作。P2-B/P2-C、快照可用性、正向在途cancelJob、复杂事项直接stdio、正式GUI/Codex宿主/旧库迁移及完整Phase2/Phase3/发布的未完成边界不变。
