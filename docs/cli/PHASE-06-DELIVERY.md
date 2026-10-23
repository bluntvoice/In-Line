# CLI Phase6：回归与交付准备

日期：2026-10-22。状态：**开发与本地检查已获用户独立确认，执行提交推送与开发Action；实机安装与整体人工验收另安排。**

## 1. 授权及基线

用户已确认阶段5通过（含季度趋势修复），提交 `4194f2bc4571220587b01cc4aef35cfe3d8c984d` 已推送至 `feat/scheduled-items`，远端核对一致，阶段6开始前工作区干净。用户另要求“阶段6完成后，通过action执行测试版本安装包生成”；该要求已纳入后续交付，采用 `dev-build.yml` 的Actions Artifact，14天保留，不创建Tag/Release。

遵守原CLI任务“每阶段独立确认后才能提交推送”；阶段6报告交付后，用户已独立选择“A：确认阶段6开发与本地检查，提交推送并立即运行已授权Action生成测试安装包；实机验收另安排（推荐）”。确认记录见evidence/PHASE-06-user-acceptance.json；立即执行提交及开发构建，无需重复授权。安装/升级/生产数据核对另行安排，不能因构建成功自行标记实机验收通过。

## 2. 本阶段实现

### 三程序安装包

- `tauri.conf.json` 的externalBin同时包含CLI和MCP，原GUI binary保留。准备脚本按tauri/custom-protocol构建两个伴随程序，验证非空Windows PE；beforeBundleCommand在所有编译完成后以stage-only最后同步最终exe，避免二次链接时间戳使打包暂存与最终产物漂移。Cargo测试阶段的占位文件不会作为有效构建产物通过准备校验。
- 首版仍用完整路径，不修改用户/系统PATH，不替换同名别名。未改数据库schema，未新增依赖；安装器保留原currentUser策略和既有MCP退出处理，不另行结束Codex或用户应用。
- GUI及MCP新增显式 `--build-info` 离线元数据探针；CLI沿用 `--version --format json`。探针不启动GUI/IPC、授权加载或业务数据库。普通stdio模式保持JSON-RPC，不混入元数据文本。

### 包核对及混版保护

`scripts/verify-cli-package.mjs`读取实际三个exe，校验非空PE、元数据、软件/API/schema/构建基线/精确提交/dirty状态一致，并核对六个版本字段及Tauri双伴随程序配置；可验证Tauri暂存文件与release目录逐字节一致。Actions传入expectedCommit时只允许对应干净提交，不能将本地dirty构建标为正式提交。

`scripts/verify-installer-package.ps1`使用7-Zip只解出实际NSIS中的三个固定文件名，比较其与已核验源程序的SHA256，再运行离线元数据探针。不会执行安装器或安装钩子。旧GUI若不支持离线探针，先检查PE内支持标记并拒绝启动，避免未知参数退回正常GUI流程。CI环境若缺少解包工具或文件/身份不符会失败，不用“配置里写了CLI”代替实际包内容核验。

IPC在认证后、派发业务前检查peer build，与宿主的软件/API/schema/基线/精确提交/dirty标记不一致或旧端未提供身份时返回incompatible；CLI状态预检也核对宿主build。缺失/非法凭证仍先拒绝，保留原限流、HMAC、权限和结果释放检查。该检查防止无意混装，不替代独立凭证或二进制绑定；dirty本地构建仍标记为未验证提交。

### CI与文档

- `ci.yml`在原完整测试之后增加三程序离线身份检查。
- `dev-build.yml`和未来正式发布workflow在现有MCP契约/匿名认证检查之后，增加三程序、暂存伴随文件及实际NSIS payload核验。未改变任何工作流的触发或发布层级，本次只会按授权手动运行dev-build。
- 开发Artifact包含安装包、`package-build.json`、`installer-payload.json`和`installer-verification.json`，能核对精确提交、包内三个程序及安装包SHA256。
- [用户指南](USER-GUIDE.md)、[Agent指南](AGENT-GUIDE.md)记录独立授权、权限、分页、写入原键、实际办理/未来事项、审批、未知结果、版本及安装边界。

## 3. 自动验证

| 项目 | 结果及边界 |
| --- | --- |
| Rust完整回归 | 219通过：189库、2sidecar、28集成，其中真实CLI21项；0失败/忽略/过滤。`evidence/PHASE-06-rust-complete.txt` |
| 新增Rust检查 | 三个真实刚构建exe的离线元数据一致；不存在的隔离security root未被创建；同软件版本但不同构建/schema/API的混版拒绝 |
| 前端及包门禁 | 129项/22文件通过；新增4项验证三程序缺失、混版、目标提交及dirty不能冒充精确提交 |
| TypeScript/Vite | 初次因新增mjs缺类型声明及测试数据类型不符失败；补充d.mts及准确输入类型后通过，首次失败日志保留，不使用ts-ignore或放宽编译 |
| Debug包目录 | 实际三个exe离线身份及版本文件检查通过，未读取生产授权或连接生产宿主；报告位于本地release/CLI-Phase6-debug-package.json |
| 原MCP契约 | 实际sidecar完整16工具schema/annotations保持API v1基线；普通匿名调用及限流另按既有脚本检查 |
| 工作流/脚本 | Actions运行时策略、Node语法与PowerShell脚本解析检查通过；Actions真正执行仍在阶段6确认提交后 |
| 本地NSIS | 编译与最终重新打包通过，实际执行beforeBundleCommand；三个release exe身份、六个版本字段、两个暂存伴随exe的逐字节hash一致，release侧16工具契约/匿名门禁通过。未执行安装器 |

阶段4/5保留的真实GUI应用适配、CLI进程、MCP stdio一致性及合成并发/原子回滚/编号/未来/父子/限域/撤销/批量/日报测试均在本次完整回归中运行；不把这些合成结果称为已安装原生GUI实机验收。

### 本地包装修复与保留产物

首次NSIS编译成功但hash核验失败，定位prepare缺少Tauri自带的custom-protocol依赖feature；统一feature后仍因二次链接PE时间戳产生24字节差异。没有忽略时间戳或放宽hash检查，最终增加beforeBundleCommand最后阶段同步，并以真实tauri bundle命令验证该hook执行、重新生成NSIS。失败及修复证据见 `PHASE-06-release-package-first.txt`、`PHASE-06-second-build-link-drift.json`、`PHASE-06-nsis-first/second.txt`和最终 `PHASE-06-nsis.txt`。

完整Rust回归后，最终产品改动仅在准备/校验脚本、beforeBundle hook及包门禁测试；Rust业务源码未再修改。门禁测试、TS/Vite、Node语法、真实重新打包和release三程序核验覆盖最终改动，不重复无关的219项Rust完整检查。216个构建/产品/测试输入冻结哈希回读，详见 `evidence/PHASE-06-final-checks.json`。

本地保留包：`release/CLI-Phase6-local-20261022/In-Line_0.5.0-dev-phase6_x64-setup.exe`，4,315,931字节；SHA256 `E7A926EF2C6F3C8D4474C8530FA067137629D04F4081BB604E0027A038E5BD2E`。这是4194f2b基线加dirty源码的本地开发包，exactCommit=null，不标为尚不存在的阶段6提交或正式Release；未执行安装。报告见 `evidence/PHASE-06-local-artifact.json`。

本机未安装7-Zip，**未执行实际NSIS解包核验**；该检查已接入开发Action，将在确认提交后真实执行，成功前不声称包内解包检查通过。当前通过的是编译、最终release目录、暂存伴随文件及其离线身份/版本核验。旧本地NSIS已留备份，已安装主程序/sidecar、Codex配置、授权文件及旧CLI便携产物保持不变。

## 4. 本机测试窗口清单（未执行）

2026-10-23远端跟进：阶段6已提交推送e0c20871bad92f3af4aa012bd76ffd35f63d223e；CI run 39737737625通过。开发run 39737746725的完整测试、NSIS编译和MCP门禁通过，但目标干净提交检查失败，解包未执行、Artifact未上传。隔离副本实际复现Tauri初始化将Cargo.toml写为LF，core.autocrlf=true下git status误报修改；新增.gitattributes仅固定该文件LF后，同一初始化保持干净。保持原严格身份/hash检查，修复提交后重新运行开发构建。证据见evidence/PHASE-06-action-first.json。

第二次跟进：换行修复9327d66的CI 39882628623通过，开发run 39882659422的完整测试、NSIS编译、干净提交/暂存核验通过。实际解出三exe后GUI比较失败：Tauri会临时把GUI中唯一`__TAURI_BUNDLE_TYPE_VAR_UNK`写为`__TAURI_BUNDLE_TYPE_VAR_NSS`后入包，再还原release源文件，故两者hash本来不同。[固定版本源码](https://github.com/tauri-apps/tauri/blob/tauri-cli-v2.11.4/crates/tauri-bundler/src/bundle.rs)与本地实际包复现一致。现在从源字节重建唯一固定变换后的预期完整文件再严格比较；不忽略任意字节区间，两个伴随程序仍完全相同，报告分别保留source/payload SHA。新增3项回归覆盖额外篡改、缺失/重复标记、伴随程序变化；本地包测试7项、TS/Vite和脚本语法通过。

10月23日已从官方7-Zip 26.04 MSI只提取解包工具到ignored release，核对官方asset SHA256；未安装工具、未执行MSI或In-Line安装器。对此前保留的本地NSIS执行真实解包与新严格预期字节/离线身份校验，结果通过，见evidence/PHASE-06-local-nsis-payload-check.txt及PHASE-06-local-nsis-payload.json。该旧包仍为4194f2b+dirty，不能改标为新提交。原10月22日“未本地解包”记录保留其历史状态。远端Artifact仍须修复提交后通过新Action才能交付。

| 项目 | 验收方式 | 当前状态 |
| --- | --- | --- |
| 安装/升级与回退 | 保存并退出In-Line/CLI，在独立确认窗口备份原程序及数据；按包内SHA核对三exe，保留原路径与回退材料 | 待安排 |
| 旧库及用户数据 | 记录升级前schema/事项/父子/队列/历史/偏好/授权情况，升级后逐项独立回读；不使用旧版直接打开不兼容schema | 待安排 |
| CLI中文/空格路径 | 在实际安装目录运行帮助、doctor及profile接入，验证文本/JSON及退出码 | 自动临时路径测试已过，安装目录实测待安排 |
| 独立权限与轮换 | 用户在GUI选择CLI用途及范围、导入、只读拒写、fullRead交集、撤销/轮换与旧profile绑定变化 | 合成检查已过，真实GUI待安排 |
| GUI与软件内批准 | 单项/父子/未来、回收站恢复、批量待批准及历史撤销本机点击，核对数据刷新及通知 | 合成服务已过，原生GUI待安排 |
| 混装/兼容 | 用受控测试副本验证旧CLI/sidecar拒绝新宿主，重新完整升级并接入 | 门禁自动检查已过，安装副本实测待安排 |
| 当前聊天MCP | 只能通过当前挂载工具get_capabilities验证有效认证/权限，单独记录；另起实例不能代替 | 未重测、未验收 |
| 季度图表与字体 | 季度实际起止日期、首尾周裁切、明细及PNG；更纱黑体下胶囊对齐等历史UI需求 | 浏览器/单元检查已过，统一实机待安排 |

## 5. 发布准备及停止点

当前版本仍为0.5.0开发构建，正式版记录保持v0.4.0。不创建SemVer预发布Tag、不修改正式Release/Latest/更新通道。阶段6确认提交后按所提交精确SHA运行开发Action，构建结束提供run链接、Artifact/安装包及SHA；不能把workflow已触发当作产物已生成。

本地构建是dirty源码，exactCommit=null；保留元数据，不重新标为阶段6未来提交。阶段6开发与本地检查已独立确认；整体实机验收和安装仍待后续测试窗口，未完成部分保持待验收。原final-checks保留检查时历史状态；实际远端run、构建SHA、Artifact及解包结果另行记录，不能把旧本地包当作Action产物。
