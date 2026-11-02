# P2-D 最小隔离验收报告

日期：2026-11-02（Asia/Shanghai）。**用户已确认 P2-D 最小隔离验收通过；正式 Codex 宿主接入及原会话自动恢复未验证。** 本次确认授权将下列三项成果正式提交至开发分支；不包含 MCP 产品开发、正式环境操作或 Phase 3 业务测试。

本报告中的运行 SHA、程序摘要、PID 和协议记录保留为原临时候选的实际证据。正式提交将产生新的构建身份，须单独核对新 SHA 的标准 CI，不以候选二进制冒充正式提交构建。后续只准备 Phase 3 方案，实际执行仍需另行授权。

## 来源与执行环境

- 验收与正式提交的起始基线：`5cae01c9de1eb5a0d239704627dd50a4d24a3da5`，`feat/scheduled-items`；原测试阶段未推进开发分支或 main，现仅获准提交本报告列明的三项成果。
- 临时分支：`codex/p2d-isolated-acceptance-20261102`，独立工作树从上述精确基线创建；只增加 `src-tauri/tests/mcp_connection_acceptance.rs`。
- 首轮：`5387295e7f31952abef928e5b0458e0b6f2ac51c`，[CI 41373481329](https://github.com/bluntvoice/In-Line/actions/runs/41373481329)，通过。
- 最终候选：`b997b3fe78a37c27363c433d9eda39d34f7a02a2`，[CI 41373819052](https://github.com/bluntvoice/In-Line/actions/runs/41373819052)，**completed/success**。在首轮基础上补充脱敏结构化日志和进程创建时间核验；Job `125049680992`，最终源码与该候选一致。
- 复用现有 `.github/workflows/ci.yml` 的 `windows-latest` 托管 Job；首轮实际镜像为 `windows-2025-vs2026` / `20261021.278.1`，不是本地或 self-hosted 环境。最终镜像详见结果 JSON。
- 不新增或修改 Workflow，不触发 dev-build / native-nsis / Release；办公电脑仅进行源码编辑、格式检查、Git 操作及证据读取，没有启动本轮 MCP/CLI 夹具或执行安装。

## D01～D06 结果矩阵

两轮四个新增测试全部通过；下表以最终候选的断言和 61 条可回读结构化记录为准。前端 **135/135（22 文件）**、Rust **252/252（原有 248 + 新增 4）**，无失败/忽略；TypeScript/Vite 构建、rustfmt、三程序离线构建身份、实际 16 工具契约与匿名协议检查全部通过。正式宿主未验证项目独立列出，不以隔离成功覆盖。

| 项目 | 隔离验证及证据 | 状态与边界 |
|---|---|---|
| D01 首次接入 | 空安全库创建唯一新身份和 600 秒接入包；实际 importer 导入合成 Codex 配置，回执 `configurationVerified=true`、`currentSessionVerified=false`；启动实际 sidecar，`get_capabilities` 核对身份及构建，再读取已知合成事项 | 通过；正式 Codex 自动加载/会话接入未验证 |
| D01 凭证拒绝 | 另一个全新、没有已授权 runtime binding 的夹具，分别使用缺失、格式无效和另一合成客户端的 Token；实际 capabilities 与查询均返回 `unauthenticated`，无业务数据 | 通过；有明确授权的 runtime binding 刷新是另一场景，不能与“完全无授权”混为一谈 |
| D02 身份与凭证复用 | 现有 Rust 测试覆盖同 ID 重签、两次等价导入、客户端总数/授权密文/配置字节及文件身份不变；现有前端测试覆盖唯一身份选择、多身份消歧、排除 CLI 和已撤销身份；过期、已使用、格式/绑定错误和导入失败分别断言 | 对应原有测试通过；复用证据而非新增重复测试；原生 GUI 交互未验证 |
| D03 活连接配置变化 | 原 transport 对象、持有的 Child、PID 和创建时间保持不变；合成其他 MCP 配置、model/注释和 `default_tools_approval_mode=prompt` 被保留；两次等价导入 `configurationChanged=false`，配置字节/文件身份/授权不变，`credentialResolution.changed=false`；磁盘改成合成无效凭证后复用拒绝，原连接继续读哨兵 | 通过；显式轮换造成真实凭证变化复用旧活连接测试；磁盘配置变化不表示宿主重载 |
| D04 交集、限域及撤销 | 先以全局默认限制较宽客户端，再以默认客户端限制较宽全局；两方向均仅 regularRead；私密投影和两次写请求返回 `forbidden`；原连接收窄为 A/T 后只读 A 哨兵，指定 B ID 不返回数据；禁止 regularRead、暂停及撤销后继续在原连接验证拒绝；另一合成客户端权限/范围和两条哨兵不变 | 通过；仅夹具配置和授权调整、只读与拒绝路径，没有新增成功 MCP 业务写入。全局暂停按设计影响所有客户端，不声称其他客户端在全局暂停期间不受影响 |
| D05 EOF 与新连接 | 关闭测试持有的 stdin；在原 Child 句柄上等待退出、检查成功退出和 stdout EOF；之后由驱动新建进程/连接并再次读到哨兵，最后正常 EOF | 隔离子项通过；这是明确的新测试连接，不是原 Codex 会话自动恢复；正式宿主恢复受现有能力限制，未验证 |
| D06 升级状态 | 仅复用 P2-C-R1 的 `mcp-lock` 42 条、`upgrade` 25 条、Windows 2025 `lifecycle` 61 条通过断言及既有 2022 对照；15 个原始 ZIP 重新核对 SHA-256 全部匹配 | 历史证据子项通过；没有新安装。原会话跨升级接入/自动恢复、真实业务库迁移未验证 |

D04 中“范围外拒绝”遵循现有查询契约：指定 B ID 的查询正常返回空结果，不伪造一个产品没有返回的权限错误。凭证错误、无读取权限、暂停及撤销则分别断言实际错误码。

## 新增与复用的具体断言

新增四个测试均在同一个 test-only 文件内：

1. `p2d_d01_first_import_authorized_sentinel_credentials_and_d05_eof_reconnect`：完整首次导入到只读哨兵链路；原进程 EOF 结束与新连接分开证明。
2. `p2d_d01_unbound_missing_invalid_and_mismatched_credentials_are_rejected`：无已授权绑定的新夹具中验证三种凭证拒绝，不以正常的授权刷新冒充拒绝失败。
3. `p2d_d03_equivalent_import_preserves_active_transport_host_policy_and_loaded_credentials`：配置/加载状态分离，重复导入和错误磁盘凭证不更改授权；同一进程持续读取。
4. `p2d_d04_original_connection_enforces_live_intersection_scope_pause_and_revocation`：原连接实时执行权限收窄、限域、暂停/撤销和另一客户端对照。

复用并由现有 CI 自动重跑的主要资产：

| 文件/模块 | 复用覆盖 |
|---|---|
| `src/lib/mcp-onboarding.test.ts` | 唯一身份复用、多身份显式消歧、首次授权可用性；共 6 个既有前端断言用例 |
| `src-tauri/tests/mcp_onboarding.rs` | 原 stdio 连接从无凭证经授权获得绑定；显式轮换使原凭证失效并经新包恢复；暂停/撤销；实际 importer 与同 ID 两次等价复用 |
| `mcp::onboarding::tests` | `reuse_reissues_for_same_identity_without_changing_store_token_scope_or_config`、`reuse_never_falls_back_to_grant_or_rotation_for_missing_mismatched_or_stale_credentials`；过期与时钟倒退、格式/绑定错误、已使用/中断包、发包失败不新建或轮换身份 |
| `mcp::codex_import::tests` | 保留其他 MCP/宿主审批策略/格式；非法选项静态脱敏拒绝；ACL 不正确或 stdio 验证失败不消费/不替换；并发配置冲突；身份、权限、范围、版本匹配 |
| `runtime_credentials::tests`、`security::tests` | 实际 changed 与绑定已应用分离；PID 复用/错误程序/损坏绑定拒绝；授权交集、暂停/轮换/撤销；持久化失败不伪称撤销成功 |
| `mcp_stdio.rs`、`mcp_security.rs` | 既有实际 stdio 读、范围过滤、无权字段不泄露、匿名拒绝和原连接授权复核 |
| `cli_foundation.rs`、`tests/cli/phase6.rs` | 既有 CLI 独立身份、导入回滚和离线启动边界辅助回归；不将 CLI 身份当作 MCP 接入证据 |
| 既有身份/契约脚本 | 三程序离线构建身份及版本一致；实际 16 工具的名称、完整 schema/注解和匿名拒绝；不是 Phase 3 的完整业务矩阵 |

没有修改已有测试断言，也没有 `continue-on-error` 或跳过失败。既有 CI 按原配置运行既有回归套件；本轮新增 MCP 请求仅有只读成功和写入拒绝，不新增成功业务写入用例。

## 元数据与脱敏记录

最终候选仅向普通 CI 日志写入白名单 JSON：合成用例别名、测试子进程 PID/创建时间、sidecar 二进制 SHA-256、协议结果码、权限/限域、凭证解析状态、构建版本/Commit、返回条数及 EOF 退出码。没有记录 Token、Token 哈希、客户端长凭证、DPAPI 密文、接入包、原始配置或原始业务响应。

`P2D_EVIDENCE` 行不改变断言或 Workflow。测试中的 30 秒 deadline 仅使夹具测试失败，不是产品超时要求，不调用强杀，也不改变 R2-B 决定。失败时保留 CI 日志及分支；不为获得全绿修改产品或放宽判定。

最终结构化矩阵、61 条原始白名单记录、复用断言日志摘录、摘要和保全核对已保存在 [P2-D checks](mcp/acceptance/MCP-PHASE2D-20261102-checks.json)。Windows FILETIME 原始十进制值在摘录时转为字符串，避免 JavaScript 数值精度损失。不上传含凭证的夹具目录；现有 CI 不生成本轮安装包或独立证据 Artifact，结构化证据来源为可回读的 Job 日志。

最终新测试共记录 **8 个 sidecar 启动和 8 次 EOF 正常退出（全为 0）、5 次导入成功、35 次工具调用**；其中 14 次为预期拒绝（`unauthenticated` 6、`forbidden` 5、`paused` 1、`revoked` 2）。预期拒绝有独立断言和原始错误码，未删减或改成成功结果；未发现新的产品缺陷。

sidecar 为软件 `0.5.0` / MCP v1 / 数据库 schema 14，实际二进制 SHA-256：`2c1fdb187d79ef2d4b188f502711bc9dd4309e1acaf8d48c4d35e1d57c4bf6db`。D03 原进程 PID 1752、D04 原进程 PID 8064 与对照 8328；D05 原进程 7812 已 EOF 结束，新进程 9492 由驱动建立。创建时间和每次结果见 JSON；不将这些临时 PID 当作正式客户端身份。

## D06 历史来源及限制

[R1 原生运行 41015746222](https://github.com/bluntvoice/In-Line/actions/runs/41015746222)及[其冻结结果](mcp/acceptance/MCP-PHASE2C-R1-20261030-checks.json)来自 `2b5ad71951374b49222682825147e59f1a894f2d`，候选安装包 SHA-256 为 `a6ae36da4f835708886e14ba2c335602d6943db021ca8984e6e2c02e7af3fea2`。它不是本轮新构建或新安装的包。

已核对该候选到当前基线的可执行产品源码、NSIS/C# 安装实现及依赖/版本没有变化；相关目录只有维护 README 的后续文档差异。本轮临时分支也仅有新增测试。因此可按原范围复用文件占用保护和生命周期证据，不能外推尚未测过的正式会话能力。

历史 `mcp-lock` 证明目标及异目录同名 sidecar 的 PID/创建时间/摘要保留，安装失败 1、实际卸载体失败 1，夹具目标正常 EOF 后重试安装 0；不证明它们已经是有授权的 Codex 会话或调用中的业务连接。历史升级/修复保留的是合成存储与配置字节，不是旧版真实业务库迁移的完整语义验收。本轮未发现需要扩大 D06 原生执行范围的新产品差异，未启动补测。

## 修改清单与工作区保护

| 文件 | 位置及状态 |
|---|---|
| `src-tauri/tests/mcp_connection_acceptance.rs` | 从最终候选原样复制到开发分支，Git blob `58873d6fb605247f5a018ae68258c42b78607a19` 不变；不合并整个临时分支 |
| `docs/MCP_PHASE2D_ACCEPTANCE.md` | 已验收报告；本次仅补充用户确认及提交边界 |
| `docs/mcp/acceptance/MCP-PHASE2D-20261102-checks.json` | 已验收脱敏结果；原运行证据保留，本次更新验收状态 |

本轮审计资料另保存在被忽略的 `release/MCP-Phase2D-20261102/`，不暂存。开始前的 850 个已有文件逐一核对未变化：12 项历史资料、2 份已审阅准备文档均保留；15 个原生 ZIP 完整。正式工作区已有跟踪内容、真实暂存区、开发分支/main 及原临时分支不清理、不覆盖。

未修改 MCP 产品代码、NSIS、授权/数据保护机制、配置入口、版本/依赖或更新通道；未读取真实客户端授权/数据库/Codex 配置，未触碰当前正式连接，没有本地安装升级、Tag/Release、常驻代理或 App Server 接管。

## 尚未满足的目标与停止点

- 正式 Codex 会话首次接入、当前宿主加载配置及自动重连均未验证；P2-B 自动恢复仍未完成，不重新调查既有不可达入口。
- 原生 GUI、异常断电、真实企业策略、旧真实业务库迁移及完整业务工具矩阵仍未完整覆盖。
- P2-C 原生总套件保持 **12/13，`ps-hang` 未通过**；R2 选择 B，长时间安全等待及兼容性限制保留，没有获得发布豁免。
- Phase 1 已完成；P2-A 调查已完成但宿主限制未解决；R1 已正式提交；P2-D 最小隔离验收已获用户确认；Phase 3 业务验收未启动，Phase 2 整体未完成。正式发布仍未获授权。

本次获准精确提交上述三文件，推送并核对正式提交 CI；完成后只整理 Phase 3 最小业务验收方案，再次停止等待确认。不启动 Phase 3 实际测试、成功业务写入、产品修复或正式安装/发布。
