# P1 客户端接入：软件内实现与隔离验收

日期：2026-10-10。用户选择A：先完成软件内流程与隔离测试，自动接入通过实机验证后再开放。P1尚未验收，当前Codex支持门禁关闭。

## 变更

- `src/components/McpSecuritySetting.tsx`与`src/lib/mcp-onboarding.ts`：选择Codex、常规读取默认开、范围折叠；无需填写名称/ID/Token。前端只存短期提示词，复制失败复用授权，重复点击互斥，未验证客户端按钮关闭；既有授权权限和撤销仍可管理。
- `mcp/onboarding.rs`：10分钟DPAPI包、私有ACL、机器/SID/客户端类型/身份绑定、一次消费和中断状态、过期惰性清理，每次清理最多扫描100个目录。授权与创建包失败不提交新的授权状态；不恢复或隐式轮换旧凭证。
- `mcp/codex_import.rs`：选定用户级配置、TOML解析合并、同名/损坏/参数/环境冲突拒绝、独占导入锁、提交前/后校验、加密备份及有条件回退。配置成功采用当前用户私有ACL，回退保留原ACL。外部编辑器不共享此锁，不能宣称它们受同一串行锁保护。
- sidecar独立`--import-client`命令：无Token参数；普通无参数stdio仍为MCP。实际子进程按写入env核对身份、权限交集、范围、修订和版本；返回configurationVerified与currentSessionVerified，两者不混淆。能力查询新增仅当前身份的clientId供核验，无新增业务工具。
- release导入、Tauri准备/轮换与UI均关闭未通过实机验收的Codex；debug测试核心用于隔离验证，不表示正式支持。

## 证据状态

| 命令/检查 | 结果 | 证据（本目录evidence） |
| --- | --- | --- |
| `npm.cmd test -- --maxWorkers=1 --no-file-parallelism` | 18文件、109测试通过 | P1-onboarding-frontend-first.txt |
| `npm.cmd run build` | TypeScript/Vite通过 | P1-onboarding-build-retry.txt |
| `cargo check --locked --manifest-path src-tauri/Cargo.toml -j 1` | 通过 | P1-onboarding-check-retry.txt |
| `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` | 退出0，零输出 | P1-onboarding-format.txt |
| `cargo test --locked --manifest-path src-tauri/Cargo.toml -j 1` | 最终89测试通过（82库、2sidecar、5集成） | P1-onboarding-rust-verified.txt |
| `npm.cmd run dist`，`CARGO_BUILD_JOBS=1` | 本地NSIS构建通过；MSVC/SDK预检通过 | P1-onboarding-nsis-first.txt、P1-onboarding-build-tools.txt |
| `docs/mcp/verify-package.ps1` | 两个sidecar输入均通过真实stdio、匿名认证拒绝和限流；版本及NSIS输入清单通过 | P1-onboarding-package-readback.txt |
| 两个release sidecar的`--import-client` | 均返回onboarding_unavailable、退出1；当前Codex配置哈希不变 | P1-onboarding-release-gate.txt |
| `docs/mcp/verify-phase1-documents.ps1` | 三原件/副本、144行、原决定、五版本及P0阶段边界通过 | P1-onboarding-documents-final.txt |
| Actions运行时策略、`git diff --check` | 通过 | P1-onboarding-actions-policy.txt、P1-onboarding-diff-check.txt |

首次Rust87测试、最终中间89测试日志也保留；以P1-onboarding-rust-verified.txt对应最终源代码。真实导入命令→配置→sidecar集成用例使用独立合成安全库和业务库，核对当前客户端身份、权限交集、范围及修订；成功结果仍为currentSessionVerified=false。集成用例约0.49秒，非性能基准。

负面用例覆盖：包过期、伪造绑定、重复消费、中断状态、私有ACL、创建包失败不提交授权、暂停/撤销/轮换后拒绝；TOML损坏、同名外国服务、禁用、额外参数或环境冲突拒绝；真实stdio验证失败后按原字节及ACL回退。外部编辑器并发更改导致无法证明安全回退时保留中断状态，不盲目重试。

安装包为`src-tauri/target/release/bundle/nsis/In Line_0.5.0_x64-setup.exe`，3,711,055字节，SHA-256：`FE39AD1C530896B9157C6ED92F443FBC976707644665C4CBE91AC9B97C48F8B6`。这是本地开发产物，未安装、未上传或发布。两个NSIS sidecar输入哈希不同，均分别完成协议与关闭入口检查，不能称为字节一致；也未声称已解包核验最终安装文件。此前P1安装包私有回退副本保留，路径见P1-onboarding-package-checkpoint.txt。

首失败均保留：offline缺少已锁定toml_edit缓存（切回locked正常下载）；TypeScript遗漏旧组件类型引用（改为新模型）；文档校验发现MCP-044原决定被最新授权覆盖（恢复冻结原文，将新授权单列，未降低校验）；patch准备两次拒绝同路径重复操作，均在修改前拒绝，改为单个Update。实际日志不含凭据。

## 尚未通过的门禁

当前Codex26.1007.2314.0没有已验证的当前实例自动重载入口；GUI点击、生产授权提示词、当前会话导入和重载、本轮安装升级及旧数据打开均未执行。跨账户/机器包的测试先覆盖伪造绑定，不能称为跨账户/机器实测。原业务数据库本轮未操作；已安装主程序和sidecar的SHA-256均与之前检查点一致。HEAD仍为f2a97b34410f850f3da8c19d4ec6e21c7d63048f、分支feat/scheduled-items，未提交/推送/Tag/Release，未进入P2。

下一步先继续验证受支持的当前Codex实例重载通道，成功后安排本轮软件安装、GUI授权和一次提示词接入实机验收；自动入口在此之前保持关闭。软件内实现和隔离测试已完成，MCP-044与P1仍部分待验收。
