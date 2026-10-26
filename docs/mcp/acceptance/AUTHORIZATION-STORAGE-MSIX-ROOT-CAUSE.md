# Codex 侧 AppData 旧副本覆盖：授权读取失败根因

日期：2026-10-26。状态：读取到两份不同物理文件的根因已实测确认；用户随后选择 A，兼容修复已开发并进行隔离验证，详见[交付报告](MSIX-STORAGE-COMPATIBILITY.md)。真实授权迁移、安装及认证成功尚未完成。本报告更正此前将 Codex 侧零记录直接解释为“软件授权未落盘”的判断。

## 对照证据

同一主程序 PID 15508 的原生设置页显示三条 Codex 授权。刷新第三条的“接入诊断”后，GUI 及其新启动的只读诊断子进程 PID 25204 均读到 revision 10、三条客户端，所选客户端存在、类型为 codex、未撤销；GUI 内存与磁盘一致。

紧接着由当前 Codex 工具启动相同安装路径的 sidecar 执行专用只读诊断，仍读到 revision 3、零条客户端。两边构建身份及安装二进制一致，报告的根目录和 canonical 路径字符串也相同，但文件身份不同：

| 读取端 | 文件身份（卷序列号:文件索引） | 字节 | revision | 客户端数 |
| --- | --- | ---: | ---: | ---: |
| GUI 与其诊断子进程 | `be7d6092:005000000007433c` | 1478 | 10 | 3 |
| Codex 工具启动的导入器/诊断进程 | `be7d6092:001b000000046df5` | 518 | 3 | 0 |

使用 `fsutil file queryfilenamebyid` 只读反查 NTFS 文件索引，得到：

- GUI 文件：`C:\Users\KB\AppData\Local\in-line-mcp-security\authorization.dpapi`。
- Codex 读取的文件：`C:\Users\KB\AppData\Local\Packages\OpenAI.Codex_2p2nqsd0c76g0\LocalCache\Local\in-line-mcp-security\authorization.dpapi`。

再以显式 Codex 私有路径查询元数据及文件 ID，确认为同一份 518 字节、2026-10-10 修改的旧文件。未读取或输出该文件内容，没有复制、覆盖、合并或删除任何授权文件。

## 报错链路

Microsoft 的 [AppData 虚拟化规则](https://learn.microsoft.com/en-us/windows/msix/desktop/flexible-virtualization)说明：读取优先命中包私有位置，没有对应文件时再回退普通 AppData。现场元数据检查显示，新接入包在请求路径可见，但不存在于 Codex 的显式私有接入包目录；旧授权文件则存在于私有目录。

结合该规则和实际文件身份，可解释本次链路：

1. 软件已经保存新授权和新接入包，GUI 的完整回读校验通过。
2. 从 Codex 执行命令时，新接入包通过缺失文件回退可见，能完成包校验。
3. 读取 `authorization.dpapi` 时优先命中 Codex 私有旧副本，其中没有新客户端。
4. `codex_import::authorize` 因未找到对应 ID/类型返回 `import_authorization_missing`，尚未修改 Codex 配置。
5. 当前会话实际 `get_capabilities` 返回 `unauthenticated`，没有客户端身份、权限和范围可核对，不能报告当前会话可用。

所以，前述“零条记录”只准确描述 Codex 侧读取视图，不能据此认定软件没有保存授权。新文件已持久化；本次重点是统一不同启动环境下的安全存储访问。

## 已排除或尚有限制的项

- 相同用户 SID、相同二进制、相同路径字符串不能证明实际读到同一个文件；必须核对物理文件身份。
- 包族名查询对本次 PowerShell、GUI 和常驻 sidecar 均返回 `APPMODEL_ERROR_NO_PACKAGE`（15700）。因此不能仅依赖进程有无包身份判断读写是否受影响；本报告依据物理文件反查结果，而非推定所有子进程都携带包身份。
- GUI 自行启动的诊断子进程继承 GUI 的正确视图，因此单看 GUI/该子进程一致还不足以证明 Codex 的执行环境一致；后续回归必须包含真正的 Codex 启动入口。
- 对业务数据库仅额外查询文件身份并反查路径，映射到真实 `AppData\Roaming\in-line\inline.db`，不是 Codex 的私有副本；没有读取事项正文或重新打开备份。此前安装窗口的只读数据核验不因本次授权文件差异自动作废。
- 本轮没有再次导入、授权、撤销、暂停或改动客户端配置。用户新命令仍只执行一次，失败后未重放。

## 已确认的修复方案（真实迁移和安装另行确认）

优先在 In Line 内修复共享安全存储，而非依赖清理某台电脑的 Codex 缓存或修改 Codex/Windows 的安全设置。

- 选用不受 AppData 包私有副本覆盖的用户私有共享位置作为统一访问入口（候选：系统返回的用户 Profile 下 `.in-line/mcp-security`），GUI、导入器、CLI、sidecar 使用同一解析规则；不要求用户手填路径或环境变量。
- 保留当前所有文件；已有有效授权以软件实际读取的权威状态为准，不自动与 Codex 的旧副本合并，不从旧副本恢复已撤销权限。实际本机迁移、安装及中断另行确认。
- DPAPI、当前机器/用户绑定、私有 ACL、独立客户端权限、撤销与暂停等约束不变；MCP 持久状态迁移范围需在实现中列清，不能只迁移授权而遗漏依赖同一根目录的状态。
- 诊断新增真实文件位置/覆盖检测；授权身份不匹配时报告存储视图问题，避免误导用户反复授权。不能仅把 known-folder 返回值换成另一个看似相同的路径字符串后宣称已修复。
- 先做隔离回归及 Codex 侧只读访问探针，再安排新包安装/迁移；后续导入仍使用新的有效提示词，绝不重跑本次旧命令。配置验证与当前会话验证继续独立。

证据文件：`release/Authorization-Diagnostic-regrant-refreshed-20261026.json`、`release/Authorization-Diagnostic-external-msix-20261026.json`、`release/Authorization-Diagnostic-msix-overlay-metadata-20261026.json`、两个 `*-file-path-20261026.txt` 和 `release/Authorization-Diagnostic-msix-process-identities-20261026.json`。仅为脱敏诊断或文件/进程元数据。
