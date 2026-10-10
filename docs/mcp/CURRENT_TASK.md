# 当前执行检查点

## 2026-10-10 最新授权：开发提交与P8统一验收

用户明确“继续，各阶段分别提交并推送，但先不验收，后续统一验收”，随后指定“在p8阶段验收”。允许各阶段验证后分别开发提交和推送，并继续至P8；P1–P7保持待验收，不创建accepted阶段Tag，不推断Release、安装或生产授权。下文限制按历史检查点保留，冲突时以最新用户决定为准。


## 2026-10-10 最新：用户指示进入P2

用户原话：“先进入下一阶段吧，顺便排查一下胶囊内文字的对齐情况，看是不是存在未对齐的问题，我更换字体后感觉没对齐”。字体确认是软件推荐的更纱黑体UI SC。

按本次最新指示进入P2；此前“不进入P2”已被覆盖。P1自动导入/当前Codex会话重载仍待验证，自动接入保持关闭，不冒称P1通过，不创建P1验收Tag。当前P2仅本地实现/隔离验证，提交、推送和新Actions安装包未获本轮另行指示。

基线`76ef34c1b843aa4ec845cd828562561f3e78b2a0`，`feat/scheduled-items`，fetch后与同名远端一致。上一轮Actions `38022058156`已成功，产物已交付；下文“等待Actions”和生产安装版本为历史快照，不能当作当前实机状态。

P2新增query_tasks/query_task_history/query_work_calendar/manage_saved_query；旧报表增加共享冻结快照、cursor与显式时区；schema11持久业务数据标识及事务内变化序号，内存SQLite快照，不混页数据。命名查询仅独立保存条件，save/delete需有效写权限及明确用户指令，没有事项写工具。详见[PHASE-02](acceptance/PHASE-02.md)及[P2查询指南](query-guide.md)。

侧栏数字及逾期胶囊统一22px高度、line-height:1、flex居中、tabular-nums；真实推荐WOFF2四字体/两档布局测试通过。其余票号、状态和联系人胶囊已有flex布局，此次不引入针对单个字体的像素偏移。

本轮最终本地验证完成：112项前端、104项Rust、fmt、TypeScript/Vite、四字体两档布局、NSIS及两份sidecar七工具包输入回读均通过；原始三份文件及144项矩阵校验通过。安装包3,785,885字节，SHA256 `BE7C72A20434EC98AFD682A7773968F43F442AB8009F316917E2F374DA482B5D`。当前生产主程序与sidecar摘要保持不变，没有安装本轮包。P2待用户验收；安装升级、生产授权客户端和原生多屏字体观感未验收。最终证据与首次失败记录见PHASE-02。

以下为此前检查点历史，遇到阶段/授权冲突按本节和最新用户决定继续。

更新时间：2026-10-10。当前阶段：**本轮新增单事项编号配色及未来子任务预占状态修复，用户授权包含全部当前改动提交/推送，并通过Actions生成测试安装包。P1仍未验收，不进入P2、不建立P1验收Tag；Codex自动接入门禁仍关闭。**

最新用户决定：提交范围B（包含全部当前改动、仍不进入P2），配色优先级B（仅逾期/最高优先级警示覆盖；加急和未来可使用自选色），子任务修复A（未来显示自动预占已开启、今天保留入队开关）。这覆盖此前“P1不得开发提交”的限制，仅允许本次开发检查点；不等于阶段验收。测试包按推荐的Actions Artifact生成，不创建Release。新数据库schema v10，生产安装仍是schema v9时代测试程序，未替换。

本轮实现已提交：`4d27a0bd752a1ba8486b59b2aa0dd613584b6d7f`；112项前端、91项Rust、两套隔离浏览器验证、fmt及最终本地NSIS/stdio通过。接下来推送开发分支并运行dev-build.yml，等待测试安装包产物成功。详情见acceptance/TASK-COLOR-AND-SUBTASK.md。以下旧“未提交”仅是历史记录，按本段最新授权和提交状态继续。

## 压缩后优先回读

1. `D:/Downloads/03-In-Line-MCP-Codex-总执行提示词.md`。
2. `D:/Downloads/01-In-Line-MCP-PRD-144项完整需求.md`，包含全部144条。
3. `D:/Downloads/02-In-Line-MCP-分阶段开发与验收方案.md`。
4. 原件不可用才读`docs/mcp/inputs/`同名校验副本；原件改变先比对差异。
5. 最新用户消息、适用AGENTS、PHASE-01及本检查点、矩阵；重新核验Git。

## 授权与固定基线

用户原话：“P0验收通过，C1–C4均选A，可以提交并推送，进入P1。”

- 仓库`C:/Users/KB/Documents/inline/In-Line`，开发分支`feat/scheduled-items`。
- P0固定提交`f2a97b34410f850f3da8c19d4ec6e21c7d63048f`已推送，annotated Tag `mcp-phase-00-accepted`远端解析同一提交；本轮开发提交必须是其后代，阶段Tag保持不变。
- main基线`1baac96252af2665274f2a955b3adacfa80519fd`；版本0.5.0，schema v9；不退回v0.4/main旧快照。
- C1=A受控MCP归档，C2=A批量改期应用内一次确认，C3=A真实办理追加/作废关联更正，C4=A全部部门AND类型、缺部门旧记录在限制范围下拒绝。C1–C3后续阶段实施。
- 三套编号配色组合/完整主题仍暂缓；单事项自选色是本轮用户另行明确新增功能。

## P1当前实现

独立客户端长期授权、DPAPI/当前用户ACL/机器SID绑定安全存储；权限组交集、范围、轮换/撤销/暂停、修订检查；受保护登录会话Named Pipe、双方nonce证明和对端路径检查、资源限制、sidecar预检及主服务限流；开库前按SID的Global mutex/就绪gate和非拥有者开库拒绝；静默自动启动及后台第二实例；旧报表共享域/限域/自由文本投影/100页长；真实3工具能力发现与结构化错误/isError；紧凑现有设置内管理UI；CI覆盖当前分支。

业务版本/快照共享类型已在basis.rs准备，尚未生成或落盘真实基准。不存在事项写工具、完整历史/稳定分页/快照、写入审计/幂等/批次、任务/增量/缓存/诊断工具。跨阶段需求只能部分完成。真实覆盖与未执行项以PHASE-01及144行CSV为准。

## 此前P1基础实现证据与首失败（不代表本轮接入流程安装验收）

- 前端106测试通过；TypeScript/Vite构建通过；Actions runtime策略6项通过。
- Rust最终全量75测试通过，包含sidecar预检限流/授权库超限原状态保留；见`P1-rust-delivery.txt`；fmt最终见`P1-format-final.txt`。
- 首次WindowsAPI导入、OS1455并行编译失败日志保留；使用-j1解决编译资源问题，不改系统配置。
- 真实主程序首次no Tokio reactor隔离复现，修复为进入Tauri运行时创建第一管道；原断言不变，定向/全量通过。
- 伪造管道端点首次错误码归类不符，缺失期望主程序规范化路径错误改host_unavailable；完整断言保留通过。
- 文档脚本计数/空键/零输出证据问题保留首次日志并修复。
- 本地标准NSIS冻结源码最终构建成功，包3,430,486字节，SHA256 F87530CD5B5FE09379DB19C31B31EE9E482A8E84334B654E67D48ECA4326B963；两份既有sidecar输入分别通过匿名stdio/限流门禁，字节摘要不同已记录；包回读证据见PHASE-01。没有Actions安装包或Release。

## 剩余门禁

用户已明确允许本机测试窗口中断当前MCP。2026-10-09 23:29–23:32执行：终止原安装目录主程序/sidecar，受保护离线备份12个文件逐一校验一致，NSIS静默升级退出0，已安装主程序和sidecar版本0.5.0；实际sidecar为第二个NSIS输入7A0E582F...。安装前/后/首次启动后schema9、268事项、11业务表逻辑内容一致，完整性/FK通过。已安装匿名stdio3工具拒绝、第6次限流/纯协议全部通过。主程序已从D:/Application/In Line/in-line.exe启动，独立授权存储已创建。

证据：P1-installed-readback.txt、P1-installed-startup.txt、P1-test-window-location.txt；回退副本保留在仓库外私有目录，原业务数据不入Git。computer-use两次初始化失败（kernel assets路径缺失），GUI可见性/布局/授权点击不得宣称通过。用户按acceptance/LOCAL-P1-TEST-WINDOW.md验收。原来无独立授权库，同机凭据升级保留仍未测；现库已schema9，本次不属于旧schema迁移实机验证。目标第三方客户端、跨账户/机器/会话、安装中断均未测。不可因为安装完成进入P2。

仅用户明确“P1验收通过，可以提交并推送，进入P2”后才能Commit→Push→固定`mcp-phase-01-accepted`→P2；正式Release另获批准。P0验证脚本是冻结历史检查，不适用于P1 HEAD/实现状态，不能重跑初始化脚本覆盖人工矩阵。

## 2026-10-10 最新接入流程调整

最新用户授权覆盖开发顺序：选择A，先实现软件内客户端选择/授权、安全提示词、导入器并完成隔离测试；自动接入通过实机验证后再开放。本轮新增onboarding.rs/codex_import.rs、前端安全Receipt与配置合并/真实stdio验证；新入口在UI、Tauri准备/轮换与release CLI共同关闭。测试和包证据另见CLIENT-ONBOARDING-LOCAL.md；当前生产安装仍为此前P1包，不能误称含本轮修改。此前“门禁前不能实现”的安排仅为历史，不再阻止已授权的本轮开发。P1不提交、不推送、不转P2或Release。

最新结果：109项前端和89项Rust测试、前端构建、Rust fmt、NSIS与两个release sidecar协议/关闭门禁均通过。当前Codex配置哈希与生产安装两exe哈希未改变；本轮仅隔离验证，无生产授权或安装。包SHA及首失败见[软件内验收报告](acceptance/CLIENT-ONBOARDING-LOCAL.md)。下一步为当前Codex实例重载验证与新流程安装/GUI实机验收。

### 历史重载排查与方案记录（以下“未实现”只描述当时状态）

最新环境覆盖：用户已自行更新Codex；实读桌面26.1007.2314.0、活动CLI0.162.0-alpha.17.2、App Server PID23556/父进程32712。新版默认控制套接字仍不存在，公开proxy实测退出1/OS10050且无协议响应；现有能力查询仍unauthenticated，仅说明MCP响应，不能作为重载成功依据。未重做新版配置写入探针，旧版本记录仅为历史证据。没有再次更新/重启Codex或In-Line，未改变配置或实现导入器。详见P1-codex-updated-channel-result.json。当前重载门禁继续未通过，不再基于旧版本建议重复升级。

用户已反馈get_capabilities为unauthenticated。只读核验当前Codex in_line配置缺少ID/Token环境字段，已安装主程序/sidecar仍为正确P1测试产物，不需要重复升级；未创建授权或写入第三方配置。

最新要求：选择AI客户端和授予权限后，复制提示词给该客户端即可自动导入，无需额外操作或输入。具体方案见client-onboarding-proposal.md：拟采用本机加密短期一次性导入包，提示词不携带长期Token，导入程序配置env并验证真实stdio和当前会话。当前只完成方案，没有实现新功能/生成新安装包。用户已选择A：严格零额外操作，仅支持实测自动导入并重载的客户端。当前Codex26.1002.7124.0/CLI0.162.0-alpha.2实机探针未通过当前会话自动重载门禁；原config.toml已字节一致恢复，不创建授权、不读取凭据或业务数据。支持列表暂为空，不再要求用户手填或重连，也不将公共API存在或新实例成功冒称当前会话成功。详见acceptance/CODEX-ONBOARDING-VALIDATION.md及P1-codex-reload-result.json。下一步依赖当前运行实例可调用且实测成功的重载通道；尚未实现导入包或生成新安装包。原阶段门禁不变，P1未验收，不能转P2。

本次继续排查公开通道：当前桌面App Server PID7884没有显式外部监听参数/TCP监听；默认控制套接字不存在，doctor确认受管daemon未运行。实际app-server proxy发送initialize后退出1/OS10050且无协议响应。mcp CLI、调试帮助、应用工具、官方深链未找到当前实例外部重载入口；不是已证明的网络故障，不能重置网络、另起daemon或用私有IPC注入冒称达标。证据新增P1-codex-reload-channel-result.json及P1-codex-reload-channel-output.txt。本次未改配置/产品代码，未重启Codex，未实现导入器。后续以发现受支持且可连接当前实例的通道为前提，再实测重载及授权能力，原门禁继续有效。
