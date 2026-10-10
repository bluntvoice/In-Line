# Phase 1：安全基础与服务协调验收报告

日期：2026-10-09（基础验收），更新2026-10-10（Asia/Shanghai）。状态：**P1仍待用户验收；最新用户授权包含全部当前改动开发提交/推送与Actions测试安装包，不进入P2、不建立P1验收Tag。此前安装证据仅代表旧测试包，新增客户端流程及schema v10本轮未安装。**

最新补充：单事项编号配色和未来子任务预占状态修复与当前MCP改动同批开发提交；前端112、Rust91测试及隔离浏览器交互通过，当前Codex自动接入仍关闭。验证及首失败见[本轮验收记录](TASK-COLOR-AND-SUBTASK.md)。

## 1. 基线与阶段授权

用户原话：“P0验收通过，C1–C4均选A，可以提交并推送，进入P1。”

- P0提交 `f2a97b34410f850f3da8c19d4ec6e21c7d63048f` 已推送 `origin/feat/scheduled-items`；annotated固定Tag `mcp-phase-00-accepted` 远端解析同一提交。
- P1在此提交上实施，工作区改动未提交。`origin/main`为`1baac96252af2665274f2a955b3adacfa80519fd`；保留0.5开发分支。
- 软件版本五处仍为0.5.0，业务schema仍v9；新增独立授权存储版本1、MCP API v1。没有把未验收开发代码发布为0.5正式版。
- C1–C3的归档/批量改期/真实办理纠错将在P3/P4实施；C4全部部门且类型交集已用于当前报表。暂停的配色主题需求没有恢复。

## 2. 实现范围及文件摘要

| 文件 | 改动与原因 |
| --- | --- |
| `src-tauri/src/mcp/basis.rs` | 准备业务版本/受控查询快照共享类型；仅前置结构，不生成虚假查询基准，P2再实施真实快照。 |
| `src-tauri/src/mcp/contract.rs` | 独立环境凭据、3工具常量、参数白名单、统一结果壳及错误。拒绝超页长及错误日期，不返回原始路径/SQL错误。 |
| `src-tauri/src/mcp/security.rs` | 三组默认/全局与客户端交集、长期授权、轮换/撤销/暂停、部门/类型范围、原子持久化、授权修订、失败及高频限流。 |
| `src-tauri/src/mcp/platform.rs` | 当前用户DPAPI、受保护用户ACL、机器/SID绑定、路径重解析拒绝；登录会话管道描述符、对端进程路径核验、开库前全局按用户SID的mutex/就绪gate。 |
| `src-tauri/src/mcp/ipc.rs` | 当前会话ACL且拒绝远程的Named Pipe；首实例防抢占、双方nonce挑战证明、限制帧/连接/时间、主程序按需静默启动；每次主服务鉴权及释放结果前修订检查。 |
| `src-tauri/src/mcp/scope.rs` | 与现有部门解析一致的SQLite确定性过滤，必须全部部门匹配且事项类型匹配；先过滤再算计数、分母、趋势和明细。 |
| `src-tauri/src/mcp/service.rs` | 只分发真实3只读工具，共享主程序Database方法；能力发现准确披露不支持写入/全历史/稳定快照；自由文本按完整读取权限投影。 |
| `src-tauri/src/bin/in-line-mcp.rs` | sidecar只作stdio适配，不自行打开业务库；全部工具认证，结构化成功/错误，业务错误同时标记MCP isError。 |
| `src-tauri/src/database.rs` | 复用统计/报表算法并增加scope；GUI默认不限域；迁移前/备份/事务内完整性/FK检查、保护迁移备份ACL、拒绝未来schema。 |
| `src-tauri/src/lib.rs` | 单实例及gate后才开库，非拥有者拒绝开库；后台启动/第二后台实例不显示窗口；仅main窗口可管理安全配置；既有GUI领域和data-changed保持。 |
| `src/components/McpSecuritySetting.tsx`、`src/api.ts`、`SettingsPanel.tsx`、`styles.css` | 在现有设置内紧凑折叠管理授权、范围、暂停、轮换、撤销、一次显示/复制配置、实际连接测试；不擅改第三方客户端配置。 |
| `src-tauri/tests/mcp_{stdio,security,host}.rs`、`src/lib/mcp-config.test.ts` | 真实stdio、共享数据库范围/交错调用、真实主程序静默/第二实例、环境配置测试；使用合成数据和独立目录。 |
| `Cargo.toml`、`Cargo.lock`、`.github/workflows/ci.yml` | Windows安全API/IPC及HMAC依赖；当前开发分支加入CI push匹配。发布workflow无变更，远端P1 CI尚未执行。 |
| `docs/mcp/*`、`docs/prd/mcp.md`、README、CHANGELOG | 保存源需求、实际实现/设计区别、完整144行矩阵、C1–C4冻结、认证破坏变化及阶段门禁。当前正式Release Notes已检查，无新正式发布内容。 |

全部144项原始名称、主阶段和验收条件保留在[完整CSV](../requirements-traceability.csv)。下表21条为P1主需求。跨阶段验收点没有虚标全部完成。

## 3. P1逐项覆盖

| ID | 实现与断言 | 当前状态 |
| --- | --- | --- |
| 001 | `Store::authorize/execute`每次鉴权；连续交错读无需重复授权，restore_database拒绝 | P1待验收；写入阶段另验 |
| 002 | default/intersection；默认常规开、完整/写关，客户端独立凭据 | 待验收 |
| 003 | `launch_host/run`；真实主程序按需启动零可见窗口，无前台焦点 | P1待验收；写调用另验 |
| 007 | grant/revoke；两客户端数据及伪造ID隔离 | 待验收 |
| 008 | 重新打开独立安全库凭据仍有效，无自动到期 | 待验收 |
| 009 | 机器/SID校验、损坏fail closed；业务恢复不复活授权 | 部分待验收；跨机与同机凭据升级保留仍未验 |
| 042 | actual tools/list、capabilities仅3只读工具，写工具空；版本/权限/范围/破坏变化真实 | 待验收；可信构建Commit未知返回null |
| 044 | 软件内授权/复制配置/连接测试、配置环境变量单测、stdio真实调用 | 部分待验收；UI点击和第三方客户端实机待验 |
| 045 | unauthenticated/revoked/paused/forbidden/incompatible等区别，错误无业务data | 待验收 |
| 046 | 开库前gate、真实第二后台实例退出；权限与数据库锁串行；同机多客户端 | P1待验收；写事务和跨登录会话实机另验 |
| 047 | 暂停持久化且能力发现也拒绝；业务恢复不能解除 | 待验收 |
| 053 | v8→v9合成升级、原数据保留、迁移前备份及ACL、失败事务回滚 | 部分待验收；NSIS/schema9旧库保留已测；旧schema实机迁移待验 |
| 056 | 原报告工具复用UI算法、日期/有效事件/统计回归 | 待验收；认证/结果壳/100条/说明投影为有意变化 |
| 057 | 匿名实际stdio子进程3工具均拒绝；不匿名启动主库 | 待验收 |
| 058 | 安全域独立于业务备份/导出，落盘失败不声称授权改变 | P1部分待验收；P3审计和P7诊断域未实现 |
| 059 | 轮换旧token失效、撤销拒绝、恒定时间摘要比较、限流；不记录token | 待验收 |
| 060 | 多部门ALL与类型AND、缺部门/空范围/字面引号、分母趋势明细过滤、GUI无scope污染 | P1待验收；历史/写入/导出限域逐阶段补齐 |
| 061 | 本机stdio、内部受保护Named Pipe、首实例/伪造端点/帧长拒绝；无TCP/HTTP服务 | 待验收；跨账户/远程攻击实机未执行 |
| 069 | 当前3只读工具领域分组，每次鉴权；未来写工具未注册 | P1待验收 |
| 070 | apiVersion/requestId/status/data/error；JSON-RPC参数与业务错误分开；isError匹配 | P1待验收；执行/快照/诊断元数据后续补齐 |
| 102 | 主服务有效60次/分钟、无效5次/分钟；sidecar预检拒绝亦限流，60–300秒分级冷却；无效身份不能锁住有效客户端 | P1待验收；P5长任务暂停另验 |

## 4. 自动化证据与命令

证据位于本目录`evidence/`，包含首失败、复现及修复后输出。全部数据为合成测试数据，没有读取生产事项。

| 命令/证据 | 结果 |
| --- | --- |
| `npm.cmd test`；[前端](evidence/P1-frontend-first.txt) | exit0，18文件106测试通过 |
| `npm.cmd run build`；[构建](evidence/P1-build-first.txt) | exit0，TypeScript及Vite生产构建通过；NSIS构建亦执行相同前端命令 |
| `cargo check --locked --all-targets --manifest-path src-tauri/Cargo.toml`；[首次check](evidence/P1-check-first.txt) | exit0；首次unused import警告随后移除 |
| `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`；[格式](evidence/P1-format-final.txt) | exit0，最终格式检查通过 |
| `cargo test --locked --manifest-path src-tauri/Cargo.toml -j 1`；[最终Rust](evidence/P1-rust-delivery.txt) | exit0，69库+2bin+1真实主程序+2范围+1stdio=75；0失败、0忽略。包含sidecar第6次匿名请求限流、授权库超限拒绝保留原状态断言 |
| `scripts/check-github-actions-runtime.ps1`；[策略](evidence/P1-actions-policy.txt) | exit0，6个Actions符合策略 |
| `npm.cmd run dist`；[首次NSIS](evidence/P1-nsis-build.txt)、[冻结后NSIS](evidence/P1-nsis-delivery.txt) | 首次及冻结源码最终构建均exit0；标准release配置，仅process-local VS/SDK环境及CARGO_BUILD_JOBS=1，无profile降级 |
| `node docs/mcp/verify-release-stdio.mjs`；[release stdio](evidence/P1-release-stdio.txt) | 实际release sidecar初始化、3工具匿名拒绝、第6次匿名调用限流；stdout纯JSON、isError与结构化错误一致 |
| 文档/版本/源需求独立校验；[文档](evidence/P1-document-checks.txt) | 3原件/副本哈希、144条原决定及13字段、21项P1、五处版本/P0 Tag/本地链接已通过；不重跑会覆盖人工进度的P0初始化脚本 |

### 首失败、原因及修复

1. Rust WindowsAPI导入（SE_GROUP_LOGON_ID/BOOL）编译错误：按实际windows-sys位置和i32签名修复，[second](evidence/P1-rust-second.txt)、[host](evidence/P1-rust-host.txt)。不是削弱断言。
2. 默认并行编译OS1455页面文件不足，[third](evidence/P1-rust-third.txt)；改用`-j 1`，[serialized](evidence/P1-rust-serialized.txt)71项通过。没有改系统页面文件或删除用户缓存。
3. 真实主程序启动失败，[final首次](evidence/P1-rust-final.txt)；隔离主程序[stderr复现](evidence/P1-host-repro-stderr.txt)明确“no reactor running”。Tauri setup创建第一Named Pipe前进入Tauri运行时，原断言不变，[定向修复后](evidence/P1-host-after-fix.txt)通过；随后全量[verified](evidence/P1-rust-verified.txt)74项通过。
4. 增加伪造端点负面测试后，[frozen首次](evidence/P1-rust-frozen.txt)出现错误分类断言不符：缺失相邻主程序在canonicalize路径中报security_unavailable。改为host_unavailable，保证端点/主程序问题有准确用户指引；保留“错误码+未传输任何字节+首实例不可替代”全部断言，[修复后](evidence/P1-rust-frozen-fixed.txt)75项通过。该夹具覆盖缺失期望主程序及伪造端点拒绝，跨账户/远程不冒充已测。

### 关键负面场景

- 真实匿名stdio子进程：两旧工具和capabilities全部拒绝；错误data为null。
- 两独立客户端部门A/T、B/T；跨部门A+B及其他类型U不得出现在事项、计数、统计分母、趋势。交错8次/客户端不串范围。
- 常规读不泄露办理自由文本；完整读须全局和客户端同时开；联系人/正文/内部备注专用字段仍不返回。
- 同一连接中暂停、撤销后立即拒绝；轮换旧token无效；授权缩小后先前结果在释放前被revision校验拒绝。
- 损坏/模拟机器标识不同拒绝读取，不自动重置；业务备份恢复不会解除暂停或撤销。
- 文件落盘失败不更新内存状态、不谎报撤销成功；ACL变宽拒绝；包长先检查再分配；nonce/方向绑定；首管道实例不能被替代。
- 开库前只有mutex拥有者；真实第二后台实例不打开库、不显示UI；失败/饱和/超时不无限启动或无限增加域线程。

## 5. 资源与性能边界

P1限制：授权密文存储1MiB（写入前检查；超限不改变内存/磁盘，读取先检查文件长度）、请求64KiB、响应8MiB、连接操作20秒、15个在途服务许可（17管道实例留接入空间）、最多100客户端及每范围100条、最多100条/页、报表371天。sidecar初步鉴权失败（包括缺凭据）也在本进程限流，主程序再次执行整体限流；sidecar重启重置其本地冷却，不能绕过主服务对有效调用的计数。有效与无效身份限流池有256条上限，真实授权客户端不会被无效ID挤死。被超时取消的blocking域任务仍持有许可，避免反复超时突破资源限制。

真实主程序最终75项轮次测试运行2.40秒，范围交错测试0.28秒；这是小夹具运行时间，**不是生产性能承诺**。1千/1万/10万合成事项、长期运行内存/GUI交互预算尚未执行，后续读查询/快照阶段补基准。授权锁与Database锁串行保证范围和授权一致性，较慢统计会推迟用户授权变更返回；UI只有持久化完成才显示生效。

## 6. 安装包和实机待验

本地生成`src-tauri/target/release/bundle/nsis/In Line_0.5.0_x64-setup.exe`；这是当前未验收源码的本地测试产物，不上传、不新建Release、不进入更新通道。冻结后安装包及主程序/sidecar/版本/打包输入回读见[回读证据](evidence/P1-package-readback.txt)。包3,430,486字节，SHA256 `F87530CD5B5FE09379DB19C31B31EE9E482A8E84334B654E67D48ECA4326B963`，主程序PE版本0.5.0。

现有NSIS脚本包含两个同名sidecar输入（prepare阶段副本与Tauri阶段target/release产物），摘要不同；保留原打包顺序，分别实际执行相同stdio认证/限流门禁均通过。初次包检查把字节相同误设为前提，失败日志[P1-package-readback-first](evidence/P1-package-readback-first.txt)保留；纠正为逐份行为门禁并记录两份摘要，不移除任何认证/协议断言。本次真实安装已核对采用第二个target/release产物，见下方安装回读。Tauri打包后会还原主程序bundle标识，回读的主程序摘要不是解包后安装文件摘要。

### 本机测试窗口（2026-10-09 23:29–23:32）

用户明确授权：“安排本机测试窗口，允许中断当前 MCP 后验收。”已执行本机方案B，不再重复请求中断许可；此授权不包含P1提交、推送、Tag、P2或Release。

- 只终止原安装目录的In-Line主程序及MCP进程，未终止Codex或其他客户端。离线复制旧安装目录及完整业务目录（含WAL/SHM），12个文件逐一摘要校验一致。回退副本只在本机受当前用户/SYSTEM ACL保护的目录，不进入仓库；位置见[测试窗口记录](evidence/P1-test-window-location.txt)。
- 执行本地冻结包：`Start-Process -FilePath <installer> -ArgumentList '/S','/D=D:\Application\In Line' -WindowStyle Hidden -PassThru -Wait`，NSIS退出码0。没有Actions、Tag或Release。
- 已安装主程序0.5.0、7,485,440字节，SHA256 `4D15983FF06F96E9978A5AA9E78BA98B394C5BB16688F54C14015F55A11FF4C2`；与打包后恢复标识的源码目录exe摘要不同，未误称两者相同。
- 已安装sidecar0.5.0、1,171,456字节，SHA256 `7A0E582F4279C94EBF8A5147DA9916AC2E9877DF1E5B0AFFDE90A5BBA3AB7373`，实际采用第二个NSIS输入`target/release/in-line-mcp.exe`。
- 安装前、安装后及首次启动后只读检查：schema9，268个事项，完整性/FK通过，全部11张业务表数量和逻辑摘要一致。没有到期待启用事项。现有库已为schema9，所以本次不冒称执行了旧schema迁移；旧schema迁移仍由隔离自动测试覆盖。
- 对已安装sidecar实际执行`node.exe docs/mcp/verify-release-stdio.mjs "D:\Application\In Line\in-line-mcp.exe"`：3工具匿名拒绝、第6次限流、`isError=true`、data=null、stdout纯JSON协议全部通过。见[安装回读](evidence/P1-installed-readback.txt)。
- 已启动已安装主程序，进程路径正确，独立授权密文存储已创建；启动后读回见[启动回读](evidence/P1-installed-startup.txt)。原来不存在独立授权存储，尚不能证明已有凭据同机升级保留。
- computer-use初始化两次均报`failed to write kernel assets: 系统找不到指定的路径。 (os error 3)`，没有执行GUI点击或授权变更。窗口可见性、布局及实际授权由用户验收，不将进程存在冒充GUI通过。
- 首次备份目录ACL准备时icacls未使用数字SID所需的`*`前缀，失败发生在退出程序及复制数据之前；修正后ACL创建成功，再中断程序。该失败保留于[测试窗口首次失败](evidence/P1-test-window-first-failures.txt)。

用户可按[本机验收清单](LOCAL-P1-TEST-WINDOW.md)验收。跨Windows用户/跨电脑凭据复制、跨登录会话、安装中断/更新失败、目标AI客户端实际配置和GUI点击均仍未执行。真实升级和旧数据保留的结果不代替这些未测项。

## 7. 下一步门禁

本机安装/升级、旧库保留及已安装匿名stdio门禁已完成；继续GUI和独立授权流程验收，再明确P1验收结论。未收到“P1验收通过，可以提交并推送，进入P2”前，保留当前改动与证据，不能建立P1 Tag或实施P2。正式发布继续需要单独授权。

P2全历史、稳定分页/数据基准仍未实现；P3/P4写入/审计/幂等/整批原子事务仍未实现；P5任务/恢复、P6缓存/健康、P7诊断都未提前宣称可用。

### P0计划前置结构

P1在`mcp/basis.rs`准备了DataVersion与QuerySnapshot共享类型（数据库UUID/代际/提交序、schema/查询/统计版本，以及客户端/授权修订/过滤/投影/排序键/期限/完整性/缺口绑定）。这是前置结构，未生成虚假业务版本、未落盘快照或开放稳定分页；P2须创建并验证真实查询基准，P3/P4各写路径维护可靠版本，阶段归属不改变。

### 独立测试设备验收步骤

1. 旧正式版中建立纯合成事项和办理记录、导出业务备份；记录事项身份/状态/数量，退出旧主程序后升级本地包。核对这些数据及升级前备份保留。
2. 设置内授权两个客户端，分别A部门/T类型及B部门/T类型；合成多部门A+B、其他类型U不得出现在越权报表、计数或办理文字中。
3. 复制配置到目标AI客户端，调用capabilities和两报表；旧匿名配置明确拒绝。开启全局与客户端完整读取后才显示合成办理说明。
4. 保持客户端会话，依次暂停/恢复、轮换、撤销，核对立即生效；恢复早期业务备份不能复活暂停前/撤销前凭据。
5. 主动退出主程序后，用有效stdio调用按需启动；核对无主窗/浮窗/通知和焦点变化；用户主动打开软件仍能显示主界面。
6. 同机保留授权后再次安装测试包，核对授权不丢；在另一测试用户或电脑复制授权文件只能拒绝，不能自动创建有效授权。未验证的步骤继续保留“未执行”。

### 文档校验首次失败

独立P1检查脚本首次读取PSObject.Properties.Count方式不正确，第二次package-lock空键需ConvertFrom-Json -AsHashtable，第三次零输出fmt没有生成证据文件；报告指向未开始的下一构建也被链接门禁拒绝。PowerShell终止错误未生成计划的单独Tee日志，工具返回原文摘录集中保存在[失败转录](evidence/P1-document-failures.txt)，明确标识为转录。修复为集合计数、Hashtable解析、显式记录fmt成功及完成对应构建。未改变144条原验收条件或降低字段/链接门禁。

## 2026-10-10 接入体验调整与Codex门禁（软件内实现前的历史记录）

用户已选择严格无需额外操作：选客户端和权限后只粘贴一次提示词，由AI完成自动导入并在当前会话生效。当前Codex26.1002.7124.0/CLI0.162.0-alpha.2真实配置探针未通过自动重载门禁；原配置已逐字恢复，无凭据或业务数据读取/变更。尚未实现导入包/导入器/UI，支持列表暂为空。不得要求手填或一次重连降级。具体证据、首次准备失败和范围限制见[实机报告](CODEX-ONBOARDING-VALIDATION.md)。MCP-044继续部分待验收；P1阶段门禁不变。

## 2026-10-10 用户选择A后的软件内实现与隔离验证

最新授权允许先完成客户端选择、权限授权、安全提示词和本地导入器。已实现Codex预设、10分钟私有加密一次性包、TOML受控合并和回退、真实stdio身份/权限/范围验证；不再要求前端填写或复制长期凭据。当前会话验证与配置验证分开，UI、Tauri及release CLI自动接入入口均关闭。

前端109、Rust89测试及前端构建/fmt通过，本地NSIS构建及两个sidecar输入协议/关闭门禁检查通过。新包未安装；原安装两exe哈希一致，当前Codex配置不变。Codex当前会话自动重载、生产授权、GUI、新包安装升级仍未验，不沿用旧安装结果冒称新实现通过。完整命令、负面用例、首失败和包哈希见[软件内验收报告](CLIENT-ONBOARDING-LOCAL.md)。MCP-044部分待验收，P1未通过用户验收，未提交/推送/Tag/Release，未进入P2。
