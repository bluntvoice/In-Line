# CLI Phase 1：统一应用服务入口

日期：2026-10-16。状态：**用户已确认CLI Phase1通过并授权提交推送；实机验证留Phase6，暂不进入Phase2**。

## 授权与边界

用户已确认Phase0的D1–D4全部选A，Phase0文档提交`d4a56d3010ab94143ea2647688022ac61bc51d58`已推送；随后明确“继续”，本轮进入CLI Phase1。开始分支`feat/scheduled-items`、工作区干净，fetch后与同名远端0/0。

采用[已确认架构](PHASE-00-ARCHITECTURE.md)的增量方案。本阶段只迁移统计摘要和直接状态修改到应用服务入口，保留既有领域规则；不实现CLI binary、命令解析、授权新类型或Phase2功能。不改变MCP的16工具契约、数据库schema、原审批或发布状态。不使用生产业务数据做测试，不替换已安装程序。

## 预期调用链

- GUI统计命令、MCP快照报表 → `application::statistics` → 原`Database::statistics_scoped`。GUI用本地全部范围，外部范围仍由原认证/查询层决定，统计SQL不复制。
- GUI状态命令/兼容Database方法 → 应用服务持有原事务；MCP状态动作/撤销 → 复用已有外部事务，再进入同一应用状态动作 → 原domain状态规则。
- 应用服务事务内入口不自行提交；外部入口原权限、版本、幂等、审计、提交回读、结果释放前授权修订复验继续由现有链路负责。
- 本地GUI不因外部客户端暂停而被禁止；外部调用不能选择可信GUI入口。现有GUI变更通知和主程序IPC通知代码保留。

## 验证安排

1. 先在重构前运行新增对照测试并保留首次结果；同一用例用于迁移后回归。
2. 核对两个隔离库的GUI/MCP状态、计划/队列号码、轮次、办理事件及幂等重复；独立生成的操作时间不要求字节相同。
3. 统计与现有GUI结果一致、保留限域、排除未来未办理事项、GUI变更后不返回旧缓存。
4. 父任务尚有未完成子任务时，直接完成失败必须整库内容/SQLite序列回滚，包括提前入队及原号码变更；不只断言最终status。
5. 拒绝只读/跨范围/旧版本/暂停/撤销；确认暂停不影响GUI本地操作。
6. 全量Rust/前端、fmt、TypeScript/Vite、实际16工具完整schema与注解/匿名拒绝，按仓库规范构建冻结源码NSIS并核对包输入。未执行的实机项目单列，不与自动化混为通过。

最终结果、首次失败和交付包摘要如下。2026-10-16用户明确选择“A：确认CLI Phase1通过并提交推送；实机验证留Phase6，暂不进入Phase2（推荐）”。本阶段验收结论与授权见`evidence/PHASE-01-user-acceptance.json`；MCP P8当前会话接入状态独立保留。

## 首次验证记录

重构前首轮4项对照测试3通过、1失败，日志`evidence/PHASE-01-baseline-first.txt`。失败为新增测试预期错误：`mcp_write::target`先按授权范围筛选目标，范围外返回not_found避免泄露存在性，不能要求它返回forbidden。修正为只读客户端严格forbidden、范围外严格not_found，整库零变化断言保留；未改变原产品行为。另把缓存验证的基准查询放到新增客户端授权之后，避免授权修订变化干扰“GUI写入导致缓存失效”的证明。

修正后迁移前4项全部通过（`evidence/PHASE-01-baseline-final.txt`）；迁移后新增事务外拒绝用例，核对未来事项未分配今日号码或留下任何表变化。前端119项/21文件通过，日志`evidence/PHASE-01-frontend.txt`。

迁移后默认全量构建首次失败于编译`mcp_stdio`集成目标：Windows mmap返回1455“页面文件太小”，不是已运行用例的断言失败；`PHASE-01-rust-first.txt`保留。只读核实当时虚拟提交余量约3.25GiB，原调试rlib约601MiB。改为仓库外独立target、`CARGO_BUILD_JOBS=1`、`CARGO_PROFILE_TEST_DEBUG=0`、`CARGO_PROFILE_DEV_DEBUG=0`及`--test-threads=1`重跑全部用例，不改Cargo/锁文件，不删除或过滤测试，不关闭debug assertions，不改系统页面文件或关闭其他程序。参数和失败关联见`evidence/PHASE-01-test-runtime.json`；最终结果另记，不能把简单重跑包装为首轮通过。

## 当前代码改动

- [application/mod.rs](../../src-tauri/src/application/mod.rs)提供crate-private统计摘要、状态修改和已有事务内状态动作，不注册新的IPC或Tauri命令。
- `lib.rs`两个原GUI命令改调应用服务，参数、返回和GUI事件代码保持原样。
- `database.rs`原统计/状态公开方法保留为兼容门面；仅把事务包装提高到crate可见，domain模块仅状态函数提高到crate可见，其他领域规则不搬家。
- `mcp/query.rs`在已有受限冻结快照上调用统计服务；`mcp_write.rs`状态修改及对应撤销在原外部事务内调用应用动作。权限、意图、版本、回执、审计、事务提交回读和IPC授权修订复验均未迁走或复制。
- 本阶段没有引入通用身份枚举、DI框架、JSON万能dispatcher或新依赖；不同入口的认证和投影差别继续在既有边界处理。后续CLI服务面、命令解析及结构化错误适配仍需各阶段实现，不能宣称三入口全部完成。
- 已检查README/Release Notes：本轮未交付用户可用CLI或新版本，不增加CLI可用声明；CHANGELOG仅增加一条内部服务调整，原MCP/GUI业务PRD决定不修改。

## 最终自动化及构建结果

| 检查 | 结果 | 证据 |
| --- | --- | --- |
| 重构前对照 | 4项通过；首次错误预期已说明并保留 | PHASE-01-baseline-first/final.txt |
| 全量Rust | 独立低调试信息目标182项通过：173库+2sidecar+7集成，零忽略；完整suite无筛选 | PHASE-01-rust-low-memory.txt、PHASE-01-test-runtime.json |
| 前端 | 119项/21文件通过，单worker；未修改前端源码 | PHASE-01-frontend.txt |
| fmt | 通过 | PHASE-01-format.txt |
| 实际debug sidecar | 16工具名字、完整input schema及全部注解一致；匿名16工具拒绝、重复限流、stdout纯JSON通过 | PHASE-01-contract.txt |
| 兼容输入 | 版本、Cargo/锁、Tauri配置、模型、契约类型、认证/IPC、工具基线及CI未改 | PHASE-01-compatibility-inputs.json |
| TypeScript/Vite、NSIS | 成功；release sidecar 4m04、主程序6m22，版本0.5.0 | PHASE-01-nsis-first.txt |
| 包输入 | 两份sidecar分别通过完整16工具契约、匿名权限/限流/纯协议检查；主程序PE及NSIS版本和输入路径核对通过 | PHASE-01-package-readback.txt |
| 源码冻结 | 167份产品源码摘要未变 | PHASE-01-retained-package.json；本地release/CLI-Phase1-frozen-source.json |
| 原有文档与CI政策 | 原三份MCP输入、144决定和版本/链接检查、Actions运行时策略通过；不代表CLI阶段已验收或获提交授权 | PHASE-01-existing-documents.txt、PHASE-01-actions-policy.txt |
| 生产保护 | 已安装两个exe、独立授权库和Codex配置摘要与本轮开始时相同 | PHASE-01-protected-readback.json |

五项新增回归分别验证：摘要与GUI统计一致且保持范围/未来排除/GUI写后缓存失效；未来提前办理两入口状态/号码/轮次/事件一致且同键不重写；父任务直接完成失败整库回滚；只读/限域/版本/暂停/撤销外部保护；事务外调用在任何领域副作用前拒绝。测试以合成SQLite和真实安全/服务代码执行，不是仅比较请求JSON。

当前未执行本轮包的实际安装、原生GUI观感及当前挂载MCP认证。现有真实stdio/IPC和导入回归均使用合成安全域；不能拿它们替代当前Codex会话接入成功。原MCP P8认证待验状态保留。本阶段只完成代表性服务入口，不宣称全部GUI已统一或CLI可用。

## 本地验收包

- 路径：[In-Line-CLI-Phase1-local-20261016.exe](../../release/In-Line-CLI-Phase1-local-20261016.exe)。4,229,305字节。
- SHA256：`C8FFF7A016EEB0205D59C746D600B1ADE645B1762027895F03A087025BC17964`。
- 基线`d4a56d3010ab94143ea2647688022ac61bc51d58`、未提交源码dirty；`exactEmbeddedCommit=null`，不把未来提交SHA当作包内精确SHA。
- 安装包仍只交付现有GUI及MCP sidecar，不含CLI binary；旧包保留。本轮没有安装、Actions分发、Tag或Release。
- 所有日志在本目录`evidence/PHASE-01-*`；软件输入摘要用于构建溯源，不是故障数据导出清单。

## 用户验收及遗留验证

1. 用户确认CLI Phase1通过，明确将新包安装/升级、原生GUI统计和状态操作、当前AI会话授权连接实机验证留CLI Phase6；这些项目仍未实测，不能标为通过。后续安装须按当时授权安排本机测试窗口，保护现有工作和数据。
2. 本阶段只提取两个代表操作，其他GUI和MCP路由仍走既有实现；后续根据已确认分期迁移，不进行无边界重构。结构化CLI错误、独立CLI授权和命令执行未实现。
3. 全量默认调试构建受本机1455资源限制；同源码、同断言在独立低调试信息目标全部通过。未通过修改系统/关闭其他程序或削弱测试掩盖；CI仍保留原全量命令，推送前不虚报本轮远端CI结果。
4. 本阶段源码、自动化及本地包检查已由用户明确验收，并授权提交推送至`feat/scheduled-items`。**本轮停在CLI Phase1，不进入Phase2，不安装、不生成Actions包或创建Tag/Release。** 包记录`PHASE-01-retained-package.json`中的`userAcceptance=false`保留构建时事实；后续用户验收另记，不改变安装包溯源或虚报实机结果。提交及远端同步结果以Git历史和独立远端回读为准。
