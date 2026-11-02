# P2-C-R1 正式提交回执

日期：2026-10-30。用户已批准 R1 代码开发及专项自动化验收，并授权正式提交/推送。本回执记录提交后的状态；[原 R1 验收报告](MCP_PHASE2C_R1_ACCEPTANCE.md)及其“待验收/未正式提交”字段保留为提交前冻结快照，不据其旧字段否认本次已提交事实。

## 提交与远端核验

- 分支：`feat/scheduled-items`。
- Commit：[`9868a50f1d05ea497a43b61bdc1bb85ad99cbfe7`](https://github.com/bluntvoice/In-Line/commit/9868a50f1d05ea497a43b61bdc1bb85ad99cbfe7)。
- 标题：`fix(installer): restore R1 metadata consistency and rollback`。
- 父提交：`ebeb57106a898fb47e0a0d3a6ca05e7b56cfbe75`，即上一轮测试资产独立提交。
- `git ls-remote` 已核验开发分支远端 SHA 与本地完全一致。未合并 main；main 保持 `1baac96252af2665274f2a955b3adacfa80519fd`。
- 未创建 Tag/Release，未更改正式更新通道，未安装或升级办公电脑软件，未终止正式 Codex/In-Line 进程或连接。

## 提交前审计与文件清单

18 项文件逐项 SHA-256 与本轮批准的冻结清单一致。其中 10 项源码/测试资产与候选 `2b5ad71951374b49222682825147e59f1a894f2d` 的 Git 内容一致；另外 8 项为已审阅的文档、PRD 和证据，不属于产品源码差异。只按明确路径暂存，未使用 `git add .`。

暂存内容的后缀、差异和凭证模式检查通过：没有 Token/私钥/签名下载链接模式命中，没有数据库、安装包、构建缓存或私有 Git 索引脚本。历史 12 项未跟踪资料的原摘要均未变。

提交文件完整清单：

| 类别 | 文件 |
|---|---|
| 测试 Workflow | `.github/workflows/native-nsis-acceptance.yml` |
| 原生测试 | `scripts/test-native-nsis.ps1` |
| 文件组组件测试 | `scripts/verify-installer-guard.ps1` |
| 元数据组件测试 | `scripts/verify-installer-metadata.ps1` |
| 静态边界检查 | `scripts/verify-installer-safety.mjs` |
| 文件事务 | `src-tauri/windows/InstallGuard.cs` |
| 元数据事务 | `src-tauri/windows/InstallMetadata.cs` |
| 辅助程序包装 | `src-tauri/windows/install-guard.ps1` |
| NSIS 钩子 | `src-tauri/windows/installer-hooks.nsh` |
| NSIS 模板 | `src-tauri/windows/installer.nsi` |
| 维护说明 | `src-tauri/windows/README.md` |
| 项目说明 | `README.md` |
| 变更记录 | `CHANGELOG.md` |
| 开发构建说明 | `RELEASE_NOTES.md` |
| PRD | `docs/prd/app-update.md` |
| R1 验收报告 | `docs/MCP_PHASE2C_R1_ACCEPTANCE.md` |
| 原 R2 技术调查 | `docs/MCP_PHASE2C_R2_HELPER_WAIT_REVIEW.md` |
| R1 结构化证据 | `docs/mcp/acceptance/MCP-PHASE2C-R1-20261030-checks.json` |

临时分支没有被合并或删除；没有纳入首次注册 Workflow 用的临时 push 引导、仅构建脚本或忽略目录中的产物。已审阅 Workflow 自身保留了严格限定的候选来源校验，这不是自动构建/发布入口。今后若另获授权从正式提交重建，应明确输入当前完整 `product_ref` 并将 `candidate_branch` 留空；过期默认 SHA/旧候选基线会安全拒绝，不绕过来源检查。

## 正式提交 CI

[CI 41021671671](https://github.com/bluntvoice/In-Line/actions/runs/41021671671)，实际 `head_sha=9868a50f1d05ea497a43b61bdc1bb85ad99cbfe7`，最终 `completed / success`。

| 检查 | 结果 |
|---|---|
| Actions 运行时策略 | 通过 |
| 前端 | 22 个测试文件、135 项测试通过 |
| TypeScript / Vite 构建 | 通过 |
| Rust fmt | 通过 |
| Rust 测试 | 合计 248 项通过 |
| GUI / MCP / CLI 三程序离线构建身份 | 与本次正式提交一致，检查通过 |
| 16 工具完整参数/注解契约 | 通过 |
| MCP 初始化、工具列表、16 匿名调用及重复调用协议 | 通过；不等于已授权业务矩阵或当前正式会话验收 |

产品源码及测试资产与候选无差异，因此没有以“验证新逻辑”为名重复执行原生套件；也没有把旧包改称本提交的安装包。新 Git SHA 会改变构建身份，发布前仍须从最终批准源码重新打包并完成最终整体回归。

## 保留的原生证据与未通过项

原生结果仍对应候选 `2b5ad71951374b49222682825147e59f1a894f2d` 的[原生回归 41015746222](https://github.com/bluntvoice/In-Line/actions/runs/41015746222)：

- 13 个原生 Job 中 12 个满足断言，`ps-hang` 未通过，Workflow 总状态仍为 failure。
- 50 次真实 NSIS 安装/卸载操作，497 条原生断言；组件检查为 x64 31 项、x86 31 项、元数据 17 项。
- 包 SHA-256：`a6ae36da4f835708886e14ba2c335602d6943db021ca8984e6e2c02e7af3fea2`。
- 15 个 Artifact ZIP、完整结构化结果及必要脱敏日志在本地 `release/MCP-Phase2C-R1-20261030/run-41015746222/` 保留，未加入 Git。GitHub Artifact 有保留期限；本地资料不作清理。

本次授权只通过 R1 专项。原生总套件、GUI 交互、异常断电、企业策略、旧版真实业务库迁移及其他未完整覆盖项没有变成通过；P2-B 自动恢复仍未完成，P2-D 和 Phase 3 未启动。

## 工作区与下一步

本回执形成时，已跟踪工作区干净、暂存区为空，12 项历史未跟踪资料保留。用户现已审阅本回执及[R2 A/B 方案](MCP_PHASE2C_R2_OPTIONS.md)并选择 B；本轮对两份文档作必要补充，连同 R2-B 收尾和 P2-D 计划等待文档验收，尚未追加提交或推送。忽略目录中的构建/下载/审计资料保留。

用户已正式选择 R2 方案 B：保留当前安全等待、恢复屏障及原失败断言，长时挂起作为已知兼容性限制保留，暂缓产品开发，不实施方案 A。R1 提交与 CI 通过不等于 P2-C 原生总体验收通过，也不构成发布豁免。当前状态及操作说明见[R2-B 收尾](MCP_PHASE2C_R2_B_CLOSEOUT.md)。下一步优先规划[P2-D](MCP_PHASE2D_ACCEPTANCE_PLAN.md)，本轮未启动实际验收或 Phase 3。
