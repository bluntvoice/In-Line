# In-Line v0.5.0 统计、时间线与备份恢复集成 PRD

| 项目 | 基线与状态 |
| --- | --- |
| 定稿日期 | 2026-11-24（北京时间） |
| 开发基线 | `feat/scheduled-items`，`af3becb8994108a34a78011147cdae7a31ad26a4`，业务 schema v14 |
| 产品输入 | `feat/f1-f2-optimizations`，`eaccb54ab494a30770b35f2721dc5c11ee720ca4`。参考 `d951e2e`（需求）、`b9befb0`（F1）、`823759d`（F2）、`eaccb54`（F3） |
| 共同祖先 | `main`，`1baac96252af2665274f2a955b3adacfa80519fd` |
| 集成分支 | `codex/v050-f1-f2-f3`。本轮为阶段 0 PRD 定稿及提交前静态核验 |
| 关联规范 | [未来事项](future-items.md)、[备份恢复](backup-restore.md)、[发布与测试](release-testing.md)、[项目协作规则](../../AGENTS.md)、[P3-D 发布门禁](../MCP_PHASE3D_RELEASE_GATES.md) |

本 PRD 以 v0.5.0 的现有实现为起点。F1 补齐本月、本季度及同期比较。F2 将同一业务动作的自动日志归组展示。F3 让已存在的待复核冲突副本在有充分对应证据时安全复用。本轮仅建立独立 Clone、本地分支及定稿本文件。产品源码变更、数据库迁移、测试、Git Commit / Push 和自动 CI 分别依后续授权执行。

## F1 七种统计范围

### 日期规则

| 选项 | 本期，含首尾自然日 | 对比期 |
| --- | --- | --- |
| 本周 | 按设置从周一或周日起算至今天 | 两端各退七个自然日，即上周同期 |
| 上周 | 上一个完整自然周，同一周起始设置 | 之前一个完整自然周 |
| 本月 | 当前自然月首日至今天 | 上月首日起取相同自然日数，最多至上月末 |
| 上月 | 上一个完整自然月 | 之前一个完整自然月 |
| 本季度 | 当前自然季度首日至今天 | 上季度首日起取相同自然日数，最多至上季度末 |
| 上季度 | 上一个完整自然季度 | 之前一个完整自然季度 |
| 自定义 | 有效的用户输入日期，允许跨月、季、年 | 结束于本期开始前一日，长度与本期自然日数相等 |

以本机本地日历计算今天、周首、季度首和加减日。自然日长度用日历序号计算，覆盖闰日和 DST。查询参数沿用 `StatisticsPanel.tsx` 的本地首日零时至末日次日零时半开区间，向现有 IPC 发送时间戳。空值、无效日期及逆序显示校验信息，同时阻止加载、导出和生成报告。现有 IPC 和按本机偏移处理趋势的逻辑照用，跨 DST 的后端日桶与半开边界分别验证并记录。

### 交互与统计数据

- 顺序按上表，窄窗口自动换行。复用 `App.tsx` 的 `StatisticsPeriodSelection` 和 `period/onPeriodChange`（`41383aa`），跨页面、详情开闭保有预设和先前输入的自定义日期，保持到主窗口关闭。周起始设置及跨自然日更新后，当前预设重新计算，自定义日期保留。
- `StatisticsPanel` 将期初期末、本期及对比结果作为同一查询快照提交。请求中新选择立即使旧响应失效，卸载后的响应也不更新视图。当前数据、类型明细、趋势柱体、PNG、原生报告与提示词必须匹配生效的查询范围。加载、无效输入或请求失败期间禁止用过期结果按新标题导出。类型明细和趋势钻取应有自己的请求有效性检查。
- 保留 `4194f2b` 的季度周桶显示和 PNG 逻辑。原来大于 62 天用周桶，现行周桶周一锚定，周起始设置作用于周预设。`statisticsTrendRange` 将桶区间裁到本期边界，钻取按裁后半开区间。本周延伸的空柱继续按现有规则展示，期外空柱无钻取。PNG 使用当前快照和对比标签，原生报告沿用 `displayRange`，提示词保留报告类型、开始及结束日期三项替换。原生报告的在办逾期数沿用当前队列快照。
- 处理量继续按 `task_work_events.handled_at` 选有效事件，`statistics_scoped`、`statistics_details_filtered` 及 `mcp_metrics.rs::RANGED_CTE` 复用现有归类、按事项去重、期内择最新和同秒 ID 次序。保留完成率/处理率原分子分母及部门、事项类型、父子任务和各分桶的统计口径。预占和自动入队本身无办理量。晚入队的队列生效日仍可归计划日并影响既有处理率分母，实际办理只归办理日。重新开启不产生办理量，已重新计划事项的历史有效办理继续统计。

实现顺序为扩展 `statistics-range.ts` 日期映射 → 面板与请求一致性 → 两组报告映射、样式和验证脚本 → 定向统计回归。参考 `b9befb0` 的纯日期用例，保留 v0.5.0 现有状态类型和季度脚本代码。

## F2 事项时间线与 v15 迁移

### 逻辑业务动作

- 现有 `task_logs` 保存用户可见业务说明。`status_history`、`task_queue_entries`、`task_work_events`、`urgent_records` 和 `queue_number_allocations` 各自保存业务事实。仅在 GUI 事项时间线折叠自动日志的展示。
- 批准后在共享领域动作内部生成高熵、带命名空间的关联标识，例如以 SQLite `randomblob(16)` 加原事项维度生成。一次主动作中的取号、状态改变、日程、加急等派生写入继承上下文。每个批量子操作有独立关联，即使同一事务内对同一事项多次修改。跨事项动作各按原事项分组。关联标识只用于展示与历史保持。待批、拒绝、失败回滚、幂等重放均不得新增业务日志。
- GUI 由 `Database` wrappers 和 `application::complete_in_transaction` 进入共享层。MCP 经 `mcp_transaction → mutate_on → mutate_with → execute_action/mcp_complex::execute`，批次经 `mcp_batch.rs::run` 逐项调用。CLI 普通写走现有 IPC，私有子任务和组完成也调用共享领域。沿 `database/domain.rs` 及实际调度入口传递关联，覆盖创建、编辑、状态、加急、办理、调度、移入/移出回收、归档和合并。沿用入口现有鉴权及事务。
- `activate_due_scheduled` 批量到期扫描中的每项独立关联。手动提前、改期、恢复或办理触发的嵌套写入属于该事项主动作。保留原计划日、实际执行时点和号段变更原文。单纯调序、配色等现有无需业务日志的动作不补造日志。
- 关联与 `log_type`、`content`、原时间随首次 `INSERT` 写入。禁止使用全事务水位去标记批次内多个业务动作或事后 UPDATE 类型。`mcp_changes` 对每次真实 INSERT/UPDATE/DELETE 生成增量事件；分组本身不能额外写两次。
- `get_logs` 返回 GUI `TaskLog` 的可选 `operationId`，按现有逆序显示并以记录 ID 处理同时间。前端可复用 `taskTimelineEntries` 及键盘可操作的 `details`，按当前事项和来源操作标识归组。保留展开原条文字和时间，缩短仅限已知类型，未知或无可靠标识的历史独立显示。备注独立、原字、保留编辑删除。事项中的业务 audit 提示也逐条显示。切换事项时使过期日志响应失效。

### 迁移、兼容与回退

1. 首选 `task_logs.operation_id TEXT NULL`。在现有 `Database::migrate` 事务中按版本守卫追加列，更新 `schema_meta` 为 v15 并校验数据库。原 `log_type`、内容、行 ID、时间、索引与触发器保留。历史默认 NULL。禁止按时间或文字推断关联或回写历史。测试新库、v8→v14 控制组、v8→v15、v14→v15、迁移失败和 v15 重开。
2. 生产打开路径沿用 `Database::open → open_root` 的 `before-migration` 一致性备份。`Database::open_at` 仅适用于合成库的领域和迁移测试，绕过这条生产备份路径。迁移前证明备份可读、版本正确、完整性为 `ok`、外键违例为零，记录 SHA 及核心业务快照。迁移失败回滚 DDL 和版本，不让失败库进入业务路径。恢复演练在独立合成库完成。旧 v14 程序按现行版本守卫拒开 v15。回退时先停止新程序并保全新库和升级后新增数据，再由获批的恢复流程使用匹配时点的 v14 升级前副本启动旧程序。升级后的写入不会自动随旧副本回退，须单独核对。
3. 解码原 v0.4.0 F2 的 `timeline:v1:<32位十六进制随机值>:<原事项ID>:<类型>`。按语法及已知自动类型验证，排除备注和审计类型。原事项 ID 可与当前归属不同。新列有可靠标识时优先使用新列，合法旧前缀只读解析，不写回历史原值。无效前缀无分组资格，MCP 仍按既有关系/合并敏感文本策略脱敏。
4. 源备份副本先在隔离暂存位置完成兼容迁移。恢复复制新列和可信旧原值，任务合并移动日志时保留来源标识。可关联日志的去重比较基础类型、来源标识、内容、时间和同键记录数，分别验证同源重复恢复、不同动作同秒、合法同组重复和跨库移入。无关联旧行保持旧兼容策略。备份恢复产生的本地改号说明与被恢复历史分开。
5. `mcp_history` 逐条返回原契约。对合法旧前缀保持 `kind=log` 并在 `from` 投影基础业务类型，照用关系/合并的脱敏规则。操作标识不进入 MCP 契约、幂等键、回执或安全审计。核对 `mcp_read.rs::mcp_basis` 从常量返回的业务 `schemaVersion` 从 14 变为 15，缓存和游标按现有校验及重同步语义工作。业务增量、查询与统计定义、授权 schema、`mcp_ai_audit`、审批、撤销和 G00 安全帧保持各自契约。

本轮静态确认可空列方案与现有迁移钩子及日志消费点匹配，阶段 2 仍需批准实际数据库源码改动、合成迁移和验收预算。若后续要求维持 v14，须另行审核编码在原 `log_type` 的方案及其投影、旧读者和增量风险。

## F3 GUI 备份冲突副本复用

沿 `Database::restore_backup` 的校验、暂存副本迁移、`before-restore`、目标锁及事务、非覆盖复制与代际递增执行。识别复用的副本计入合并数，继续走现有历史归并。新冲突继续显示在现有结果中。阶段 3 默认无额外 schema 变更。

1. 在目标事务内先规划源事项 ID 到目标事项 ID 的一对一映射。沿用原题路径；若查证该路径漏判重要业务差异，只在匹配环节补足。待复核副本必须具有 `is_import_conflict`、相同源 `created_at` 及 `conflict_title` 生成的精确标题形态，如 `（冲突）`、`（冲突 2）`。只有一个候选同时通过下列业务、关系和历史证据且目标未被另一源事项占用，才自动复用。已消除冲突标志、已改名、信息不匹配或存在多个可用候选时，保留独立冲突。
2. 比较 `same_task_content` 既有部门、联系人、事项类型、详情、状态、优先级、工作量、加急、截止时间/标签、内部备注及办理生命周期时间。以现有规则规范化事项类型和单事项 `ticket_color`，补齐 `planned_date`、`is_scheduled`、`schedule_action`、`schedule_action_at`。编号、数据库 ID、管理时间 `updated_at` 和绝对排序值具有本库语义。对父子建立映射不能将源库 ID 或排序值直接用于身份判断。
3. 先确定父级映射再验证子级；源根与目标根、已映射同一父级的子任务才具备关系条件。不同父、缺父、自环、父歧义或本地已重新挂父时不复用旧副本。沿用 `validate_parent_assignment` 和现有相对顺序处理。保留现有目标父关系。
4. 核验当前号码、预占/激活、有效和已关闭队列、作废号、高水位及历史关联 ID。保持 `imported_identity`、`copy_queue_entries`、`merge_number_history_on` 的合法重编号。来源号与目标号不同，需用候选实际 owner、可核实的原号证据和对应队列/分配记录证明唯一映射。单一文本 marker、同号或标题各自不能建立对应。原主号录入目标时 `allocated_at` 可被本地赋值。依实际允许的导入变换比较，验证他方占号、被作废和无主历史，不重领号或转移既有当前 owner。
5. 核对状态转换、队列区间、加急、备注和 F2 日志标识，以及办理、作废和 `correction_of` 纠错链。结构化 ID 通过事务内映射对照。可以忽略已证明本地生成的改号日志和身份改写，仍要保持已有审计、历史基数和当前待批/回执。无法证明对应关系时留独立冲突；若核心重编号或父子夹具重复恢复仍增副本，保留失败证据并重新审批来源标识设计。
6. 成功复用后以逐业务表数量、父关系、号码和历史证明二、三次恢复稳定，同时记录文件大小和页数。允许业务代际按基线递增，以及合法导入文件和 `before-restore` 增加。数据库快照与备份目录总容量分开核对。失败回滚并保留源备份及可用恢复前副本，暂存副本和 sidecar 由现有流程处理。

F3 局限于 GUI 合并式恢复。恢复权限、G00 完整备份/整库恢复、安全存储、正式安装和清理规则仍按原门禁执行。


## 分阶段计划与执行边界

后续各阶段先冻结 SHA、预计改动、测试入口、隔离环境和副作用预算。预算应按获准用例列出合成 DB、迁移和备份数量、文件与目录写入、保留/删除、进程与端口、磁盘、运行时间及失败留存位置。执行结果按 `PASS/FAIL/NOT_RUN` 记录，旧 SHA 结果只作基线证据。

| 阶段 | 预计改动与常规测试 | 环境及数据副作用 | 提交前完成条件 |
| --- | --- | --- | --- |
| 0 独立环境与 PRD | `docs/prd/f1-f2-optimizations.md`。核对基线、Git 隔离、文档和 CI 触发 | 独立 Clone 的本地 Git 对象、refs、签出、PRD 写入。运行环境只规划 | 初验确认文件、引用与 31 项清单，按用户批准再提交推送 |
| 1 F1 | `src/lib/statistics-range.ts`、`src/components/StatisticsPanel.tsx`、`native-report.ts`、`report-prompts.ts`、`src/styles.css`、`scripts/verify-ui.mjs` 及日期、报告测试。沿用 `src/App.tsx` 状态绑定，Rust 仅新增必要统计回归夹具 | E1 桩、回环浏览器、导出 stub，E2 合成办理与调度事实。真实 GUI/剪贴板/PNG 属 E4 | F1-01 至 F1-09 获准范围通过。原生证据如获准安排在阶段 4，阶段记录注明待验 |
| 2 F2 | `src-tauri/src/database.rs`、`database/domain.rs`、`database/scheduling.rs`、`models.rs`、`src/types.ts`、`TaskDetail.tsx`、`task-timeline.ts`、样式和测试脚本。沿实际调用链最小适配 `application/mod.rs`、`database/mcp_write.rs`、`mcp_complex.rs`、`mcp_batch.rs`、`cli.rs` 及旧前缀的 `mcp_read.rs` 投影 | E1 展示桩，E2 合成库写入、迁移前备份、回滚、业务触发器验证。E3 授权、宿主、回执、stdio 另批 | F2-01 至 F2-08 的必需证据，包含旧库、恢复、共享路径及其授权保护。迁移和回退经独立验收 |
| 3 F3 | `src-tauri/src/database.rs` 的匹配及测试，按已证明的号码用例最小适配 `database/scheduling.rs`。必要时同步备份 PRD 与 CHANGELOG | E2 合成导入、迁移、恢复及重试，临时副本、恢复前文件和 SQLite sidecar。E4 实际设置页另批 | F3-01 至 F3-07，结构化记录完整、无不当复用、重复恢复业务数量稳定、失败保全 |
| 4 整体回归 | 复用基线构建、原生及 NSIS 脚本，交付同一候选 SHA 的回归记录。任何修复按获准范围再验 | E1–E5 按行审批。包括原生 Windows、CLI/MCP 宿主、NSIS、CI 和独立副作用记录 | I-01 至 I-07 必需证据齐备，经用户最终确认。G00 候选和发布各按独立门禁 |

### 环境和路径规划

代码根 `D:\codex-data\projects\inline\In-Line-v050-F1F2F3`。后续本机产品合成验证拟用独立同级根 `D:\codex-data\projects\inline\In-Line-v050-F1F2F3-runtime`，按 `<阶段>\<候选SHA>\<运行ID>` 分隔。阶段 0 只完成路径规划。后续使用前核对祖先、文件所有权、重解析点、剩余空间和实际进程写路径。

| 环境 | 拟用路径及独立执行条件 |
| --- | --- |
| E1 普通产品前端 | 运行根的 `npm-cache`、`temp`、`browser`、`exports`、`evidence`。已批准的依赖和生成目录放在新 Clone 或独立运行根，Vite 仅回环，浏览器临时 profile。API、剪贴板和报告来源用合成桩 |
| E2 普通产品 Rust/SQLite | 运行根的 `cargo-home`、`cargo-target`、`temp`、`cases\<用例>\data`、`evidence`。用 `Database::open_at` 指向每例新合成库，备份、迁移、失败和 sidecar 计入额度。需要验证生产迁移守卫和备份时另走 `open_root`，例如以隔离 debug 入口和合成数据根进入。固定 `Cargo.lock`，核验 MSVC、SDK 及实际工具位置 |
| E3 CLI/MCP 端到端 | 经独立批准的 Windows 隔离身份及其专用 `data`、`security`、`cli-profile`、接入配置、pipe、TEMP、证据根。核实二进制 SHA、进程、身份、授权文件和 IPC。共享领域的纯业务验证可走 E2；实际宿主、授权、审批、stdio、DPAPI、接入包及真实账号分别审批 |
| E4 Windows 原生/安装 | 指定隔离账户或 VM，候选 EXE、WebView2、app identifier 和 profile、包与输出路径。debug 环境核实 `IN_LINE_MCP_TEST_DATA_ROOT` 和实际 WebView 数据目录，release/NSIS 使用独立 Windows 环境。为安装、升级、启动项、注册表、权限、剪贴板和回退锁定副作用 |
| E5 同 SHA GitHub CI | 用户批准具体 Commit / Push 时写明自动 CI；记录 run、SHA、runner、仓库权限、日志和依赖缓存。Windows 托管原生及手动 artifact 工作流另行批准 |

保护原 `In-Line`、`In-Line-F1-F2`、登记的 P2-D/P3-A/P3-B/P3-C、共享 `.git`、已有 `src-tauri/target/g00-*` 和整个 G00-I1 交付目录。运行路径不得指向原构建、用户正式数据、凭据、备份或当前宿主。普通 Clone 与 TEMP 重定向不证明运行/身份隔离。G00 为 `open`，I1 初验未通过，隔离执行 `BLOCKED`。冻结候选执行继续要求获批的独立 VM、标准测试 SID、离线、无宿主可写挂载、独立工作及证据卷、事前外部硬限额和独立观测实证。

### Commit / Push 与 Windows CI

每阶段提交前先给出本地结果、精确文件、候选基线、所需测试及待验证项目。用户明确批准具体 Commit / Push 后才推集成分支，并回读同一 SHA 的远端和 Actions。必要的 CI 与业务证据通过、用户最终验收后，再按下一阶段授权推进。未获批的项目记 `NOT_RUN`，保留失败记录。

已核对 [标准 CI](../../.github/workflows/ci.yml)。`push.branches` 包含 `codex/**`，且无路径过滤，因此仅含 PRD 的推送也会触发 `windows-latest` 的 `Verify Windows build`；PR 也是触发入口。runner 以 `contents: read` checkout 已提交代码，检查 Actions 运行时策略，装 Node 24 和 Rust stable/rustfmt，读写 npm/Rust 缓存，执行 `npm ci/test/run build`、`cargo fmt --check` 和 `cargo test --locked`，核对三个 debug EXE 与 `GITHUB_SHA`，运行实际 MCP sidecar 的契约及匿名 stdio/认证检查。日志、依赖和缓存流量、构建产物、Runner 时间，以及现有套件的临时合成库、备份、profile、IPC 和子进程属于该次执行副作用。匿名检查本身不使用正式凭据或业务宿主，整个 Rust 测试套件含宿主相关夹具，应按实际候选用例检查。获批的运行不得通过伪造 `GITHUB_ACTIONS` 取代托管条件。

[开发构建](../../.github/workflows/dev-build.yml)和[原生 NSIS 验收](../../.github/workflows/native-nsis-acceptance.yml)是手动工作流，[正式 Release](../../.github/workflows/release.yml)由 Tag 触发。标准 CI 流程不发布安装包或 Release。阶段 0 候选提交范围只有本 PRD，推送及其自动 CI 待批。

## 31 项验收检查清单

各项初始状态为 `NOT_RUN`。逐项执行后填写候选 SHA、完整测试名或脚本模式、隔离根、实际副作用、判定及日志位置。前端定向采用 `npm.cmd test -- <用例路径>`，浏览器复用 `node scripts/verify-ui.mjs <mode>`。Rust 用 `cargo test --locked --manifest-path src-tauri/Cargo.toml --lib <模块::函数> -- --exact` 核对模块路径再执行。实际 CLI/MCP、托管 CI 和原生操作采用各自获批入口。

### 阶段 1 · F1

| 检查项 | 执行入口与动作 | 环境及副作用 | 通过条件 |
| --- | --- | --- | --- |
| [ ] F1-01 七范围 | 移植 `b9befb0` 的 `src/lib/statistics-range.test.ts` 和 `verify-ui.mjs periods`。固定今天，逐项点选，检查本期和对比参数 | E1，API 桩和浏览器 profile | 七项各一次、顺序和默认正确，头尾日期一致 |
| [ ] F1-02 周起始 | 日期单测与浏览器切换 `week_start_day`，覆盖周六/周日/周一和跨年 | E1，假设置 | 当前周截至今天、上周完整，对比移动 7 或 14 天，现有趋势周桶锚点正确 |
| [ ] F1-03 月季闰年 | 复用日期测试，新增非闰 2 月、四个季首、季度跨年以及月/季整期对比 | E1，无业务数据写入 | 例如 2024/2025 年 3 月末分别截至 02-29/02-28；季度同期按自然日等长截断 |
| [ ] F1-04 自定义与时区 | 单测跨年/跨季、单日、空白/逆序及 DST；需要时用 E2 核对日界实际 IPC 聚合 | E1 假日历，E2 合成事件库 | 半开边界无漏无重，自定义前期等长；无效日期无查询/导出；DST 日桶与基线差异单列 |
| [ ] F1-05 状态与乱序 | `periods` 模拟快切、旧请求晚回、详情与切页、日期和周设置变化 | E1，延迟桩 | 自定义值与选择保留，过期回包不更新视图、报告或导出 |
| [ ] F1-06 指标、对比及分类 | `statistics_use_latest_event_and_guard_historical_attribution`、`statistics_distinguish_top_level_tasks_and_subtasks`，加两期有区别及边界夹具 | E1+E2，合成业务行 | 摘要、完成/处理率、周期对比、部门及类型、父子拆分同源且符合预期数量 |
| [ ] F1-07 趋势、钻取和 PNG | `verify-ui.mjs trend/periods`、`src/lib/statistics-trend.test.ts`、`tests/statistics-export.test.ts`，造 62/63 天和两端半周 | E1，导出桩；真实保存另批 E4 | 钻取入参裁切、未来空柱无查询，PNG 日期及对比标签一致，真实导出判定 300 DPI |
| [ ] F1-08 原生报告、提示词 | `tests/native-report.test.ts`、`verify-ui.mjs periods`，按事项类型生成并检查本月/本季和三占位符 | E1，复制桩；真实剪贴板 E4，线上生成 E3 | 类型、日期、数值与当前快照一致，逾期遵循现行规则，提示词按预期替换 |
| [ ] F1-09 计划日和办理日 | `due_scheduled_is_idempotent_keeps_reserved_order_and_skips_non_pending`、`future_early_workflow_all_entry_points_and_defer_override`、`reactivation_retains_historical_statistics_clears_deadline_and_starts_new_cycle`；加计划/办理分两期、子任务及作废 | E2，合成队列及办理 | 未到期无该计划办理量，延迟入队分母按基线归属，办理按真实日；重开、多轮及作废按期统计正确 |

### 阶段 2 · F2

| 检查项 | 执行入口与动作 | 环境及副作用 | 通过条件 |
| --- | --- | --- | --- |
| [ ] F2-01 分组、展开、备注 | 复用 `823759d` 的 `task-timeline.test.ts`、`verify-ui.mjs timeline` 和 `timeline_operations_keep_repeated_actions_notes_and_audit_independent`。造相同时间不同操作及失败 | E1+E2，日志写入和回滚 | 一操作一组，独立动作及备注逐条、键盘展开可读，失败无新增，首次 INSERT 之外无标记 UPDATE |
| [ ] F2-02 旧库与前缀 | `timeline_operations_survive_restore_merge_and_legacy_import`、`production_upgrade_from_v8_preserves_data_and_creates_pre_migration_backup`；加合法/无/异常旧前缀、新列优先和 v14→v15 | E2，合成迁移、备份、重复读 | 可信 token 可读，无标识行保持独立，原业务类型和文本完整，原数据库副本可核对 |
| [ ] F2-03 父子、批量完成和关系 | `database/application_tests.rs`、`create_subtask_on_domain` 的现有测试；另验证创建子项、同秒批量动作、脱钩、调序、完成 | E2，合成层级及写入；CLI 流 E3 | 组以事项为边界，保留子任务、队列、关系；调序无伪日志 |
| [ ] F2-04 调度、提前、重开 | 既有调度用例增加自动批次两件、同事务提前和办理、重新计划、暂缓、取消和停止 | E1+E2，合成计划运行和日志 | 组分明，计划、实际和作废事实均可展开，失败回滚无关联残留 |
| [ ] F2-05 合并、回收和恢复 | `merge_tasks_preserves_history_and_deduplicates_events`、`archive_delete_restore_and_permanent_cleanup_apply_only_the_selected_scope`，加多次恢复、同组重复基数和移入已有行 | E2，合成移动、回收、迁移、备份 | 旧记录不重标，有效/作废事实完整，同源不扩张、同秒不同来源不误去重 |
| [ ] F2-06 MCP/CLI 写入、批次、重放 | `mcp_write_tests.rs`、`p4_batch_refs_two_level_structure_and_group_undo_are_atomic`、`cli_tests.rs`、`cli_foundation` phase4 共享路径；同一事项多动作、拒绝、pending、重放、批准补偿 | E2 领域；E3 真实端点/身份/审计文件 | 与业务动作对应的日志组、批次原子性、回执、权限和安全审计一致 |
| [ ] F2-07 历史和增量契约 | `mcp_history`、`gui_transactions_and_failed_batches_do_not_leave_derived_events`、`p3b.rs`/`p3c.rs` 对照，检查有限 scope、异常旧前缀、schemaVersion 和游标 | E2 基线与合成触发器；E3/E5 接入或托管 | 日志仍逐条、保持基础种类和脱敏，无标识泄漏；增量仅计真实行变更，v15 公开版本及重同步符合预期 |
| [ ] F2-08 纠错与作废 | `void_work_event_on`、`mcp_complex::execute`、`mcp_p4_tests.rs`，加事件更正、作废、备份同源性核对 | E1+E2；实际端点/确认 E3 | 业务审计、有效/作废和 correction 链完整，统计排除作废项，安全审计仍独立 |

### 阶段 3 · F3

| 检查项 | 执行入口与动作 | 环境及副作用 | 通过条件 |
| --- | --- | --- | --- |
| [ ] F3-01 同备份连续三次 | 移植 `eaccb54` 的 `restoring_the_same_conflict_does_not_expand_the_next_backup`，与 `backup_restore_merges_tasks_preserves_conflicts_and_applies_settings` 对照，先占用 `（冲突）` | E2，3 轮合成恢复、快照 | 首轮 1 新增/1 新冲突，后两轮 0 新增/1 合并/0 新冲突，3 事项及业务表、快照页数/体积稳定 |
| [ ] F3-02 不同内容与歧义 | 制造不同详情、未来日期、状态、备注和配色的同名同时间，已解标志、改名、双候选和目标已被占用 | E2，冲突夹具及恢复 | 完整保留独立冲突与本地原记录，无误复用，已复核状态保持 |
| [ ] F3-03 父子和排序 | `backup_restore_maps_parent_ids_in_two_passes_without_overwriting_current_relation`，加晚 ID 父、同名异父、父重编号、缺父及重挂 | E2，ID 映射与恢复 | 同源同父稳定复用，异父或不明关系不匹配，现有顺序和父关系保全，无自环/孤儿 |
| [ ] F3-04 预占、重编号、作废 | `future_backup_preserves_reservations_deleted_history_and_rolls_back_atomically`、`backup_history_cannot_void_an_existing_local_current_number`；重映射同源连续恢复，对照异主同号、双 marker、无主和到期 | E2，真实合成号码账簿 | 预占/有效队列唯一，现任 owner 不变，旧号可回读且不再分配，重试不重领号且不重复复制 |
| [ ] F3-05 颜色及历史数据 | `task_ticket_color_is_independent_persistent_and_preserved_by_edits_and_backup`、`backup_restores_valid_ticket_colors_and_skips_corrupt_color_settings`；加入同一 F2 token、纠错链、不同队列/状态、备注 | E2，业务表和回执哨兵 | 颜色、关系、有效/作废链和历史不覆盖或串号，同源业务计数稳定 |
| [ ] F3-06 恢复前快照、失败与体积 | 复用回滚用例，注入合成业务事务失败，核验 `before-restore` 重开、源散列、暂存清理及代际。报告数据库和目录体积 | E2 备份、恢复和临时删除额度；实际 GUI E4 | 业务失败原子回滚，恢复前备份有效，成功后无非预期业务体积增长，文件保留符合规则 |
| [ ] F3-07 v8/较高版本备份 | `production_upgrade_from_v8_preserves_data_and_creates_pre_migration_backup`、`imported_backup_uses_import_time_and_is_listed_first`。v8 带/无旧标签、损坏/超版本，同一有效源至少两次 | E2，备份的暂存迁移 | 有效旧库可恢复且同源稳定，源散列一致，损坏或更高版本走基线拒绝 |

### 阶段 4 · 集成回归

| 检查项 | 执行入口与动作 | 环境及副作用 | 通过条件 |
| --- | --- | --- | --- |
| [ ] I-01 v8→v14→v15 与回退 | 旧 SHA 运行 `production_upgrade_from_v8_preserves_data_and_creates_pre_migration_backup`，候选验证 v8→v15、v14→v15、新库、回滚和重开 | E2，预迁移快照及新库；安装路径 E4 | 版本、业务行、号段、历史、设置与备份可回读，旧程序拒开 v15，获准旧副本回退成功 |
| [ ] I-02 旧备份与完整性 | `Database::check_integrity` 及恢复测试；独立只读连接对库和备份执行 `PRAGMA integrity_check`、`PRAGMA foreign_key_check` | E2，读合成库；恢复本身在已批额度内 | integrity 仅 `ok`，外键违例 0，业务行和重复恢复计数符合夹具 |
| [ ] I-03 GUI、WebView2 和 DPI | `verify-ui.mjs trend/rows/layout/periods/timeline`，实际建项、入队、办结、重开、合并、回收与备份；默认及 125/150/200% DPI | E1，实际会话 E4；合成资料、临时 PNG/剪贴板 | 从真实入口可操作，窄窗口无关键裁切，状态和报表一致，键盘展开可用 |
| [ ] I-04 MCP/CLI 契约 | `tests/cli_foundation.rs` phase4/5、`tests/mcp_queries.rs` scope/历史/增量，合成库与真实端点分开测 | E2 逻辑；E3/E5 已批宿主和托管用例 | 输入/返回、并发、权限、脱敏、审批、重放、回执及审计符合契约，版本变化可解释 |
| [ ] I-05 前后端构建和回归 | `npm.cmd test`、`npm.cmd run build`、`cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`、`cargo test --locked --manifest-path src-tauri/Cargo.toml` | E1+E2，混合宿主用例 E3/E5；缓存、编译、fixture 与子进程 | 同 SHA TypeScript/Vite、Rust 编译及已批准的完整套件通过，定向测试单列 |
| [ ] I-06 NSIS 安装、升级与回退 | `scripts/test-native-nsis.ps1`、`native-nsis-acceptance.yml`，先核对开发包和三个 EXE，再做 v0.4/v8 和 v0.5/v14 升级 | E4/E5，包、安装、注册表/WebView2 和临时数据 | 干净安装、升级保留资料、回退可复验，CLI/MCP 组件与候选包一致 |
| [ ] I-07 正式 SHA 和最终证据 | `ci.yml`、`AGENTS.md`、G00 阶段规则 Q09-A。回读 refs、workflow run、候选产物摘要和用户确认 | E5，获批推送和 Windows CI；托管专属另批 | SHA、运行对象、日志和必要证据同源，经用户最终验收，G00/I1 的独立状态保持原记录 |
