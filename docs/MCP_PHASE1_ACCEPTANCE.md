# MCP Phase 1 验收报告

日期：2026-10-28（北京时间）。状态：开发、自动回归及本地包核对完成，等待用户验收；未安装、提交或推送。

## 1. 授权、范围与实施结论

用户本轮明确认可 Phase 0，选择 D2-A、D3-A，要求 Phase 1 完成后等待验收，验收后才提交推送及进入 Phase 2。本阶段按该新指令执行，没有沿用旧任务“先提交、P8统一验收”的安排。

- **D2-A 已落实**：普通接入修复选取既有 Codex MCP 身份，凭证仍有效时重签该 ID 的新一次性包；不创建替代身份、不轮换、不改权限/范围/全局开关。无法安全取得凭证时停止，仅提示由用户明确选择同 ID 轮换，按钮标明旧凭证立即失效。
- **D3-A 已落实**：保留当前私有 Codex 配置进程 env 和 DPAPI 存放方式，不迁移凭证架构，不新增可恢复的长期秘密库、常驻服务或中转层。
- **状态语义已分开**：配置验证、配置文件变化、凭证实际变化、本次服务调用、原对话会话验证不再用同一个新字段表示。旧字段保留兼容并弃用。
- **配置合并采用局部更新**：安全的宿主策略原样保留，其他服务器/配置字节保留；等价配置不替换文件，私有 ACL 仍必须满足。危险或未知选项不自动放行。

没有开发客户端删除、修改安装器或实现宿主重载；自动恢复依然是 Phase 2 的关键未解决事项。没有安装/升级正式环境、重启 Codex、终止正式客户端进程、修改真实工作事项、提交/推送或创建 Tag/Release。

## 2. 修改清单与根因对应

下列清单以本轮开始的工作区为基线，包含此前未提交补丁上的增量，不把此前工作重新算成本轮实现。

| 文件 | 本阶段增量及目的 |
|---|---|
| `src-tauri/src/mcp/runtime_credentials.rs` | 新增安全的解析结果 `source/changed`；实际比较新旧凭证，重复应用同绑定不再称为值变化；保留旧 bool 接口兼容 |
| `src-tauri/src/bin/in-line-mcp.rs` | 能力响应新增精确状态和调用验证范围；旧两个兼容标志继续返回原“应用绑定”含义，不再作为会话验收依据 |
| `src-tauri/src/mcp/codex_import.rs` | 拓宽已知安全宿主字段的类型验证；局部保留原文；等价配置不替换；返回 `configurationChanged`；补齐无变化路径和发布前私有 ACL 检查；增加兼容、格式、安全和回滚回归 |
| `src-tauri/src/mcp/onboarding.rs` | `reuse_at()` 仅对显式选中、仍有效的 Codex 身份重签短期包；签发前后检查授权修订和存储一致性；补充不丢授权/不改变范围的测试 |
| `src-tauri/src/mcp/contract.rs` | 增加“无法安全复用凭证”的脱敏错误说明，明确用户可选同 ID 轮换及其影响 |
| `src-tauri/src/lib.rs` | 增加受现有本地主窗口检查约束的 `mcp_reuse_onboarding` Tauri 命令；不新增 MCP 工具 |
| `src-tauri/tests/mcp_onboarding.rs` | 同一 stdio 连接下验证首次变化/重复不变；用新合成包重复导入验证身份、权限、范围、授权库和配置文件身份不变 |
| `src/api.ts` | 对应本地复用命令的类型化调用 |
| `src/components/McpSecuritySetting.tsx` | 已有身份/明确新增选择；单候选默认复用，多候选必须选择；复用时原权限/范围只读；复制失败、过期、重开和显式轮换保持所选身份；区分配置/会话/自动恢复 |
| `src/lib/mcp-onboarding.ts` | 过滤可复用身份及单/多候选选择规则 |
| `src/lib/mcp-onboarding.test.ts` | 排除 CLI/撤销身份和多候选消歧回归 |
| `scripts/verify-mcp-phase1-ui.mjs`（新） | 真实组件、合成 API：12 项复用流程检查和 6 个窄窗/放大布局场景 |
| `scripts/verify-mcp-button-spacing.mjs`（原有未跟踪脚本） | 将单一已有身份场景改为验证默认复用及复制不重复签发，保留原布局矩阵 |
| `docs/prd/mcp.md` | 新增最新阶段要求及历史记录边界 |
| `docs/mcp/connection-guide.md` | 新流程、字段迁移、安全和实机边界说明 |
| `docs/mcp/client-onboarding-proposal.md` | 记录本轮 D2-A/D3-A 与新阶段门禁 |
| `README.md` | 最新开发状态及历史断连表述的时间边界 |
| `CHANGELOG.md` | 未发布的 Phase 1 变更摘要 |
| `RELEASE_NOTES.md` | 可编辑的开发构建说明，不构成发布或安装授权 |

另新增本报告和[脱敏检查结果](mcp/acceptance/MCP-PHASE1-20261028-checks.json)。未改 `security.rs` 的授权持久格式、业务 schema、依赖/版本文件、安装器和 Actions。原先 `src/styles.css`、`McpStorageGate.tsx`、`SettingsPanel.tsx` 的未提交修改保持原字节。Phase 0 报告和既有验收材料保持。

## 3. 状态字段与兼容方案

| 字段 | 精确定义 | 兼容处理 |
|---|---|---|
| `credentialResolution.source` | `runtimeBinding` 为本次应用受保护绑定；`currentCredentials` 为保留当前凭证（初始环境或以前绑定） | 能力响应新增；不含凭证值或身份原值 |
| `credentialResolution.changed` | 本次实际 clientId/token 值是否变化 | 新增；重复应用相同绑定为 false，不表示连接是否变化 |
| `verificationScope="thisToolCall"` | 本次能力请求完成了主程序往返与鉴权 | 新增；导入器子进程也可得到，不能证明它属于当前 Codex 线程 |
| `configurationChanged` | 成功导入回执中，本次是否替换了配置文件 | 新增；等价文件为 false，仍执行内部 stdio 验证和回读 |
| `configurationVerified` | 写入/回读及内部 stdio 能力核对成功 | 原字段保留，不升级为会话证明 |
| 导入回执 `currentSessionVerified` | 导入器不能确认当前线程，因此继续 false | 原字段保留；须由当前对话挂载调用另行确认 |
| `sessionCredentialRefreshPrepared` | 已发布可供活连接使用的受保护绑定 | 原字段保留，不表示已经消费或连接恢复 |
| `sessionCredentialsRefreshed` / `currentSessionAutoImportVerified` | 原“本次应用绑定”的兼容别名 | 保留类型/旧含义并明确弃用；不删除或偷偷改成另一个含义 |
| `connectionVerified` | 本次服务调用成功 | 保留；不是宿主/线程生命周期证明 |

例如同一绑定被第二次应用时，新 `changed=false` 与旧两个 true 可以同时出现；这是兼容窗口中的预期，不是自动恢复成功。调用方应迁移到新事实字段及原对话调用证据。以后删除旧字段须另做兼容版本决定，本阶段不做。

16 个工具名称、完整输入 Schema、协议注解和 API v1 不变。新增本地 Tauri 命令不属于 MCP 工具目录。工具契约基线未重生成或放宽。

## 4. 身份复用与配置安全边界

### 4.1 复用

`reuse_at()` 先核对当前安全存储、所选 ID、Codex 类型、撤销与暂停，再从私有目标配置中读取匹配该 ID 的凭证，复核 Token 归属和授权修订，最后签发新短期包。安全库只存 Token hash，不能据 hash 恢复 Token；本阶段没有扫描不明确宿主的运行时绑定作为秘密来源。

缺文件、缺凭证、身份不匹配、已轮换凭证、非私有 ACL、异常启动项或并发变化均安全停止。不会偷偷新 grant、轮换、解除暂停或增权。签发沿用既有过期包维护；不从旧包恢复授权，也不重放已消费命令。

GUI 默认复用唯一有效身份；多个同名身份必须选择，并显示该身份原权限/范围。新建一次授权后，过期重签也复用该 ID。用户仍可明确选择“新增客户端”或“轮换并接入”，两者与普通修复分开。

### 4.2 配置

保留合法 `default_tools_approval_mode`、`enabled=true`、`required`、启动/工具超时、`enabled_tools/disabled_tools` 以及逐工具审批/输出限制。超时边界参照[官方配置 Schema](https://learn.chatgpt.com/docs/config-schema.json)：毫秒为非负整数；秒值按可转换为 Duration 的非负有限值验证，包含零。不是把任意未知字段自动视为安全。

已有两项凭证通过解析器给出的字节位置精确更新，保留其他字节和混合换行；自身表缺失时局部补入。standard/inline、部分 env、注释、单引号、CRLF 和无变化路径都有合成回归。变更结果重新解析并核对实际写入字节。

禁用、非空 args、非目标 command、不受支持的同表选项、危险 env、非法类型/值继续拒绝；错误说明只输出固定分类，不输出用户键值、路径或秘密。保留独占锁、并发原文比较、密文备份、原子替换、回读和条件回滚。无变化路径先验证私有 ACL，不通过就停止，不以“不重写”保留已放宽的权限。

## 5. 自动验收结果

| 检查 | 结果与依据 |
|---|---|
| 缺陷先复现 | 合法宿主策略拒绝测试先失败；CRLF/表头 span 与无变化 ACL 的新增回归也曾失败，均定位修复。保留开发日志，没有跳过断言或放宽工具契约基线 |
| 前端单元测试 | 22 文件、135 项通过，`npm.cmd test` |
| TypeScript / Vite | `npm.cmd run build` 通过；打包前再由现有流程构建 |
| Rust 全量 | 247 项通过，0 failed/ignored/filtered；单任务、无调试信息、串行测试 |
| 最后 Schema 边界调整后的回归 | 94 项 MCP 库测试 + 2 项真实 stdio 接入测试通过；该最后调整仅涉及超时验证，没有机械重跑无关套件 |
| fmt / 差异检查 | `cargo fmt ... -- --check`、`git diff --check` 通过 |
| 既有接入/偏好恢复浏览器 | 原脚本通过，覆盖复制失败、敏感确认、CLI 独立、诊断及窄窗；全为合成 API |
| 原布局矩阵 | 126 场景通过，更纱黑体、等效 100%/110%/150% 缩放与放大文字；最小动作间距 10 CSS px |
| 新身份复用浏览器 | 12 项流程检查、6 个布局场景通过；含多候选、锁定权限范围、复制重试、过期/重开、缺凭证不自动轮换、明确同 ID 轮换、CLI 切换后正确选中目标身份及暂停保持 |
| 实际 release sidecar 工具契约 | 16 个名称、完整输入 Schema、全部协议注解与已审阅 API v1 基线一致；未重生成基线 |
| release sidecar 匿名边界 | initialize、tools/list、16 个匿名工具调用和 3 次重复调用通过；第 6 次起限流，stdout 仅 JSON，错误均 isError=true，无业务数据 |
| 三程序离线身份 | GUI、MCP、CLI 身份/版本核对及 staged companions 字节一致；不访问正式存储或 IPC |
| NSIS 本地包及解包 | 构建退出 0；仅解出三个程序并验证预期 payload 一致，未执行安装器 |
| 真实原生 GUI | 未执行，不将浏览器模拟算作人工验收 |
| 新构建当前 Codex 会话 | 未执行；没有安装新包，不能用独立连接代替当前会话 |

关键源码测试包括：

- `runtime_credentials::tests::resolution_distinguishes_binding_application_from_actual_credential_change`：首次应用改变、同值重复不变、真实轮换改变，以及输出不含秘密。
- `onboarding::tests::reuse_reissues_for_same_identity_without_changing_store_token_scope_or_config`：三次新包仍是一份授权，原安全库字节/修订号、凭证及范围不变。
- `reuse_never_falls_back_to_grant_or_rotation_for_missing_mismatched_or_stale_credentials`：缺失/错误身份/失效/暂停/撤销都不创建或轮换授权。
- `codex_import` 的 `merge_keeps_safe_host_policy_*`、`merge_adds_only_managed_sections_*`、`merge_rejects_invalid_host_options_*`：兼容、原文及负面边界。
- `unchanged_import_rejects_nonprivate_acl_before_consuming_ticket`、`unchanged_configuration_keeps_private_file_identity_when_stdio_fails`：不跳过私有 ACL、失败不替换等价配置。
- `stdio_failure_restores_*`、`detects_concurrent_config_change_*`：回滚字节/ACL、并发不覆盖、锁释放。
- `tests/mcp_onboarding.rs` 两个真实协议测试：同一活连接的新/重复状态、两个新合成包重复导入后 clientId/权限/scope 不变，授权库不变，等价配置文件身份不变，仍不宣称当前 Codex 会话通过。

测试数据库、安全根、Codex Home、身份与包均为隔离合成数据。本轮未执行真实业务读写或真实授权操作。原有 16 工具的全部业务验收仍属于 Phase 3，此处不提前标为完成。

本地日志集中于 `release/MCP-Phase1-20261028/`：`frontend-full.txt`、`frontend-build-final.txt`、`rust-final.txt`、`rust-schema-final.txt`、`onboarding-schema-final.txt`、`ui-access.txt`、`ui-final.txt`、`ui-spacing.txt`；检查摘要见 `ui-check.json`、`layout-check.json`。原历史布局日志/截图已保留回原位置，本阶段结果另存，不覆盖历史验收依据。

## 6. 本地包、协议与正式环境边界

本地构建与核对已完成：[本地验收安装包](../release/MCP-Phase1-20261028/In-Line_0.5.0-mcp-phase1-local_x64-setup.exe)，4,421,954 字节。SHA-256：`04EFDF7C6EFD3A27AEC76CB03C88F3AD57AC247A4166ED11D3F0242B910483F9`。安装器未执行。

最终构建于 12:40:24 开始、12:52:48 结束，退出 0；最低可用内存 3.68 GiB，最高提交量 80%，没有触发资源保护停止。构建冻结清单中的 818 个文件在打包后逐一回读，均无变化。包内三个程序已离线提取核对：两个 companion 与构建字节完全一致；GUI 仅有现有 Tauri NSIS bundle-type marker 的预期变换。三者均为软件 0.5.0、MCP API 1、schema 14、基线 `4d9bb89258c9870fb70dc360b493b42430edf5c6`，`sourceDirty=true`、`exactCommit=null`。

协议、匿名边界和包核对日志为 `contract.txt`、`anonymous-stdio.txt`、`package-build.json`、`nsis-payload.json`；构建及文件保护结果为 `nsis-final-runtime.json`、`installer.json`、`source-freeze-final.json`、`final-readback.json`，均位于本阶段本地日志目录。正式安装的 GUI、MCP、CLI 三个文件与本阶段开始时的哈希一致。这些结果证明本地包和协议通过对应检查，不构成安装、人工验收或连接恢复证明。

编译工具已核对：Visual Studio Build Tools 14.44.35207 的 x64 `cl.exe/link.exe`、Windows SDK 10.0.26100.0 的 `rc.exe/kernel32.lib`。单构建任务；监控低内存与提交量，仅可终止本轮自己启动的构建子树。开发审查中两次停止过本轮构建以补 ACL/Schema 边界，均未针对正式进程；最终构建结果独立记录。

本轮 12:28:20 当前挂载 `get_capabilities` 成功，请求号 `b7be483e14f45c2e33a9006ce4bb8ccd5906d816a96dd22d851cccea37c2eab9`。身份与 Phase 0 本机比较一致；revision 20，读/完整读/写均 true，部门/类型不限，16 工具，软件 0.5.0/schema 14。返回的是旧安装构建，没有本阶段新字段。这只能说明该时点旧正式环境仍可调用，不能验证本阶段新代码已在当前会话生效。

## 7. 剩余风险、限制与回滚

1. **宿主重载/关闭连接恢复未解决**：未增加 reload 调用、重启或额外代理；Phase 2 继续先核验官方受支持的当前实例入口，再做有界恢复实测。
2. **新包未安装**：原生 GUI→Tauri→真实私有配置的端到端联调、真实客户端回执及新包在当前对话生效均未验证。用户禁止本轮升级/中断，未越过该边界。
3. **配置兼容有明确边界**：未知同表选项仍安全停止；不自动迁移不同安装路径，不支持任意启动参数或环境变量。特殊合法配置形态可补合成 fixture，不根据猜测放行。
4. **秘密来源受限**：复用要求 GUI 进程能解析到对应 Codex Home，并有匹配该身份的私有配置；自定义多 Home 或缺失配置时可能停止。不会因此自动建身份。凭证仍按 D3-A 存于原有位置，不能宣称已经实现“配置无 Token”或抵御任意同用户恶意进程。
5. **旧字段的兼容窗口**：它们保留旧语义，外部消费者须按文档迁移；当前没有证明所有外部调用方已完成迁移。
6. **开发构建溯源**：工作区未提交，仍为 `sourceDirty=true/exactCommit=null`；不能将基线提交或未来用户验收后的提交冒充本包精确来源。

代码回滚只针对本阶段增量，先核对验收清单及私有 `release/MCP-Phase1-20261028/before/` 快照；保留此前 9 个已修改文件及原验收资料，禁止整树 restore/reset/clean。本阶段没有数据库/授权格式迁移，无需回滚正式数据或授权；绝不恢复旧授权备份使撤销身份重新生效。

真实导入失败仍按原文/候选字节比对条件恢复配置与 ACL；第三方发生更新则停止并报告不确定状态，不覆盖其新内容。无变化文件不替换、ACL 不满足时拒绝；不自动重放业务请求或旧一次性指令。

## 8. 阶段门禁

Phase 1 开发与自动验收结果交由用户确认。确认前不提交、推送或进入 Phase 2；本地包不是安装或发布授权。客户端删除继续留在 Phase 4 单独确认。

用户验收后，提交时只精确暂存本阶段文件；此前审批兼容/间距补丁与本阶段有重叠，应先按本报告核对提交范围，不能一条 `git add .` 把所有旧资料一并提交。推送后精确核对远端提交，再经用户阶段安排进入 Phase 2。

Phase 1 到此停止，等待用户验收，不进入 Phase 2。
