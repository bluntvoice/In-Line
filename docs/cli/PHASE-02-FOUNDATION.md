# CLI Phase2：控制台基础设施

日期：2026-10-21。状态：**实现及自动检查完成；用户在收到验收报告后指示“进入阶段3”，按阶段流程记录Phase2通过并提交推送，再进入Phase3**。

## 授权与范围

用户在Phase1已验收并推送后指示“继续下一阶段”。本轮进入CLI Phase2，基线`1dcc1b02253f3bd80ad72a58cf3c3c7fab608aa5`，分支`feat/scheduled-items`，开始工作区干净，fetch后与同名远端0/0；`origin/main`仍为`1baac96252af2665274f2a955b3adacfa80519fd`。

遵循[原架构与阶段计划](PHASE-00-ARCHITECTURE.md)、[命令草案](COMMANDS-DRAFT.md)、[CLI PRD](../prd/cli.md)。原任务附件在`C:/Users/KB/.codex/attachments/9d6f9e9e-144a-4c02-807b-646248f67f1b/已粘贴的文本.txt`。实机安装、原生GUI与当前MCP会话验证按用户决定留Phase6；CLI阶段与MCP P0–P9分开记录。

本轮只增加独立控制台binary及其适配器，不开放事项查询或写入。不改变领域、数据库schema、MCP工具及参数、认证协议、宿主分发或审批。

## 实现

- `src-tauri/src/bin/in-line-cli.rs`是控制台入口，Cargo按现有`src/bin`规则自动发现，无Windows GUI subsystem声明。
- `cli/parse.rs`显式匹配四个基础命令及五种全局选项；无需新依赖、Cargo配置或锁文件变更。未知/重复选项、多命令、缺失值和非法profile都拒绝，错误不回显参数原文。后续命令扩展时再评估成熟解析库，不借此阶段扩大范围。
- `--help`、`--version`只使用静态帮助和现有构建身份。无参数显示帮助。未实现命令返回2，不能模拟成功。
- `doctor`只检查同目录host/sidecar与授权库/所选profile的存在性，不读取密文或Token、不打开数据库、不启动主程序、不修复或建目录。缺同目录程序返回6；存在也仅报告`compatibility=not_verified`和`connectionVerified=false`，不证明二进制可执行或版本一致。
- `status --profile <name>`只读取指定CLI身份，随后复用原受保护IPC调用`get_capabilities`。复用原权限、限域、挑战/HMAC、服务端身份、限流和结果释放前授权修订复验。只有返回的客户端ID、软件/API/schema、16工具及实际连接确认匹配时才成功；本地传输标为`local_ipc`，原MCP能力结果嵌套保留。
- `status`缺profile或找不到该profile返回未认证，不读取`IN_LINE_MCP_CLIENT_ID/TOKEN`，不调用Codex Provider，不回退其他profile。成功也明确`currentMcpSessionVerified=false`，不能证明本聊天会话MCP自动导入成功。
- `--format json`始终在stdout写一个UTF-8 Envelope（含错误），无颜色/进度/日志；人类格式成功使用字段和值两列，错误只去stderr。输出失败返回6。`--no-color`可用，目前输出全部无颜色。
- `--input <file|->`本阶段仅供status接受空JSON对象；读到上限1MiB+1即拒绝超限，接受单个UTF-8 BOM，拒绝非法UTF-8、未知字段、数组/null和尾随JSON。路径直接作为文件参数使用，无shell解释。stdin仅在显式`--input -`时读取，拒绝交互终端；读取/校验期限10秒，未关闭的管道不会无限挂起。文件须为普通文件，拒绝目录/设备输入。超时返回单一参数错误Envelope，不显示输入正文。

### 安全授权适配与安装设计

本阶段仅实现profile读取器，**不提供创建、授权、导入或修改凭证的生产入口**。`client import`、GUI中CLI用途授权/一次性包签发属于Phase3（草案原列2–3）；Phase2不能让用户手工拼装明文文件作为接入步骤。

profile拟放在现有用户私有安全根下`cli-profiles/<name>.dpapi`，名称严格限制为1–48位小写字母开头的字母/数字/下划线/短横线，并排除Windows保留设备名。加载时核对路径无reparse、目录和文件私有ACL、32KiB上限、DPAPI、purpose/name、machine/SID、CLI绝对路径、二进制SHA-256及软件版本，再核对现有授权库中有效独立`kind=cli`客户端。损坏文件保留原样，无自动恢复或提权。合成测试可以构造fixture；这不表示GUI已能签发真实CLI profile。

首版由安装目录完整路径调用，不改用户/系统PATH或覆盖别名。Phase6才将CLI加入安装包并实际核对三binary、中文路径、旧数据保留和混版拒绝；本轮不改NSIS配置或安装钩子。CLI可按原IPC规则在有效授权后按需后台启动同目录host；无效授权在连接和启动前拒绝。离线doctor永不走此链路。

## 输出与退出码

Envelope沿用`apiVersion/requestId/status/data/error`；CLI本地语法错误仍使用`invalid_arguments`，消息说明本地问题且不回显参数。失败的doctor/status允许在`data.local`附安全存在性事实，但`status=error`、非0退出码与`connectionVerified=false`保持明确，不能当业务成功。

| 退出码 | 当前分类 |
| --- | --- |
| 0 | 当前只读/元数据命令完整成功 |
| 2 | 参数、输入、未实现命令 |
| 3 | 未认证、禁止、撤销、暂停、明确意图或敏感风险确认不足 |
| 4 | 授权范围内不存在 |
| 5 | 版本/幂等/业务冲突、疑似重复或需消歧 |
| 6 | 环境、连接、安全库、兼容、限流及查询资源/恢复错误 |
| 7 | 内部错误或未知错误code，未知Envelope状态不默认为成功 |
| 8 | 后台结果尚未完成的`job_not_complete` |
| 9 | 结果不明或执行期间授权变化；不得换键重放 |

目前没有写命令、jobs或pending成功结果。后续Phase4/5接入时必须在适配器增加对应提交/核验/pending状态判定，不能直接将任何`status=ok`写入结果判0。此处基础退出分类不意味着这些业务能力已实现。

## 验证及首次失败

前端119项/21文件通过，TypeScript/Vite构建通过（`evidence/PHASE-02-frontend.txt`、`PHASE-02-build.txt`）。Rust首轮使用Phase1仓库外target、jobs=1、test/dev调试信息0及test-threads=1，在编译库时出现`rustc-LLVM ERROR: out of memory`并以0xc0000409退出，尚未运行Rust用例；原始日志保留在`evidence/PHASE-02-rust-first.txt`。不能声称CLI子进程、IPC及原有Rust回归已通过，未简单重跑。

用户随后反馈整机持续卡顿，转为只读系统排查。Windows资源耗尽事件2004记录：2026-10-21 09:23提交内存41,577,467,904/41,708,347,392字节（约99.7%），昨天17:21已出现同类告警；09:25附近rustc、Codex及输入法更新程序崩溃。13:00采样仍为93–95%提交内存，CPU4–18%、磁盘队列0，已无cargo/rustc/link进程。停止新增构建，不为通过验证关闭用户软件、改页面文件或削弱断言；待资源稳定后按用户指示恢复。

## 最终验证与交付结果

| 检查 | 结果 | 证据 |
| --- | --- | --- |
| 全量Rust | 191项通过：178库+2sidecar+11集成；零失败/忽略/筛选，完整suite | evidence/PHASE-02-rust-resumed.txt |
| 新增CLI回归 | 5项单元+4项实际子进程；UTF-8/BOM/中文空格路径、单Envelope、未知参数、10秒未结束管道、私有profile绑定、拒绝MCP身份回退、真实IPC认证/暂停/撤销 | 同上 |
| 资源保护 | 全量用时约2分38秒（编译1分54秒）；守卫未触发，提交余量最低17.65GiB、提交额度最高58% | evidence/PHASE-02-rust-runtime.json、PHASE-02-rust-memory-samples.json |
| 前端与构建 | 119项/21文件通过；TypeScript/Vite通过；本轮后续未改前端/依赖/构建配置 | evidence/PHASE-02-frontend.txt、PHASE-02-build.txt |
| Rust格式 | cargo fmt --check通过 | evidence/PHASE-02-format.txt |
| 实际MCP契约 | 新构建sidecar的16工具名字、完整input schema及全部注解与既有基线一致 | evidence/PHASE-02-contract.txt |
| 实际匿名协议 | 16工具匿名拒绝、重复限流、stdout纯JSON、结构化错误通过 | evidence/PHASE-02-anonymous-stdio.txt |
| 产品输入冻结 | 194份产品源码/测试/构建输入摘要未变；既有变更仅lib模块注册 | 本地release/CLI-Phase2-frozen-source.json、evidence/PHASE-02-final-checks.txt |
| 独立本地CLI副本 | 帮助、版本、缺同目录宿主的doctor、缺profile的status均按预期输出单Envelope和退出码，隔离目录未创建安全库 | evidence/PHASE-02-local-artifact.json |
| 生产保护 | 已安装两个exe、授权库和Codex配置摘要与恢复开发前一致；未运行生产业务测试 | evidence/PHASE-02-protected-readback.json |

本地CLI验证产物：[in-line-cli.exe](../../release/CLI-Phase2-local-20261021/in-line-cli.exe)，4,160,512字节，SHA256 `4DCA0C80ADC13FFF36E712BE507B6E4A69A0094BF0BAFBDD571113E449D38032`。这是独立debug控制台程序，**不是安装包**，没有复制真实profile或长期凭证。基线`1dcc1b02253f3bd80ad72a58cf3c3c7fab608aa5`、源码dirty，`exactCommit=null`；不声称对应尚不存在的Phase2提交SHA。

源码启动示例（已配置Rust/Windows构建环境；生产安装目录尚无CLI）：

```powershell
& 'D:\codex-data\projects\inline\In-Line\release\CLI-Phase2-local-20261021\in-line-cli.exe' --help
& 'D:\codex-data\projects\inline\In-Line\release\CLI-Phase2-local-20261021\in-line-cli.exe' --version --format json
```

安装目录完整路径调用与三binary打包仍按Phase6实施。本轮没有NSIS/安装器变更，不将CLI加入既有安装包；未新建NSIS包或执行安装升级。GUI授权包/profile生产签发尚待Phase3，不能以合成profile测试代替真实客户端部署验收。当前AI会话MCP连接未因本轮CLI验证而获得确认，MCP P8待验状态独立保留。CI原有全量cargo test会自动运行新增CLI集成测试，无需修改工作流或工具基线。

## 验收与停止点

### 恢复开发记录

用户13:34正常重启后反馈流畅，之后明确要求“继续inline开发”。恢复时重新fetch核对远端0/0，保留所有未提交工作；17:14提交余量约19.4GiB、无运行中的编译进程。源代码复核补齐10秒输入期限及完整权限/范围响应校验；保留原失败日志，并补充不关闭写端管道的真实子进程回归。恢复后全量Rust仍用同一独立target、jobs1、test/dev调试信息0、test-threads1，不筛选或忽略用例。临时本地验证守卫以5秒等待加一次计数器查询的间隔采样；提交余量低于4GiB或占额度达到90%，连续两次则只停止本轮cargo及其子进程。守卫日志记录实际触发与否，不改变产品、CI或系统配置。

README及Release Notes已同步检查并注明CLI未进入现有安装包；不把本地源码功能描述为正式发布。前端源码、依赖和构建配置未变，已通过的119项前端及TS/Vite不为文档/后端输入修复重复执行。

完成自动检查后报告结果及限制，等待用户单独确认CLI Phase2验收；之后才提交推送。**本轮不进入Phase3，不安装实际程序，不修改生产授权/业务数据或Codex配置，不启动另一个服务冒充当前MCP接入，不生成Tag/Release或Actions安装包。**

## 用户阶段指示

2026-10-21，用户收到Phase2交付报告和验收选项后指示“进入阶段3”。按既定阶段顺序记录Phase2通过并授权提交推送，随后开始Phase3；实机部署仍留Phase6。该指示不包含安装、生产授权、业务写入或发布。验收单独记录于`evidence/PHASE-02-user-acceptance.json`，不改写构建时本地产物的`userAccepted=false`溯源记录。
