# P8 首次授权与安全导入门禁修复

日期：2026-10-15。基线`f9cf36fbc9ae6fc3f73471f881d92a5c86ea2db8`，分支`feat/scheduled-items`，开始时工作区干净。用户选择A，授权修复首次授权与验收门禁，保留零额外操作目标。仍在P8，不代表用户验收通过，不进入P9或Release。

## 已核实原因

- 当前安装的sidecar摘要与P8实际构建一致，已运行并能返回16工具声明；本机授权库客户端数为0，全局未暂停；全局常规读取开，完整读取/写入关。
- 当前Codex配置启用`mcp_servers.in_line`，路径指向已安装sidecar、无启动参数；没有设置或转发两项认证环境凭证。只输出存在性/有效性判断，没有输出Token、IPC密钥或完整授权/配置内容。
- sidecar在进入主程序管道前先验证凭证，故当前unauthenticated不能用于判断当前会话重载能力。
- `CODEX_CURRENT_SESSION_VERIFIED=false`原来同时阻止软件内首次授权和生产版导入。首次验证所需的授权/配置被验证结果门禁挡住，形成前置流程阻断。
- 当前公开`codex app-server daemon version`仍无法连接现有控制套接字，错误OS10050。没有启动新app-server或把另一个实例当作当前会话；官方存在reload协议方法，不等于本会话存在可调用通道。

## 本次修复范围

1. 增加独立`authorizationAvailable`事实；Codex授权适配器已具备安全导入能力，可先授权/生成一次性接入包。`available`及当前会话验证标志保持false，不把准备授权等同于会话接入成功。
2. 主窗口准备/明确轮换、生产版导入器统一使用授权适配器门禁，仍拒绝未知客户端。主窗口限制、用户勾选权限、首次敏感提示、DPAPI/ACL/机器及用户绑定、十分钟单次包、配置冲突/回退、独立身份/撤销/暂停等检查保留。
3. 前端按钮依据授权能力启用；配置导入及会话连接分别说明。复制失败保留原包，再次复制不重复授权。导入器仍只验证配置，`currentSessionVerified=false`保持，不能假称当前Codex可用。
4. 软件内接入说明从实际`TOOLS`常量生成完整16工具及数量，去掉“三工具/尚无写入/首次授权入口关闭”的过期文字。
5. 不读取业务库、不创建真实客户端、不修改Codex配置、不安装新程序。后续实机步骤须使用软件内由用户发起的授权，不能由AI绕过软件自建生产授权。

## 实测与首次失败

- 新前端门禁回归先失败：旧实现将授权能力误绑会话验证；`P8-onboarding-first-authorization.txt`保留。修复后定向通过，新增不可用/繁忙/旧预设兼容/显式拒绝负例。
- Rust新用例确认可在未宣称会话验证时准备第一个独立客户端，默认不授予完整读取或写入；原生产自动验证门禁仍拒绝未实测与未知客户端。
- 真实导入器/合成授权IPC已有集成测试继续验证独立身份、有效权限交集、限域、凭证不回显、配置保留/单次消费及撤销；不冒称当前会话验证。
- 浏览器首次无法加载模块，增加错误输出确认bare React导入失败；目录已迁移到D盘，而Node cwd仍经C盘联接。仅设置Vite根或更换缓存仍遇到依赖优化错误；统一测试进程cwd为canonical路径后通过。首次及诊断/根路径/新缓存日志均保留，未修改产品依赖或放宽断言。
- `P8-onboarding-ui-final.txt`通过：实际授权/维护组件，风险取消/确认一次、首次授权可用、再次复制只创建一次、默认客户端权限不增权；P1不可静音、标待验证不假恢复、导出/审计清理取消零写保护继续通过。
- 最终全量及本次新包的验证结果见文末，不用此前P8包代替本次修复产物。

## 已确认的凭证刷新路径

用户已确认修复首次授权门禁，随后对凭证刷新技术取舍再次选A：允许已运行sidecar安全刷新用户已授权凭证，替代对Codex原生配置重载的依赖；要求绑定所选客户端、独立授权并实测。

- A：允许已运行的In Line sidecar在用户已授权后，绑定所选客户端，从本机受保护存储安全刷新凭证，以替代对Codex原生配置重载的依赖；必须实测，不暴露凭证。
- B：只使用Codex原生配置重载；先完成授权/安全导入，当前会话重载继续保留阻断。

因此实施A，B仅为选项历史。没有将第一个A误当成第二个选择，也没有把等待时间当作同意。一次性包导入仍返回配置已验证/当前会话待验证；只有当前会话真实调用能力工具、核对身份/有效权限和范围后才能报告当前会话接入成功，不要求手填Token或重连。

### 已实现的绑定与刷新

- `mcp/runtime_credentials.rs`只接受软件内明确授权并通过一次性包和配置验证后的凭证。目标为执行导入的Codex实例：进程ID、内核创建时间、可信本机Codex路径，以及sidecar路径/构建二进制摘要共同绑定；主机退出或PID重用不继承。
- 导入器可从PowerShell祖先进程定位所选Codex。运行态sidecar仅在**直接父进程**为该Codex时使用刷新；导入器另起的配置验证子进程不能冒充原MCP会话。缺少目标、配置有多个In-Line服务器条目或凭证不完整时不猜测，保留待验证和脱敏原因。
- 受保护安全域`runtime-credentials`内只写DPAPI密文，32KiB/条、1MiB/64条上限，当前用户私有ACL；不进入业务备份。原配置身份只接收本身份轮换或明确针对该前序身份的接入，其他已配置客户端不自动继承。损坏/伪造/未知格式拒绝且不覆盖原文件。
- 每次调用仍由IPC独立复验客户端Token、暂停/撤销、有效权限交集、范围和授权修订；刷新不创建授权、不提升全局权限、不自动执行业务写入或重放请求。新环境凭证不会被旧刷新记录覆盖，旧记录只有仍匹配当前授权时才被采用。
- 导入报告新增`sessionCredentialRefreshPrepared`和脱敏错误字段，`currentSessionVerified`仍false。只有真实Codex父进程的原连接在刷新后完成受保护调用，其能力响应才报告`sessionCredentialsRefreshed/currentSessionAutoImportVerified=true`；仍要求通过当前挂载工具验证，不能拿合成测试冒称桌面验收。
- 用户级Codex配置目录迁移为联接时先解析所选目录，再对内部文件保留重解析/ACL/并发保护。未放宽已安装sidecar路径验证。

### 真实连接回归

`existing_stdio_connection_refreshes_authorized_identity_without_restarting`保留同一RMCP客户端/stdio连接：先未认证，再执行用户授权包导入，原连接认证并核对权限；暂停拒绝、轮换后旧凭证拒绝、再次导入后同一连接恢复、撤销立即拒绝。配置验证子进程与原连接分开，报告仍不提前标当前会话成功。

首次失败在轮换处，根因为旧刷新记录覆盖了新环境Token；修复采用当前安全库验证候选Token后，两条真实导入器测试通过，日志`P8-onboarding-existing-session-first/final.txt`均保留。进程复用/不同二进制/其他身份/不完整凭证及损坏文件负例在模块测试中保留。当前桌面程序尚未升级，本会话仍待真实软件内授权，不将这些隔离测试称为当前会话验收通过。

## 2026-10-16 最终开发验证及交付

| 检查 | 结果与证据（acceptance/evidence） |
| --- | --- |
| 全量Rust | 177通过，168库+2sidecar+7集成，零忽略；`P8-onboarding-runtime-full.txt` |
| 前端 | 119项/21文件通过；`P8-onboarding-frontend.txt` |
| 实际组件浏览器 | 首次授权与重复复制、默认权限、风险提示及维护保护通过；`P8-onboarding-ui-final.txt` |
| fmt及debug协议 | fmt通过；16工具完整契约/注解、匿名拒绝及限流通过；`P8-onboarding-format.txt`、`P8-onboarding-contract.txt` |
| TS/Vite及NSIS | 成功，sidecar 5m38、主程序7m39；`P8-onboarding-nsis-first.txt` |
| 构建输入 | 两份sidecar分别通过实际16工具契约及匿名权限协议检查；`P8-onboarding-package-readback.txt` |
| 源码 | 165份冻结源码摘要保持不变；保留包记录`P8-onboarding-retained-package.json` |
| 文档与Actions | 原3份输入及144项/13字段校验、版本/链接及Actions运行时策略通过；`P8-onboarding-documents.txt`、`P8-onboarding-actions-policy.txt` |
| 生产状态 | 已安装两exe、独立授权库及Codex配置摘要与排查前相同；`P8-onboarding-protected-readback.json` |

最终全量Rust从真实D盘仓库执行。此前C盘联接路径被安全测试按原规则拒绝；首次日志`P8-onboarding-rust-full-first.txt`和旧名`P8-onboarding-rust-final.txt`均为失败历史，原断言保留，未削弱路径安全规则或跳过测试。

本地包：`release/In-Line-P8-onboarding-local-20261016.exe`，4,229,582字节；SHA256 `02BA681F51150A19DF6C15EBEBD872E6DEEF7FD34084654FB48FD2116AA85CE4`。构建基线`f9cf36fbc9ae6fc3f73471f881d92a5c86ea2db8`，本地dirty源码，`exactCommit=null`；不得将后续提交SHA冒充二进制内嵌SHA。旧P8包保留原样。

尚未安装、尚未验证真实旧数据升级及用户数据保留、尚未在实际软件内由用户授权、尚未通过当前挂载`get_capabilities`认证。当前旧程序不能使用新刷新模块；首次部署需先更换程序。安装后用户按正常流程选择Codex与权限、授权并复制提示词给当前AI，再执行一次性导入并核对当前挂载调用的身份、权限及范围。禁止另起服务代替当前会话，不要求用户填写凭证、手动重连或重启Codex。P1–P8仍待统一验收，不进入P9或创建Tag/Release。
