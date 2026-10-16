# 2026-10-16 P8首次授权与已运行sidecar刷新开发验证完成

最新用户已确认A：允许绑定所选Codex客户端的已运行sidecar安全刷新用户已授权凭证，替代原生重载。177项Rust（168库+2sidecar+7集成）、119项前端、首次授权/重复复制/维护浏览器、fmt、TS/Vite、实际16工具契约及匿名认证全部通过。最终全量Rust日志P8-onboarding-runtime-full.txt；旧P8-onboarding-rust-final.txt为首次失败历史，不能作为通过证据。

165份源码冻结后NSIS成功，包release/In-Line-P8-onboarding-local-20261016.exe，4,229,582字节，SHA256 02BA681F51150A19DF6C15EBEBD872E6DEEF7FD34084654FB48FD2116AA85CE4。两份sidecar输入分别通过真实契约/匿名权限协议回读，165份冻结摘要未变。当前安装/授权/Codex配置未修改，当前挂载会话尚未认证验证；安装与软件内用户授权后再实测，不能用合成连接代替。详见acceptance/P8-FIRST-AUTHORIZATION.md及P8-onboarding-retained-package.json。

本次继续P8开发提交推送，不建accepted Tag、不标统一验收通过、不进入P9/Release。完成后提供具体包供用户选择本机升级测试窗口；正常流程须由用户在软件内选择客户端及权限并授权，AI不代设生产授权。下文实施中/待选择/未回答均为历史。

# 2026-10-15 P8首次授权与已运行sidecar刷新实施中（历史）

用户已对第二个技术问题明确选A：允许绑定所选客户端的已运行sidecar安全刷新用户已授权凭证，以替代Codex原生重载；保持独立授权并实测。因此下文“待选择/未回答”为历史，现已允许实施。新模块runtime_credentials.rs按Codex PID/创建时间/可信路径和sidecar构建摘要绑定DPAPI密文，直接父进程限定现有MCP，配置验证子进程不能冒充；保持IPC每调用认证/暂停/撤销/限域，其他身份/不完整凭证不继承。导入报告仅预备刷新，当前挂载工具调用后才可判会话验证。

真实隔离同一stdio连接：未认证→授权包导入→认证→暂停→轮换旧token拒绝→导入新token→恢复→撤销拒绝已通过。首次轮换失败因旧刷新记录覆盖新环境凭证，修复后定向2集成通过。随后收紧直接父进程识别并加入本请求实际刷新标志。全量Rust在途，会话53334，日志P8-onboarding-runtime-full.txt；此前从C盘联接跑的全量失败于安全测试的重解析路径拒绝，原断言保留，改真实D目录验证。前端119项/21文件、实际首次授权/重复复制与维护UI通过。新真实桌面尚未安装/授权，不能声称当前会话已认证。详见acceptance/P8-FIRST-AUTHORIZATION.md。尚未冻结NSIS；编译中不要改Rust。下文为历史。

# 2026-10-15 首次授权门禁修复（选择前历史）

用户只读排查后选A，授权修复首次授权/验收门禁，继续零额外操作目标。开始HEAD=f9cf36fbc9ae6fc3f73471f881d92a5c86ea2db8、工作区干净。安装sidecar与P8摘要相同；实际安全库客户端0、全局未暂停；Codex启用in_line但未注入/转发ID和Token。此次不创建真实授权、不改配置或安装。代码已将authorizationAvailable与当前会话verified分开，允许主窗口首次授权/安全导入；当前会话验证标志仍false，默认权限和安全边界不变。软件内连接说明改为实际16工具，旧三工具/无写入文字已修正。

前端首次缺陷用例失败保留；定向4用例及首次授权/重复复制/维护浏览器已通过。浏览器因C盘联接迁移到D盘的cwd/模块解析失败，前几次日志保留，统一canonical cwd后通过。Rust onboarding定向通过；全量Rust会话63068/前端7103在途，日志P8-onboarding-rust-final.txt、P8-onboarding-frontend.txt。详见acceptance/P8-FIRST-AUTHORIZATION.md。

另有异步问题等待用户回答：A允许已运行sidecar绑定所选客户端安全刷新用户已授权凭证，替代Codex原生配置重载；B只用Codex原生重载、先修授权导入并保留阻断。**未收到此问题答案，不得按A实现依赖代码，也不把最初的A误当此选项答案**。当前公共控制套接字仍OS10050，无新app-server。完成已授权独立工作/验证后等待该技术选择；不得声称当前会话已接入，不进P9/Tag/Release。下文为历史。

# 2026-10-12 P8开发验证完成，提交推送后停在统一验收（历史）

最终173 Rust（165库+2sidecar+6集成）、115前端、六组合成UI、真实更纱四字体两档胶囊、16工具契约/匿名认证负例、fmt/TS/Vite、144原决定/文档/Actions政策通过。数值损坏文件保留原字节、计数checked_add及持久化前验证永久回归，用户A批准；首次失败仍保存，未削弱/隔离测试。

NSIS会话41880已成功关闭，sidecar3m51/main5m54，163份冻结源码未变；本地release/In-Line-P8-local-20261012.exe，4,213,972字节，SHA256 2AD7D37CD5475FDB0532A3C0A4A84539C9D6CFBC901DBA105A005B456E9A0B94。主/两sidecar输入真实契约和匿名协议回读通过，未安装/未验证升级，生产两exe保持原摘要。构建嵌入P7基线ba38b1002d3da15192fffab896168708918f6748/dirty，exactCommit=null。

本次独立P8开发提交标题test(mcp): 完成P8契约门禁数值校验与统一验收准备，最终SHA从Git历史与远端独立回读，避免自引用未来SHA。按现有授权提交推送后停止在P8；P1–P8未验收、不建accepted Tag、不进入P9/Release或安装生产程序。统一清单acceptance/UNIFIED-P8-CHECKLIST.md，结果acceptance/PHASE-08.md。当前会话get_capabilities仍unauthenticated，严格自动导入/重载未通过且入口关闭，不改配置/授权/另起服务。下文所有构建中与早期状态为历史。

# 2026-10-12 P8统一验收准备，NSIS构建中（历史）

P7独立提交ba38b1002d3da15192fffab896168708918f6748已推送，远端/main/P0 Tag精确回读，P7 CI38215281440 success。P8数值诊断损坏首次测试已失败证明原实现接受极值，用户选A批准补校验、保留原文件及长期回归；修复后173 Rust（165库+2sidecar+6集成）、115前端、实际16工具契约与匿名认证、六组合成浏览器均通过。真实更纱胶囊复验在途。163份包源码已冻结release/P8-frozen-source.json，NSIS构建会话41880、日志P8-nsis-first.txt；禁止改包源码直到完成。144原决定/原文档及当前映射校验通过，首次数值/文档旧036映射失败保留并已修复；Actions政策通过，生产两exe未变。

当前会话get_capabilities仍unauthenticated（P8-current-session.json），公开CLI/help复核无可用当前会话reload出口，严格入口关闭，不改授权/Codex配置或另起业务服务。详见acceptance/PHASE-08.md、UNIFIED-P8-CHECKLIST.md及integration-and-regression.md。完成本阶段包/证据后独立提交推送，停在P8等待用户统一实机验收；P1–P8未验收、不建accepted Tag、不进P9/Release。以下P7与之前进度均为历史。

# 2026-10-12 P7开发验证完成，准备独立提交推送（历史）

171 Rust/115前端/真实16工具stdio与认证管道/合成维护UI/fmt/TS/Vite/文档/冻结NSIS双sidecar输入回读通过，163份冻结源码未变。包4,214,129字节，SHA256 EF3CAC9A6A3459A55EA03B234A920DCAEF13F212B53EADE9B2F7086F013018DC，保留release/In-Line-P7-local-20261012.exe；生产两exe摘要未变，未安装。P6 CI success且精确SHA。P7准备提交推送后继续P8集成验收准备并等待用户，不建accepted Tag/Release，不进入P9。严格Codex零额外操作自动导入/当前会话重载仍未证明，入口关闭，MCP-100部分实现如实列出。详细结果见acceptance/PHASE-07.md。下文构建中记录为历史；NSIS会话48632已成功关闭。

# 2026-10-12 P7源码已冻结，NSIS构建中（待P8统一验收）

P7最终171 Rust（163库+2sidecar+6集成）全量通过，真实16工具/认证管道/自身错误编号关联通过；115前端此前通过，最终重测在途；P7维护组件最终浏览器验证、fmt、TS/Vite通过。源码冻结release/P7-frozen-source.json，NSIS正在构建，执行会话48632、日志acceptance/evidence/P7-nsis-first.txt。禁止期间修改被冻结的Rust/前端/清单。P7尚未提交，完成包双sidecar回读及文档校验后独立提交推送，再进入P8准备并等待用户统一验收。严格Codex零额外操作当前会话导入/重载仍未通过，入口关闭。原安装/授权/Codex配置未动。

新增代码：mcp/{diagnostic_types,diagnostics,observation,maintenance,build_identity,p7_tests}.rs、database/mcp_maintenance.rs；实际IPC统一请求ID，后台request/job/round及audit/原回执散列关联；危险未知写入只关联原回执核验，不由新写入成功解除；故障聚合/3轮30天/2次真实成功30秒/P0P1不可静音/7天静音/未解决长期保留/已恢复30天；RAM精确预览nonce、CSV/JSON本地确认及共享只读文件句柄字节回读；审计清理保护待撤销关联连通证据/回执/业务/正式导出，拒绝活动任务/预览变化，更新代际及派生基准使旧查询失效；首次敏感权限提示未确认时锁定授权字段，确认不直接授予权限；维护技术事件/重大故障和复发阈值合并应用内提醒；主/sidecar实际构建兼容元数据，不猜未来Commit。GUI详情折叠，无高级故障搜索、无故障校验文件、无自动上传/调用AI。

首次失败均保留：wire脚本编码失败导致两项缺失接口；State闭包类型推断；测试误用private open_root；两个旧15工具断言；stdio参数在RMCP层拒绝未到主机的夹具错误；缓存状态Mutex需要mut。最终Rust见P7-rust-frozen.txt，历史P7-rust-final.txt为首次mut失败不可作最终通过；UIfirst为受控checkbox check误用，second为对话框mock优化依赖问题，third/final均通过，虚拟mock不涉及生产。P7尚需NSIS包结果、独立Git提交/推送及P8集成验收准备。下文为历史。

# 2026-10-12 当前进入P7（待P8统一验收）

P6独立开发提交d7b103ccb8c6673d9b1748d56c09c8239da74aa7已推送、远端精确回读，初始工作区干净；157 Rust/115前端、真实15工具stdio、缓存/任务合成UI、冻结NSIS及双sidecar输入回读通过。包4,152,687字节、SHA256 6B003EB798ED4A3B2BABD10051958242880BB71410A14FC182ECF8F7C24D0664，保留release/In-Line-P6-local-20261012.exe，生产安装未变。P6 CI 38199299111 success（精确SHA）。按授权实施P7界面/诊断/故障/脱敏导出，安排见[PHASE-07](acceptance/PHASE-07.md)，独立提交推送后P8统一验收准备并等待用户；P1自动接入继续关闭，不建accepted Tag/Release。下文P6提交准备及早期状态为历史。

# 2026-10-12 P6开发验证完成，准备独立提交推送

157 Rust、115前端、真实15工具stdio、缓存/任务组件浏览器、fmt/TypeScript/Vite、文档及冻结NSIS双sidecar包输入回读通过。包4,152,687字节、SHA256 6B003EB798ED4A3B2BABD10051958242880BB71410A14FC182ECF8F7C24D0664，保留release/In-Line-P6-local-20261012.exe；生产安装未变。P5 CI 38195294658 success，21e856a5精确SHA。详见[PHASE-06](acceptance/PHASE-06.md)。准备P6独立提交推送后继续P7，P8统一验收；自动接入仍关闭，不建accepted Tag/Release。下文P6开始及早期状态为历史。

# 2026-10-12 当前进入P6（待P8统一验收）

P5独立开发提交21e856a533b0b72d200355339361470df6656035已推送且远端回读一致，初始工作区干净。147 Rust/115前端/真实stdio/合成任务UI/文档校验及冻结NSIS双sidecar14工具回读通过；包4,107,375字节，SHA256 313634A8D871D03F71071A8B520A0D942E0C459B28E6CD30954878FDD6B50D07，保留release/In-Line-P5-local-20261012.exe，生产安装未变。P5 CI 38195294658已success（精确SHA）。按用户授权继续P6数据健康、统计溯源及内存优先缓存，安排见[PHASE-06](acceptance/PHASE-06.md)，然后P7、P8统一验收；自动接入关闭，不建accepted Tag/Release。下文P5准备提交等均为历史。

# 2026-10-12 P5开发验证完成，准备独立提交推送（待P8验收）

147 Rust、115前端、真实stdio、合成任务UI、fmt/TypeScript/Vite、原文档/144决定校验及冻结NSIS/双sidecar14工具包回读通过。包4,107,375字节、SHA256 313634A8D871D03F71071A8B520A0D942E0C459B28E6CD30954878FDD6B50D07，保留release/In-Line-P5-local-20261012.exe；生产安装未变。详见[PHASE-05](acceptance/PHASE-05.md)。准备独立P5开发Commit/Push后继续P6、P7，到P8统一验收；P1自动导入/当前会话重载入口仍关闭，不建accepted Tag/Release。下文P5开始及更早阶段为历史。

# 2026-10-12 当前进入P5（待P8统一验收）

P4开发提交4dc2e35db4dbe988e6bb1e027db79134190d3f67已推送、远端回读一致，131 Rust/115前端/浏览器整批批准/冻结NSIS及双sidecar回读通过。包4,039,704字节、SHA256 293948468D7FAFF6D74D49738ADD148CF931E68CD5C5CD531CA33285A6095C5E，保留release/In-Line-P4-local-20261012.exe；未安装生产。P4 CI 38192183467已success且精确SHA回读。按用户指示继续P5长任务及增量读取，安排见[PHASE-05](acceptance/PHASE-05.md)，之后P6/P7、P8统一验收；P1自动接入仍关闭。下文P4提交准备为历史，最新本节优先。

# 2026-10-10 P4 开发检查点（待 P8 验收）

最新用户授权各阶段分别提交推送，验收集中于 P8。P4新增12工具契约、共享复杂领域动作、批次/改期确认/业务文件及schema13；验证结果见 [P4报告](acceptance/PHASE-04.md)。P1自动接入门禁保持关闭，各开发阶段不标验收通过。完成P4提交推送后继续P5/P6/P7，P8汇总验收并等待用户；不进入P9或Release。

# 当前执行检查点

## 2026-10-10 最新：各阶段开发提交推送，P8统一验收

用户明确“继续，各阶段分别提交并推送，但先不验收，后续统一验收”，随后指定“在p8阶段验收”。允许P2/P3检查点及后续P4–P7完成验证后分别提交推送，继续到P8集成验收。P1–P7保留待验收，不创建accepted阶段Tag，不推断Release或生产安装授权。P1当前Codex自动导入/重载仍未通过，入口继续关闭。以下旧限制保留为历史，冲突以本节最新用户指示为准。

P2通过已保存的P3前源码快照与隔离暂存树拆分提交`9e252a5786a7b5bf62ac2c4e798a5de76c3bdd2c`并推送；当前P3工作文件逐一摘要核对保持不变。快照CRLF按Git清理规则转LF，原始日志末尾空行保留。P2原112前端/104Rust/NSIS/七工具证据未改。P3独立提交`b5a306586b196144ee1cf3453d3f297dafc2a17e`并推送，使用最终115前端/116Rust/NSIS/十工具证据；首次TLS握手失败后保持原提交重试成功，远端回读一致。P2/P3的提交不代表验收通过。现在实施P4，安排见acceptance/PHASE-04.md；完成各阶段提交后自主继续P5/P6/P7，P8集中交付验收准备后等待用户。

## 2026-10-10 最新：P3本地完成，待统一验收

用户最新原话：“先进入下一阶段吧，后面我统一验收”。按此授权进入P3；P1、P2仍待验收，P1自动接入门禁关闭。本轮不推断提交、推送、阶段Tag或Release授权，不执行生产安装。P2源码已完整保存到忽略目录`release/p3-baseline-20261010.zip`，原改动没有清理或覆盖。

P3已搭建：五个现有GUI领域函数提取到database/domain.rs，同一Transaction供GUI/MCP使用；schema12新增字段/事项版本、长期AI审计、幂等回执与撤销申请；mutate_task（create/patch/setStatus/setUrgent/recordWorkEvent）、manage_preferences、request_undo，工具总数10。后两阶段的队列/子任务/批次/备份动作仍未提供。query_tasks新增字段/事项版本，时间线新增audit来源；设置内紧凑审计和软件内批准撤销入口，未开放MCP自动撤销。

当前验证：115项前端及TypeScript/Vite通过；P3-rust-frozen曾全量116项通过（108库+2sidecar+6集成）；真实stdio已测权限拒绝、局部写入幂等、用户批准撤销、audit摘要及提交后响应异常原键核验。十二项P3库用例覆盖字段/偏好ABA、整项回滚、重启回执、无变化跳过、相邻越权、未来撤销、迁移和后置核验失败。隔离浏览器审阅/批准/冲突/拒绝测试P3-undo-ui-seventh通过；首次脚本及编译/夹具/契约失败保留。

最新补强：偏好撤销记录实际前后值并独立后置读取。P3-rust-delivery的后台启动测试发现隐藏窗口偶发取得焦点，根因是WindowConfig缺省focus=true（visible:false仍已存在）；已在背景模式创建窗口前同时设visible=false/focus=false。保留原断言，三次定向及修复后的全量116项通过，最终日志P3-rust-final.txt；fmt通过。冻结源码最终NSIS与包输入回读通过，P3-nsis-first仅保留为补强前历史，最终日志P3-nsis-final/P3-package-readback。包3,925,946字节，SHA256 `8B7CB86CFF400010AD12651140E5D5D096B30E607C449CF5F7F49EF38211D754`，保存release/In-Line-P3-local-20261010.exe；两个sidecar输入均十工具匿名权限/限流/纯协议通过。18条P3矩阵及文档已更新，原始三份和144决定校验通过。生产两exe摘要独立回读保持不变。最新基线仍76ef34c，P2/P3均未提交；P2旧本地包保存在release/In-Line-P2-local-20261010.exe。P1/P2/P3均待用户统一验收，自动接入继续关闭，未进入P4。详细覆盖、失败与统一验收清单见acceptance/PHASE-03.md。下文P2“下一阶段待指示”为历史，本节最新授权优先。

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
