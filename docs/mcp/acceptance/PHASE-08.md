# P8 稳定性、契约及统一集成验收准备

2026-10-12。状态：**开发验证与验收材料完成，待统一实机验收**。用户授权各阶段独立提交推送，P8集中验收。P7提交`ba38b1002d3da15192fffab896168708918f6748`已推送，远端同SHA；P7 CI38215281440 success且精确SHA；main与P0 Tag不变。P1–P8无accepted Tag，未进入P9/发布/生产安装。

## 对照需求与文件

043、051–052、089–094、098–099、139–144，补054和诊断跨阶段回归。详见[集成规程](../integration-and-regression.md)、[统一验收清单](UNIFIED-P8-CHECKLIST.md)、[144项CSV](../requirements-traceability.csv)。源文件及原决定不改。

- `diagnostics.rs`：用户选A批准数值损坏校验；时间/计数/状态/容量，checked_add及持久化前校验；永久两条真实文件与运行态负例，保留损坏原字节。
- `scripts/verify-mcp-contract.mjs`和`contracts/tools-v1.json`：P7真实16工具输入schema/注解基线；缺失旧工具、字段类型变化和readonly变化负例均拒绝。
- 三个workflow：CI实际debug契约+匿名认证；开发/正式NSIS两个实际sidecar输入契约与认证门禁，在上传/发布前检查。只修改门禁，未派发开发包或触发Tag/Release。
- 文档、144矩阵与各阶段精确Commit、长期回归/首次失败/历史复现/迁移规程及可执行实机清单。

## 已验证与仍需补证据

P8全量Rust、前端、组件浏览器、真实更纱胶囊、fmt/TS/Vite、契约、原始文档及NSIS结果将在最终节填入实际日志/数量，未结束不标通过。首次数值负例P8-numeric-first退出101，原实现接受损坏started；按A补校验后全量重测。关键断言保持，无ignore/skip或临时隔离。

当前会话直接get_capabilities仍unauthenticated，`P8-current-session.json`；只证明服务响应，未证明读取权限、自动导入或重载。CLI0.162.0-alpha.17.2及desktop26.1007.2314.0公开help重核，`P8-codex-public-channel.txt`；mcp命令未提供reload，本会话可调用工具无公开reload，不能据此宣称整个产品不存在内部能力。未启动新app-server/业务实例，未改配置或授权，严格入口保持关闭。

P7 CI初次REST EOF后独立重试读回success（非产品测试失败，不靠重跑放行）。P7包保留原摘要与证据，修复前历史包不能验收P8数值修复。P8包构建冻结与摘要待最终回读。

## 未完成、风险及验收出口

1. **严格零额外操作Codex当前会话导入/重载未成功，功能关闭**。不能改成手填Token或重启来通过。MCP-044/100等仍部分完成/待实测，属于整体验收阻断项。
2. **原生安装升级/旧真实数据保留、生产授权/实际读取写入、多屏更纱观感未执行**；仅构建及合成隔离验证不能证明。按统一清单实机验收，自动测试通过不等于P8验收通过。
3. 大规模性能完整预算未测量；历史版本worktree迁移没有具体待迁移缺陷，本次不虚构已执行。已有受控规程，未来个案需当前影响证据和批准。
4. MCP-099允许移除并非本次必须移除；旧两工具保留，实际删除另行确认和迁移披露。P9三条未启动，发布清单/应急/热修另获阶段授权。
5. P0/P1严重度阻断发布；未执行项不算“零缺陷”。首次失败和后续日志保留，不削弱测试。

完成本阶段独立提交推送后停在P8等待用户统一验收，不标通过、不建accepted Tag、不进入P9，不创建Release。

## 自动验证结果

| 验证 | 命令/实际证据 | 结果 |
| --- | --- | --- |
| Rust全部层次 | `cargo test --locked -j1 --manifest-path src-tauri/Cargo.toml`；[完整日志](evidence/P8-rust-final.txt) | 173通过：165库+2sidecar+6真实集成；零忽略，编译2m18，库测试6.36秒，最长主服务集成2.57秒。测试耗时不作大规模性能预算 |
| 前端旧功能 | `npm.cmd test -- --maxWorkers=1 --no-file-parallelism`；[日志](evidence/P8-frontend.txt) | 20文件115通过 |
| 实际协议契约/匿名授权 | 两条node命令；[日志](evidence/P8-contract-actual.txt) | 完整16工具input schema/注解匹配；16匿名工具+3重复调用无业务数据、6次起限流、纯JSON stdout |
| 漂移负例 | [日志](evidence/P8-contract-gate-negative.txt) | 旧工具移除、参数类型、readonly注解变化均失败，基线不自动更新 |
| 复合命令失败退出 | [日志](evidence/P8-native-failure-gate.txt) | 实际子pwsh原生命令退出7，后续命令未运行；三个workflow逐命令守卫 |
| 六组合成UI | `verify-mcp-{undo,batch,jobs,cache,maintenance}-ui.mjs`及`verify-task-ticket-colors.mjs` | 审阅/取消不写、单次批准、冲突拒绝、取消与缓存/维护安全、右键自选色和未来子任务均通过；对应P8-verify-*.txt保留 |
| 更纱及胶囊 | `verify-capsule-alignment.mjs`；[日志](evidence/P8-capsules.txt) | 真实推荐WOFF2按摘要只读加载，CDP确认custom font，四字体/100%与150%布局通过；不代替原生多屏观感 |
| fmt | `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`；[日志](evidence/P8-format.txt) | 退出0 |
| TS/Vite | NSIS beforeBuild执行`npm.cmd run build` | 通过，资源index-BXJaxZ0Z.js/index-Dbjf6Eu2.css |
| 144项/原件 | `verify-phase8-documents.ps1`；[日志](evidence/P8-documents.txt) | 原三份和副本摘要不变、144原决定、13字段、版本0.5.0及链接；16条P8主阶段、P9三条未进入 |
| Actions策略 | `check-github-actions-runtime.ps1`；[日志](evidence/P8-actions-policy.txt) | 通过 |
| 生产保护 | [独立摘要](evidence/P8-production-readback.json) | 两份已安装exe未变，未安装本次构建 |

首次失败完整保留：数值极值测试exit101；文档旧MCP-036仍指向拟建被新门禁拒绝，改为实际16工具排除附件边界并复验通过（P8-documents-first）。没有削弱测试、跳过断言或临时隔离。软件构建/包的摘要用于产物溯源，非故障导出哈希清单。

提交检查首次因公开CLI help原输出末尾空格拒绝（P8-staging-format-first），原始输出另存忽略目录release/P8-codex-public-channel-raw.txt，仅移除入库help证据的行末空格；命令、内容和产品断言未变。

## 最终冻结包与独立开发提交

[NSIS日志](evidence/P8-nsis-first.txt)退出0：sidecar3m51、主5m54，TS/Vite及makensis成功。163份冻结包源码在完成后逐一摘要核对未变。保留本地`release/In-Line-P8-local-20261012.exe`，**4,213,972字节**，SHA256 **`2AD7D37CD5475FDB0532A3C0A4A84539C9D6CFBC901DBA105A005B456E9A0B94`**，见[产物记录](evidence/P8-retained-package.json)。该包包含本次数值校验修复；P7历史包保持原摘要。

[包回读](evidence/P8-package-readback.txt)：主PE0.5.0、NSIS清单和两份不同摘要的sidecar输入均实际16工具契约、匿名认证、限流及纯协议通过。没有提取或安装本次包，输入回读不能冒称安装升级验证。构建时源码未提交，嵌入已推送P7基线/dirty、exactCommit=null，不事后伪造未来P8Commit。

本报告及完整证据在本次独立P8开发提交内，确定SHA由`git log --format='%H %s'`中`test(mcp): 完成P8契约门禁数值校验与统一验收准备`及远端分支回读确认，避免将提交自己的未来SHA写入构建。自动CI针对该精确SHA运行；开发/正式包workflow未派发。开发提交不代表用户验收。

下一步由用户按[统一清单](UNIFIED-P8-CHECKLIST.md)实机验收，反馈逐项通过/失败/未执行。严格Codex接入仍阻断整体验收，当前不进入P9/创建Tag或Release。
