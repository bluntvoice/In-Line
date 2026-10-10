# Phase 0：只读基线审查与设计验收报告

日期：2026-10-09（Asia/Shanghai）。状态：**P0已获用户验收；C1–C4均选A。以下审查证据保持原始基线。**

用户验收原话（2026-10-09）：“P0验收通过，C1–C4均选A，可以提交并推送，进入P1。”已授权本阶段Commit、Push、固定`mcp-phase-00-accepted` Tag后进入P1；未授权正式Release。C1–C4下文的建议A均成为冻结决定，B/C仅为历史备选。提交SHA由该固定Tag解析，避免提交内自引用SHA。

## 1. 最新 Git 基线

| 项目 | 实际核验结果 |
| --- | --- |
| 仓库 | `C:/Users/KB/Documents/inline/In-Line` |
| origin | `https://github.com/bluntvoice/In-Line.git` |
| 当前分支 | `feat/scheduled-items`，保持此前用户选定的0.5开发分支 |
| HEAD / origin/feat/scheduled-items | `41383aad406f376ef83bd29284463ee953a0a768` |
| origin/main | `1baac96252af2665274f2a955b3adacfa80519fd` |
| 当前分支与跟踪远端 | ahead/behind = 0/0；无需pull |
| main与当前分支 | main独有0、开发分支独有18；main是当前基线祖先 |
| 初始工作区 | 干净，无未提交/冲突；本轮结束新增docs/mcp草案及证据 |
| 软件版本/schema | 六个版本来源中5个项目文件均0.5.0（本轮不创建Tag）；业务schema v9 |

fetch首次 `git fetch origin --prune` 失败：`schannel: failed to receive handshake, SSL/TLS connection failed`。随后 `git -c http.sslBackend=openssl fetch origin --prune` 成功（exit0），只对该命令使用OpenSSL；没有改Git持久配置、硬重置、强推、切换分支、合并或覆盖用户工作。

输入PRD的main/0.4/schema8是旧快照，与最新开发分支不同；本轮以已核实的0.5/schema9设计，不在main旧版本开工。未自行Commit/Push/Tag/Release或运行打包Action。

## 2. 当前 MCP 与安全/架构缺口

- 实际工具数 **2**：`get_report_summary`、`list_report_items`，rmcp 3.1.2、本机stdio；sidecar用只读SQLite连接查询，不改业务数据。
- 返回真实区间统计和有效办理事项，默认排除回收站/作废事件；不返回联系人/正文/内部备注专用字段。**办理说明note仍是自由文本，不具有通用敏感脱敏保证**。
- 分页默认100、最大500，日期最多371天，offset分页不固定跨请求快照；无客户端凭证、权限组、限域、撤销/轮换/暂停、任务/增量/AI业务审计。
- 主程序已有队列、状态、加急、未来、子任务、办理记录、归档/回收站、备份/统计领域逻辑。应复用，不由sidecar另写SQL实现。
- DB Mutex只限单进程，公共领域操作自行提交，不能简单循环满足20事项混合原子事务。现有完整TaskInput也缺字段版本冲突控制。
- 主程序启动和single-instance回调均show_main，Database::open发生在单实例插件处理前，不满足静默启动及开库前协调要求。
- 安全/任务/诊断存储尚不存在；新凭证不能放进可随业务备份合并的settings。MCP管理目前只是接入说明窗口。
- 已有真实stdio/SQLite/领域测试和NSIS sidecar链路。CI Push只匹配main/codex/**，当前feat分支Push不自动触发该CI；建议P1补匹配。release.yml仅v*，mcp-phase-XX-accepted目前不会发布。

完整文件/函数现状证据S1–S9见 [architecture.md](../architecture.md)。缺口分类不是缺陷修复结论；新增PRD能力尚未实现。

## 3. 交付文档与144项覆盖

- [权威输入副本](../inputs/03-In-Line-MCP-Codex-总执行提示词.md)：三份逐字原件归档，源哈希/大小在source-manifest.json。
- [144项追踪矩阵](../requirements-traceability.md) / [完整CSV](../requirements-traceability.csv)：001–144唯一，每条有原功能、原阶段、原验收口径、差距类别/现有源码、拟实现函数、计划测试、证据、状态、用户确认、Commit/Tag字段。
- [架构草案](../architecture.md)：领域权威、可信本地IPC、身份/范围、安全/业务/任务/缓存/诊断存储、事务/幂等/撤销、迁移/备份/代际、恢复/资源边界。
- [API草案](../api-draft.md)：按领域分组，统一结构化结果/错误、写入判别联合、分页/游标、授权和禁止动作。
- [P0–P9阶段计划](../phase-plan.md)及 [隔离测试设计](../test-plan.md)：原阶段分配不变，前置能力/跨阶段出口与负面测试明确。
- [执行检查点](../CURRENT_TASK.md)：压缩后先回读三份输入，再对照矩阵/报告/最新用户验收消息与Git；不只依赖对话摘要。

P0审查覆盖全部144条。P1安全/协调 → P2只读/报表 → P3单项写入/审计 → P4复杂批次/队列/子任务 → P5任务/增量 → P6健康/溯源/缓存 → P7 UI/诊断/导出 → P8集成/兼容 → P9发布准备。各条原主阶段及跨阶段范围完整保留，所有功能新增验收均未标记通过。

## 4. 关键技术建议

1. **sidecar协议适配＋主程序本地Named Pipe协调**：所有MCP读写均经认证/限域主服务，GUI与MCP复用共享域；显式ACL/拒绝远程/端点与挑战证明，开库前单实例，后台启动不show_main。Named Pipe须显式权限，不能沿用默认宽泛读权限。[Microsoft说明](https://learn.microsoft.com/en-us/windows/win32/ipc/named-pipe-security-and-access-rights)
2. **凭证、权限、暂停独立存储**：当前用户DPAPI/ACL＋本机绑定；环境凭证，不出现在命令行/业务备份/导出。DPAPI有漫游例外，本机迁移规则需单独检查机器绑定。[Microsoft说明](https://learn.microsoft.com/en-us/windows/win32/api/dpapi/nf-dpapi-cryptprotectdata)
3. **一套事务领域操作**：单次提交业务、长期AI审计、幂等结果及变更序列；字段冲突和实际受影响事项限制；崩溃仅核验账本，不重放。Dry Run用隔离副本同规则，生产零写。
4. **可靠代际/快照/检查点**：跨页固定内容基准；授权变化即失效；所有GUI/导入/自动写路径维护版本；历史缺口如实。任务分别执行30分钟暂停/24小时恢复/最多一次完整重查/30天摘要与7天临时结果。
5. **分离审计、诊断和缓存**：业务审计同库长期；技术日志脱敏轮转；磁盘缓存只ACL无额外加密、最小化内容；清理不删证据/检查点。故障只状态筛选、手动预览确认导出，无哈希清单/外部AI调用。

上述是待验收建议，不是已验证实现。IPC/认证、混合事务、恢复幂等与快照资源是最高风险，阶段必须有负面证据及隔离安装升级。正式版本号/旧接口最终移除/Release不在本轮决定。

## 5. 产品差异及需要核验的选项

不能根据新提示词默默改写已有PRD决定。用户2026-10-09已选择以下C1-C4全部A；B/C仅保留为历史备选。C1-C3在P3/P4实现，C4在P1实施。

### C1：MCP归档与已取消手动归档UI

来源：subtasks.md第7节、future-items.md第6行的2026-10-05决定：旧归档规则只用于兼容，不提供新手动操作入口。MCP-004/025/039则要求明确意图归档。

- **A（建议）**：将MCP明确指令归档视为本次新增的受控业务入口，复用现有归档域规则；保留GUI取消按钮的决定，不新增GUI手动归档。保留archive与completed/cancelled语义差别，不把归档强行转换为完成产生虚假办理。
- B：继续全面不提供新手动归档，MCP只读取旧归档历史及处理完成/取消；这会缩小已确认144项范围，必须由用户明确修订相关需求，不能自动采用。

### C2：MCP批量改期与原应用内二次确认

来源：future-items.md第5/60节禁止首版批量改加入日期，最后用户裁决要求改期应用内二次确认与后端确认标记。MCP-023/039/040允许未来事项、混合批次及预演；授权后不逐项弹窗不代表可伪造用户确认。

- **A（建议）**：本次允许明确指令的MCP批量改期；整批先生成影响计划（作废编号/移出队列/重新激活/截止清空），用户在软件内**一次确认整批**后共享域事务执行。普通单项GUI原确认保持，不为MCP新增每项弹窗，也不抢焦点；待确认操作以pending状态返回。
- B：允许MCP批量改期，并在AI对话中的明确指令/确认后执行，不要求应用内确认；需要用户明确调整原“应用内二次确认”的产品决定，设计必须如实承认服务端无法证明AI自报确认等于人类实际确认。
- C：保持原禁止批量改期；MCP只能单项申请并在软件内确认。须明确收窄MCP混合批次范围并同步需求，不将未做项标记通过。

### C3：真实办理记录追加/纠错

来源：README/CHANGELOG v0.2.1取消UI编辑/手工补录，统一作废；后端record_work_event仍用于测试/既有规则。MCP-004/019/026明确允许真实追加及纠错，当前只有作废入口，没有精确更正关联模型。

- **A（建议）**：允许MCP按明确真实办理事实追加及纠错；更正采用原事件作废＋关联的新事件＋审计，保留首次有效事件统计影响确认。GUI旧入口保持，不允许AI虚构办理或覆盖原迹。
- B：本轮仅开放已有作废纠错，不开放追加/更正；须用户明确修订MCP-004/019/026。

### C4：多部门事项的客户端限域

来源：现有事项可属于多个部门，MCP-060要求部门/类型服务端限域，但未定义“授权A部门的客户端可否看到A+B事项”。这会影响隐私与可用性，属于产品范围选择。

- **A（建议）**：事项全部所属部门均在客户端授权范围才允许读取/写入；完整事项不会暴露未授权部门；范围未设限制则全库。缺部门旧数据默认不匹配已设置的部门范围。
- B：任一部门交集即可访问整项，但隐藏未授权部门名称/关联；业务正文可能仍涉及其他部门，需明确接受跨部门内容可见范围及定义写入权限。

附：MCP-034仅开放现有可验证的低风险偏好（字体、缩放、已有编号色），**不恢复用户已暂缓的三套编号组合或整体主题开发**。迁移进行中保护安全备份、禁止并发清理；成功后遵守现有用户主动确认的清理规则，协调策略在P1验收。旧接口名保留为建议，P8如直接移除须另外明确迁移批准。

## 6. 本轮实际测试证据

环境：PowerShell 7.6.4、Node v24.19.0、npm 11.17.0、cargo/rustc 1.97.1。基线代码未修改，现有测试使用已有隔离/合成夹具。

| 证据 | 命令 | 实际结果 |
| --- | --- | --- |
| B1 | `npm.cmd test` | exit0；17文件、105测试通过；[输出](evidence/P0-frontend.txt) |
| B2 | `cargo test --locked --manifest-path src-tauri/Cargo.toml` | exit0；59库+2MCP+1真实stdio=62通过；[输出](evidence/P0-rust.txt) |
| B3 | `npm.cmd run build` | exit0；TypeScript/Vite生产构建成功；[输出](evidence/P0-build.txt) |
| B4 | `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` | exit0；[输出](evidence/P0-format.txt) |
| D1 | `pwsh -NoProfile -File docs/mcp/build-traceability.ps1` | 144唯一ID、阶段/验收文本保留、每条计划映射，新增功能无通过状态 |
| D2 | 输入副本/文档链接/改动范围校验 | 输入副本与原件相同；链接存在；仅docs/mcp新增，HEAD未改变；[校验记录](evidence/P0-document-checks.txt) |

本轮基线测试无失败/重跑；fetch首次失败和成功重试在报告中保留。文档校验工具三次失败及修复分别为：输出自引用检查顺序、Git中文路径引用转义、package-lock空属性名需AsHashtable解析。保留[第一次](evidence/P0-document-checks-first-failure.txt)、[第二次](evidence/P0-document-checks-second-failure.txt)、[第三次](evidence/P0-document-checks-third-failure.txt)证据，修复后重跑全部文档校验；未削弱断言、修改产品/锁文件或持久Git配置。构建仅生成被忽略dist，不是NSIS安装包。

**未执行**：所有新增认证/权限/IPC/并发事务/任务/增量/缓存/故障/导出测试；NSIS、安装/升级、真实GUI/焦点、多账户/第二设备验收；远端CI/Actions运行。原因是本轮仅P0只读审查与文档草案，尚无新增实现；不能用B1–B4替代这些门禁。

## 7. 文件级范围与下一步

新增仅 `docs/mcp/`：三输入副本/清单、README、检查点、架构/API/阶段/测试草案、144矩阵/CSV/初始化映射与文档生成脚本、本报告及本轮基线证据。未修改src/src-tauri业务实现、数据库、项目版本、既有公开发布文案或workflow。公开README/CHANGELOG/RELEASE_NOTES已检查，本轮无功能变化，不将草案宣传为已可用；后续功能阶段按MCP-054同步。

请核验P0总体方案及C1–C4选项。收到明确阶段验收及“可以提交并推送，进入下一阶段”前，保持本轮草案未提交并停止在P0。用户可只批准设计/选择选项而不授权Git；必须区分。若验收通过，先复核实际文档与基线再Commit/Push/固定阶段Tag，之后才开始P1；正式发布另行授权。

提交前补充：输入目录使用 -text 属性保持权威原件字节不受 Git 换行归一化影响；原件 Markdown 硬换行空格按原文保留。
