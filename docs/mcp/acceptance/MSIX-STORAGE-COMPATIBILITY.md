# 跨环境安全存储兼容修复

日期：2026-10-26。本报告记录提交前的本地开发检查快照；用户随后已确认提交推送并生成开发 Action Artifact，不创建 Tag/Release。本机迁移和安装仍另行确认。下文 checks JSON 中的 Git/Action 状态及基线 `feat/scheduled-items` / `80db9e7` 均属于该本地检查快照，本地修改构建不作为确定提交的发布二进制。

## 问题与结果

软件实际保存了三条授权；Codex 启动的导入器却读取 AppData 私有旧副本中的零条记录，导致 `import_authorization_missing`。物理文件身份及路径证据见[根因报告](AUTHORIZATION-STORAGE-MSIX-ROOT-CAUSE.md)。

GUI、导入器、sidecar 和 CLI 现统一使用 Windows Profile known folder 下 `.in-line/mcp-security`。生产路径不接受环境覆盖；测试覆盖仅在 debug 构建有效。凭证仍由 DPAPI 加密，校验机器/用户及私有 ACL。文件句柄的实际路径与请求路径不符时返回 `storage_redirected`，不继续读取授权。

发现旧存储而新存储缺失时，主程序不创建空授权库。AI 设置显示迁移说明、源/目标、异常和保存工作确认框，确认后才请求迁移并重启；其他业务界面继续可用。新存储有效时不再依赖旧副本的内容或可读性。

## 迁移边界

- 唯一入口为本机主窗口命令，要求明确确认及当前源授权文件身份；MCP/CLI 不能发起迁移。
- 来源必须为原生旧目录，拒绝重定向副本、重解析路径、不符的私有 ACL、损坏或机器/用户不匹配的授权。
- 迁移完整保留 `authorization.dpapi`、`saved-queries.json`、`diagnostics`、`jobs`、`changes`、`onboarding`、`cli-onboarding`、`cli-profiles`、`runtime-credentials`。不读取业务库、Codex 配置或备份，不合并旧隔离副本，不新增或恢复授权。
- 现有独立权限、范围、敏感权限确认、全局开关、暂停、撤销、管道秘密及程序绑定均保持。复制运行时绑定不绕过进程身份和构建校验；旧临时包仍按原到期和一次性规则处理，不重试现场旧命令。
- 来源授权加读锁；复制前后比对清单、文件身份、大小和修改时间，逐文件验证 SHA-256，并校验完整授权一致性。目标使用私有暂存目录，全部通过后原子重命名；已存在的完整或不完整目标均拒绝覆盖。
- 单文件最多 32 MiB、总量 512 MiB、8192 项、6 层目录；超限或未知顶层项停止。失败保留源文件及私有暂存，不自动重试。完成回执不表示客户端配置或当前会话验证通过。
- Windows 权限和文件元数据接口使用扩展路径，覆盖迁移暂存路径超过 260 字符的情况；原失败由回归测试真实触发，未通过缩短测试路径绕开。

## 验证与交付

自动检查和包核验结果记录于 [checks JSON](MSIX-STORAGE-COMPATIBILITY-checks.json)。Rust 使用单编译任务和单测试线程并监测提交内存；测试临时目录位于 D 盘隔离位置，避免 AppData 本身的重定向干扰安全断言。

最终结果：235 项 Rust、133 项前端、前端构建、Rust 格式和差异检查、两组实际 React/Edge 隔离回归通过；debug 与解包 release sidecar 的 16 工具完整契约通过。构建期间提交内存保护未触发。

隔离用例验证权限/范围、撤销、暂停和各类持久文件完整保留；未确认不写入，来源变化、损坏、未知文件、目标已存在均拒绝；旧副本保持不变，新目录可独立读取。实际 sidecar 子进程在当前 Codex 启动环境读取 Profile 下的独立合成目录，与父测试进程读到同一物理文件；AppData 合成探针另外核对重定向检测。测试目录与真实 `.in-line/mcp-security` 完全分离。

本次新建的 AppData 合成文件没有触发重定向，不将它当成真实旧副本的重现。对现场旧授权另外仅申请 `FILE_READ_ATTRIBUTES`，普通路径及扩展路径均反查到同一 Codex 私有旧文件，确认长路径处理没有绕开现场覆盖现象；未读取凭证字节或改写文件，证据为 `release/MSIX-Storage-extended-path-metadata-probe.json`。

React/Edge 隔离验证迁移确认、失败后重新确认、无自动重试、隔离来源阻止迁移，以及新根就绪后显示接入控件；原授权弹窗、剪贴板重试、诊断和字体/字号恢复回归继续通过。所有界面调用均为合成 API，没有操作生产授权。

本地 NSIS 仅生成并解包核验三程序身份、暂存及载荷 hash 和 16 工具契约；没有执行该安装包。具体路径和摘要见 checks JSON。

本地包：`release/MSIX-Storage-local-20261026/In-Line_0.5.0-msix-storage-local_x64-setup.exe`，4,372,870 字节，SHA-256 `0FE80DE7BFC3EF90027CAF7A4B065B7D7EB5F3D8395F88A79C71D06FDB423304`。三程序均为 0.5.0/schema 14/API 1，基线 `80db9e7`、dirty=true、exactCommit=null，不冒充已提交构建。

解包 release sidecar 的只读诊断在当前 Codex 环境报告根目录 `C:\Users\KB\.in-line\mcp-security`，授权文件缺失且探针前后均未创建目录；确认生产编译路径使用新的共享位置。该诊断没有执行导入、读取旧凭证或验证当前挂载会话。

## 尚未完成的实机门禁

当前安装仍为此前诊断版本。本轮没有创建生产新存储、迁移三条真实授权、安装/重启软件、删除或合并 Codex 旧文件、打开既有接入包或修改客户端配置。

之后的本机窗口需用户单独批准：先保留回退与现场摘要，再安装、从原生 GUI 确认迁移，核对三条授权及设置/数据保留；生成全新的接入提示词，配置导入与当前挂载 `get_capabilities` 分别核对。当前会话最近实测仍为 `unauthenticated`，两项验证均不能标为成功；隔离 sidecar 和包检查不能替代当前会话认证。
