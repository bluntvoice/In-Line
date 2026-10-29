# P2-C 安装器安全性优化验收报告

日期：2026-10-29（北京时间）。状态：代码、自动化及隔离验证、Actions 安装包核对完成，提交用户验收；已停止开发。P2-B 暂缓，P2-C 尚未获用户最终验收，未正式提交推送开发分支。

## 1. 根因与本阶段决策

原 `installer-hooks.nsh` 使用两次 `taskkill /F /T /IM in-line-mcp.exe`，不校验安装路径、用户、PID 或创建时间；固定等待不能证明文件已释放，宿主也可能重新启动 sidecar。Tauri 默认 `CheckIfAppIsRunning` 还会按名称处理 GUI；维护页某些路径先调用旧卸载器，再逐个覆盖程序，存在绕过新钩子和半组替换的风险。

GUI 的普通 CloseRequested 会阻止关闭并隐藏窗口（`lib.rs` 的现有行为），不能把 WM_CLOSE 当作可靠退出。MCP 也没有供安装器独立使用、能保证办理结果和连接状态的退出契约。因此本阶段采用明确规则：**不自动终止任何进程；目标文件不能取得独占访问时停止，并报告准确占用者。** 这符合无法安全退出即停止的要求，但没有解决严格自动恢复目标。

只读 Restart Manager 查询注册的五项安装文件，返回 PID 和创建时间；通过同一只读进程句柄核对创建时间、可执行文件路径和 Windows 用户 SID。这里的用户 SID 是操作系统身份，不是 MCP 授权凭证。进程身份只用于说明原因，不授予终止权限；真正的操作依据是目标文件独占句柄，避免识别后退出或 PID 复用影响其他进程。Microsoft 对 [RM_UNIQUE_PROCESS](https://learn.microsoft.com/en-us/windows/win32/api/restartmanager/ns-restartmanager-rm_unique_process) 的定义包含 PID 与启动时间；[CreateFileW](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilew) 的零共享模式用于排除并发访问。

## 2. 文件组与安装流程

管理范围固定为目标安装目录中的 `in-line.exe`、`in-line-mcp.exe`、`in-line-cli.exe`、`WebView2Loader.dll`、`uninstall.exe`。不递归管理未知文件，不读取数据库、授权库、偏好或 MCP 配置内容。

1. 安装器将完整载荷及新卸载器准备在其私有 plugins 目录。已有受管文件须与本用户注册安装位置匹配；无法核对的旧 MSI/便携目录不覆盖，不执行旧卸载器。
2. 有限 PowerShell/C# 辅助程序使用现有 Windows PowerShell 与系统 .NET。核对本地 NTFS、绝对路径、重解析点及单硬链接；持有安装目录句柄和互斥文件锁。
3. **所有目标文件都取得 read/write/delete、share NONE 句柄后，才修改任何已有字节。** 占用、拒绝访问或身份不明均停止。即使识别后状态改变，文件共享规则仍保护实际文件；不回退名称匹配或终止宿主。
4. 备份和 SHA-256 校验后写入新组，刷盘并逐一核对。正常失败使用仍持有的句柄恢复原字节、时间及原有 ACL；新建文件撤回。NSIS 注册信息写入失败会停止并调用程序组回滚。
5. 恢复事务目录保留至 NSIS 完成。三个入口在事务目录存在时拒绝业务启动，防止意外中断后运行混合程序；离线构建信息及 CLI 帮助/版本契约保留。
6. 卸载先核对注册目录，再对同一文件组预检、备份和标记删除；失败撤销删除标记并恢复整组。卸载不显示删除用户数据的选项，不删除用户数据目录、自启动偏好或未知安装目录文件。

未完成或被修改的日志、未知锁内容、并发改变的程序均停止并保留证据；不盲目重放恢复。正常回滚与操作成功分别有校验，不能以退出码或固定等待冒充恢复完成。

## 3. 修改范围与关键代码

[Tauri 官方安装器扩展说明](https://v2.tauri.app/distribute/windows-installer/)提供 hooks 与自定义模板。hooks 无法替换默认 GUI 名称处理、旧卸载器路径和逐文件 File 命令，因此固定采用 CLI 2.11.4 的上游模板作限定修改。未安装新服务、没有新增常驻代理或已安装的第四程序。

| 文件 | 修改目的 |
|---|---|
| `src-tauri/windows/installer.nsi`（新） | 固定上游模板；安全就地修复/安装；去除名称退出及旧卸载器调用；私有载荷、所有权核查、失败/取消回滚和数据保留 |
| `src-tauri/windows/installer-hooks.nsh` | 移除 taskkill，接入有限 guard；保留原中文字体及单选项高度修复 |
| `src-tauri/windows/InstallGuard.cs`（新） | 文件身份与全组句柄、只读占用者核对、备份/校验/回滚、卸载删除标记、恢复日志保护 |
| `src-tauri/windows/install-guard.ps1`（新） | Windows PowerShell 包装；任何编译/策略/运行失败都返回失败，不走终止回退 |
| `src-tauri/windows/README.md`（新） | 上游来源、模板升级审查与恢复规则 |
| `src-tauri/tauri.conf.json` | 选择定制模板；版本、安装模式、外部程序和资源范围不变 |
| `src-tauri/src/install_gate.rs`（新） | 只读未完成事务启动屏障及回归测试 |
| `src-tauri/src/main.rs` | GUI 屏障；未完成时显示本地错误，离线 build-info 保留 |
| `src-tauri/src/bin/in-line-mcp.rs` | 屏障在初始化/授权存储前停止，stderr 提示，stdout 不受污染 |
| `src-tauri/src/cli/mod.rs` | 业务 IPC 前停止并沿用结构化 incompatible 错误；帮助/版本保留 |
| `src-tauri/src/lib.rs` | 仅导出屏障模块，无业务或授权修改 |
| `scripts/verify-installer-guard.ps1`（新） | 真实 Windows 文件句柄、ACL、合成数据及信号退出的隔离进程测试 |
| `scripts/verify-installer-safety.mjs`（新） | 构建前限定版本、文件组与禁止退出路径，检查三个启动屏障和失败回调 |
| `scripts/prepare-mcp-sidecar.mjs` | 增加上述构建边界检查 |
| `scripts/verify-installer-package.ps1` | 按私有载荷目录准确提取三个程序，保留逐字节和构建身份校验 |
| `.github/workflows/dev-build.yml` | 在现有开发 workflow 增加 x86/x64 隔离 guard 测试及结果 Artifact |
| `README.md`、`CHANGELOG.md`、`RELEASE_NOTES.md` | 开发状态及未安装/未发布边界 |
| `docs/prd/app-update.md`、`docs/prd/mcp.md` | 新安装规则、P2-A 认可、P2-B 暂缓与 P2-D 分项门禁 |
| 本报告、`MCP-PHASE2C-20261029-checks.json`（新） | 脱敏验收证据及剩余风险 |

关键位置：`InstallGuard.cs::Inspect()` 和 `Run()`、`installer.nsi` 的 Install/Uninstall 两个 Section 与 `InLineOnAbort` 回调、`install_gate.rs::ready_at()`。未改数据库 schema、授权持久格式、16 工具定义/输入 Schema、依赖或版本文件，未开发客户端删除。

## 4. 自动化与隔离验证

| 验证 | 结果 |
|---|---|
| 本地前端测试 | 22 文件、135 项通过 |
| 本地 Rust 全量 | 248 项通过，0 failed/ignored/filtered；新增 1 项启动屏障测试 |
| 本地 x64 Windows PowerShell guard | ACL 兼容修复前 30 项通过；最终结果另以 Actions x64 核对 |
| 本地 x86 guard | 最终源码 31 项通过；含 ACL 兼容正向和权限变化反例 |
| Actions x86 / x64 guard | 最终源码各 31 项通过；真实 Windows Server runner |
| 缓存程序的 NSIS 编译检查 | 模板、安装/卸载 guard 和回调编译通过；使用旧缓存程序，不作为新包验收 |
| 真实复制程序的启动屏障 | 隔离 debug MCP 退出 78 且 stdout 为空；CLI 退出 6、结构化 incompatible；三程序离线查询通过；未启动 GUI 原生错误窗口 |
| 最终 Actions 全回归/NSIS/载荷 | CI 与开发安装包任务均 success；135 前端、248 Rust、x86/x64 各 31 guard；默认 release、16 工具契约、匿名拒绝、三程序精确 Commit 及包内字节核验通过 |
| 下载包独立回读 | ZIP 摘要与 GitHub digest 相同；安装器摘要匹配；提取三程序字节/构建记录、保护脚本及 WebView2 DLL 匹配；未执行本机安装器或提取程序 |
| 正式安装及原生 GUI/卸载 | 未执行，须另行授权 |

31 项 guard 覆盖：新安装无运行进程、完整程序组；安装和卸载在第 1～5 项故障后全部恢复；新装失败无部分程序；共享占用、拒绝写/删除 ACL；并发安装锁、未知锁、缺失载荷和完成日志；不同新旧文件长度的组恢复；元数据阶段程序回滚及 ACL/时间保持；卸载后重建程序组并恢复受保护 DACL；卸载保留四类合成历史数据；并发修改拒绝回滚/完成；未知恢复目录、硬链接和 junction；真实同名目标/异目录进程的保护；识别后目标自主退出，再次安装成功；Windows AI 位转换的正向与保护位、SID、权限、ACE 标志/增加/顺序变化的反例。

同名测试进程的可执行文件为专用无业务 console fixture，只响应其私有退出文件。目标运行时安装和卸载都失败，目标及异目录进程的 PID/创建时间不变，两个目录的字节不变；仅由测试自己的退出文件让 fixture 正常退出。不存在强制结束测试或正式进程的 API。独立证据 `non-target-proof.json` 记录两个路径、PID、创建时间、Windows 用户匹配结果和每项字节回读；目标自主退出后重试成功，异目录实例继续运行。

测试目录中的 inline.db、authorization.dpapi、preferences.json、config.toml 均为合成文本，不是真实库或凭证。其他用户真实会话未建立；不同用户保护依据无终止 API、操作系统访问控制和拒绝访问测试，不能称为已执行跨用户原生测试。

日志位于 `release/MCP-Phase2C-20261029/`：`frontend-tests.txt`、`rust-full.txt`、`guard-x86-r7-corrected/results.json`、各次 x86/x64历史结果、`launch-barrier-results.json`、`nsis-syntax-r3.txt`。首次本地标准 release 因资源保护停止，记录保留；用户随后要求 Actions，未改变默认 LTO/优化参数或系统资源设置。

### Windows Server ACL 兼容修复

Actions 曾在卸载后重建文件时安全停止：原 DACL 为 `D:(A;ID;FA;;;BA)(A;ID;FA;;;SY)(A;ID;0x1200a9;;;BU)`，回读仅增加 `AI`，所有 ACE 均未改变。[Microsoft 的继承转换说明](https://learn.microsoft.com/en-us/windows/win32/secauthz/automatic-propagation-of-inheritable-aces)确认受支持的 SetSecurityInfo 会把转换到当前继承模型的描述符标记为 AUTO_INHERITED。

现在对比 DACL 二进制及控制位，只允许该 AI 位从 false 到 true 的原生转换，不允许反向变化；保护位、每个 ACE 的 SID、权限、继承标志、条数及顺序仍严格一致。原始备份描述符不改写。增加相应反例验证，不跳过或删减权限校验；没有使用不适用于文件系统对象的 SetKernelObjectSecurity 绕过该行为。失败记录见 `acl-compat-red.txt`。本地曾因 PowerShell 数组构造将一个反例变成未修改的输入而失败，已修正输入括号，原拒绝断言保留。

## 5. Actions 临时构建与源码对应

用户明确批准仅为构建创建并推送临时快照。工作分支仍是 `feat/scheduled-items`，HEAD 保持 `5cffe059dfdaa2c153c7f1266c4ca9fa3f7c883b`，实际暂存区为空。临时分支为 `codex/p2c-build-20261029`；最终构建目标为 `497b61fcb92c2d573d60adbaa0d223612fb216aa`。

临时快照只含 16 个与编译相关的安装器/启动屏障/测试/workflow 文件，排除此前未跟踪资料。GitHub 连接工具没有首次 dispatch 能力，故临时快照的 workflow 额外加了**只匹配该临时分支的 push 触发**，不修改工作区 workflow 的触发方式，不创建 Release/Tag 或推进开发分支。八个快照按实际修正快进保留；中间构建有被替代/取消及 ACL 校验失败记录，仅最终源码的任务作为验收依据。

`31adf23` 已通过 guard、全回归、默认 release 编译及 MCP 契约检查，但提取脚本仍查找顶层程序，未找到新模板私有目录内的载荷，Artifact 上传因此被阻止。`497b61f` 仅修正提取脚本的准确目录；不更改产品源码、权限校验或比对断言。本地缓存包已复现顶层提取为零，并验证准确目录提取三项成功；缓存包不能作为最终新包证明。

最终任务：[CI 40812225378](https://github.com/bluntvoice/In-Line/actions/runs/40812225378)、[开发安装包 40812225368](https://github.com/bluntvoice/In-Line/actions/runs/40812225368)，均为 success；[Artifact 12871357406](https://github.com/bluntvoice/In-Line/actions/runs/40812225368/artifacts/12871357406)，2026-11-12 11:46（北京时间）到期。

- Artifact ZIP：4,412,265 字节，SHA-256 `4b3c6aa20145142d1c7046bf79c28ba65d8dacf06bb9522c17d81e6c4469887e`。
- NSIS `In Line_0.5.0_x64-setup.exe`：4,427,509 字节，SHA-256 `0f72b1ed5ee65895b57507e1e556579bfcfa3d9f1ab2c0cd930473a1d41a6d32`。
- 三程序均为软件 0.5.0、MCP API 1、数据库 schema 14，`exactCommit=497b61fcb92c2d573d60adbaa0d223612fb216aa`、`sourceDirty=false`。MCP/CLI 与构建源逐字节一致；GUI 仅有 Tauri 官方 NSIS bundle marker 转换，CI 重建预期文件逐字节比较，本地逆转换后整文件摘要也匹配唯一候选。

下载包仅解压并回读，本机没有执行安装器或这些新程序。脚本与快照按 Git 换行规范比较一致，WebView2Loader.dll 与既有资源字节一致。记录见 `actions-final-runs.json`、`actions-ci-evidence.json`、`actions-dev-evidence.json`、`download-verification.json` 和 `actions-final/` 下完整 Artifact 原件。

工作区共 14 个已跟踪文件增量、9 个新文件；基线 822 项变为 831 项，0 删除、0 范围外变动。原有 12 项未跟踪历史资料均未改变。16 项构建源码与最终快照对应；实际暂存区为空，开发分支和 main 的本地/远端状态未推进。本轮最终文件摘要保存在 `final-stage-files.json`，便于后续提交前再次核对。正式 P2-C 提交与推送仍等用户验收，不得把临时快照推送等同于 P2-C 最终提交验收。

## 6. 回滚与剩余验收

普通可捕获失败已隔离验证：全组锁定后保存并校验原字节，写入/卸载失败时在同一组句柄中恢复；NSIS 后续失败和取消调用显式回滚。当前字节不再匹配预期旧/新摘要时拒绝覆盖，保留 `.in-line-install-transaction` 和恢复备份，启动屏障继续生效。未知日志或恢复失败不自动删除、重放或宣称回滚成功。

本实现对程序文件组提供备份/校验/回滚；NSIS 的注册表、快捷方式及 UI 不是跨资源原子事务。相关错误已中止并尝试程序组恢复，注册表/快捷方式的完整恢复、用户取消、旧卸载器自复制行为和系统 WebView2 组件边界须在获准的隔离原生环境逐项实测，不能用 guard 单测代替。已验证重建文件的 DACL 与时间；不宣称重建文件时 owner/group/SACL 的完整还原已获验收。

掉电、存储硬件故障、企业 PowerShell/AppLocker 限制、杀软、实际第二用户、真实安装/升级/卸载及原生 GUI 提示未验。突发中断依赖保留的恢复资料和启动屏障停止使用；旧版本程序没有新屏障，不能宣称对所有历史版本提供硬件级原子更新。应从同一核验源码/安装包取得 guard，由维护者确认原文、摘要和目录后执行显式恢复；不得盲目删除标记。

补丁只作用于新安装器，不会改写已分发的旧安装器/旧卸载器。无法安全核验的旧安装或占用场景停止，部分迁移、长路径别名、非 NTFS 和受限企业环境的可安装性仍需真实验收。没有以降低安全校验来放行这些环境。

正式环境的数据库、授权、偏好、MCP 配置及工作事项未修改；当前正式三个程序未替换，Codex/MCP 进程未重启/终止。最终保护回读结果见脱敏检查文件；原会话能力查询不用于证明新包或自动恢复通过。

16 个工具的协议兼容及匿名拒绝检查不等于完整业务工具矩阵。原生 GUI 联调、新构建在当前 Codex 会话接入、完整真实业务矩阵继续待验，未改为通过；正式发布前仍须对届时最终源码执行完整回归，并按后续增量补充必要测试。

## 7. P2-B / P2-D 与停止点

- **P2-B 暂缓**：官方方法存在不代表当前实例可达。不新增自动重载、常驻中转、私有 IPC、句柄注入或其他接管方式。只有发现新的官方能力才另提方案确认。
- **P2-D 分项保留**：首次接入、同活连接刷新、已关闭连接恢复、升级恢复分别验收。前两项可按后续授权安排，严格的已关闭/升级自动恢复仍依赖受支持入口，不因安装器安全改善而通过。
- **产品目标保留**：无需用户手动重启或重连仍为既定目标，当前安全停止/提示安排退出占用程序不构成替代达标。

P2-C 交由用户验收后才正式提交推送；需要真实安装验证时另行申请。未进入 Phase 3，未将 Phase 2 标为完成。
