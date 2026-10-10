# Codex 零额外操作接入：当前会话实机验证

日期：2026-10-10。用户已选择严格无需额外操作，不接受手动填写或一次重连的降级方案。状态：**当前Codex未通过自动重载门禁，一键接入功能尚未实现或开放；P1仍待验收。**

最新环境：用户已更新并重启Codex，当前桌面26.1007.2314.0，活动CLI0.162.0-alpha.17.2，App Server PID23556、父进程32712。旧版本探针记录保留为历史证据。新版默认控制套接字仍不存在，没有显式监听参数/TCP监听；实际app-server proxy退出1/OS10050且无协议响应。mcp CLI仍无reload命令，当前应用工具未暴露此方法。真实能力调用b79d0b59c6b341430b9eafc880c089332363d14d6a5817e5da51bdd5a1109f28返回unauthenticated，只证明现有MCP可响应，不代表导入/重载通过。此次没有重做新版临时文件写入探针，不能把旧版文件更新结论当作新版实测；新版公开连接通道尚未通过。原配置摘要未变化，In-Line仍为0.5.0。更新曾作为排查选项提出，未经验证不能称为修复方案。见[P1-codex-updated-channel-result.json](evidence/P1-codex-updated-channel-result.json)。

## 环境与标准

- Codex桌面应用26.1002.7124.0，已运行App Server对应CLI0.162.0-alpha.2。
- In-Line已安装P1测试版0.5.0，当前客户端没有ID/Token环境配置；匿名能力查询返回unauthenticated是既有门禁的正确行为。
- 成功标准：用户一次粘贴导入提示词后，配置自动导入并由当前运行的Codex加载；当前会话真实调用能力查询进入新配置并验证已授权身份。新起App Server或stdio子进程的成功不能代替当前会话结果。

## 执行与结果

1. 当前工具列表存在In-Line能力查询工具，没有面向当前实例的MCP配置重载工具。
2. 官方文档有`config/mcpServer/reload`；这是App Server协议方法，不能因为文档存在就声称当前AI会话能直接调用。[官方文档](https://learn.chatgpt.com/docs/app-server)
3. 读取当前安装的应用代码，确认实际程序中有此方法调用；没有修改程序文件或通过内部IPC/渲染器注入请求。源代码线索没有替代行为验证。
4. 公开CLI的`app-server daemon version`尝试连接默认控制套接字，退出1、OS10050。未启动新daemon，未重启或结束Codex。该失败不证明所有版本都无法连接。
5. 私有ACL目录保存原配置。校验原服务器没有凭据后，只临时替换`in_line`的command/args为透明stdio转发探针；完整TOML解析、与原结构逐项比较，只允许这两个字段变化；原ACL保留。没有创建授权或处理Token。
6. 探针独立自测成功：实际启动已安装sidecar，JSON-RPC握手/能力查询/匿名拒绝正确，生成独立nonce及日志。它只证明探针可用，不算当前会话重载成功。
7. 配置真实写入后，从当前会话实际调用get_capabilities两次，requestId分别为`df8a2d37bc4cf3f16b8c301248ee54bde28d97a26418a6cb3eafd6a1d34367c3`、`197cb8f361266cf903068efb87af8947030e69da4dec292c757d546a1ba46971`。两次均未进入探针，未生成当前桌面探针启动/调用日志，仍收到原连接的unauthenticated结果。因此文件更新路径未让当前会话自动重载；错误码本身不是重载失败的判据。
8. 对原配置与试验配置做并发变化校验后，原子恢复原文件，并校验字节摘要一致。没有保留探针配置，没有结束当前MCP或Codex，没有访问业务数据库。

结构化记录见[结果](evidence/P1-codex-reload-result.json)，私有回退及自测目录位置见[探针记录](evidence/P1-codex-reload-probe-location.txt)。原配置备份和运行代码不进入Git；仓库只保存无凭据的验证结论。

## 准备过程首次失败（工具输出转录）

- 应用归档代码扫描首次输出遇GBK UnicodeEncodeError；改为UTF-8和转义文本输出后读取成功。无应用或配置变更。
- TOML候选第一次用正则替换字符串时Windows反斜杠被解释，解析校验拒绝；修正为函数返回替换内容。失败候选未写入真实配置。
- PowerShell调用三参数File.Replace，null备份路径被转换为空路径，报`The path is empty`；真实配置仍保持原摘要，改用同目录File.Move的覆盖重命名。
- 候选摘要第一次按内存LF计算，Windows写入CRLF导致校验拒绝；实际候选与暂存文件字节相等、解析结构不变校验成功后，以实际候选字节记录摘要。没有忽略或删除校验。
- 公开控制套接字连接失败保留于上方第4项。这些失败没有变成生产配置损坏；最终原始配置逐字恢复。

## 结论与后续边界

当前版本/当前会话未满足用户要求，**支持列表暂为空**。尚未测试有效授权自动导入，未实现临时凭据导入包和客户端导入器；核心重载门禁未通过时不先包装出不可兑现的一键按钮。

后续需要当前运行实例提供导入端可调用的受支持重载通道，并实测新配置在当前会话生效。没有调用入口时不能通过启动第二个实例、后台重启应用或要求用户增加操作伪装成达标。仅此路径未通过，不能推断所有未来客户端版本不可能支持。

P1-044继续待验收。原阶段实现和安装证据不受此结论替代，未Commit/Push/Tag/Release，未进入P2。

## 2026-10-10 继续排查公开重载通道

用户追加要求：继续排查，通道实测成功后才实现“选择客户端和权限 → 复制提示词 → 自动导入并验证”。本次核对公开入口及当前运行进程，没有再次写入配置或生成安装包。

| 入口或证据 | 实际结果 | 对当前会话的结论 |
|---|---|---|
| 当前App Server PID7884，桌面父进程PID29660 | 启动参数含app-server，没有显式listen/sock选项；没有TCP监听端点 | 当前桌面进程未提供已发现的网络连接入口 |
| 默认app-server-control.sock及daemon.pid | 套接字不存在；doctor确认后台受管服务器未运行 | CLI的受管daemon连接目标与当前桌面进程不同 |
| codex app-server proxy | 实际发送initialize，退出1、OS10050，无协议响应、未超时 | proxy未建立连接，因此尚不能发送reload；不能仅凭OS10050归因为代理或网络问题 |
| codex mcp --help | list/get/add/remove/login/logout，没有reload命令 | 配置管理命令不等于当前桌面重载通道 |
| debug app-server send-message-v2帮助 | 没有当前进程/套接字/host附加选项 | 未启动调试会话，不能作为已验证的当前实例入口 |
| 当前可用Codex应用工具 | 没有MCP配置重载工具 | 当前AI不能直接调用协议方法 |
| 官方深链说明 | 设置或插件安装页面/流程，没有公开的无交互MCP重载动作 | 打开页面不能替代自动导入并验证 |

安装代码只读检查中，桌面本地启动构造的是app-server参数；显式Unix监听的线索位于SSH远端启动逻辑。此线索与进程/诊断结果相符，但不单凭代码文本断言所有可能通道均不存在。

官方App Server协议确有config/mcpServer/reload；外部协议客户端须先连接所选传输再初始化。[协议及传输](https://learn.chatgpt.com/docs/app-server)。官方桌面MCP设置步骤仍包含保存后选择Restart，[桌面配置](https://learn.chatgpt.com/docs/extend/mcp?surface=app)；[公开命令](https://learn.chatgpt.com/docs/developer-commands)与[深链参考](https://learn.chatgpt.com/docs/reference/commands)未提供本次所需的当前实例外部重载入口。

结构化结果见[P1-codex-reload-channel-result.json](evidence/P1-codex-reload-channel-result.json)，实际proxy错误和筛选的doctor输出见[P1-codex-reload-channel-output.txt](evidence/P1-codex-reload-channel-output.txt)。doctor整体退出1包含其他诊断项目，不能当作当前App Server失败的单独依据；这里仅采用受管服务器、配置解析、桌面版本检查。

结论仍为**重载门禁未通过**：当前版本尚未建立可由导入程序调用的受支持连接。严格零额外操作要求保持有效，支持列表仍为空。没有开始实现导入器，没有触碰凭据或业务数据，没有启动daemon、重启桌面或修改应用。原配置摘要保持一致。本结论限定于当前安装版本与本实例；后续需有可连接当前实例的公开通道，再做请求关联和有效授权验证，不能把新实例成功算作通过。
