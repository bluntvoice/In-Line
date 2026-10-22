# CLI Phase3：独立授权与只读命令

日期：2026-10-21。状态：**实现及隔离自动检查完成；用户于2026-10-22指示“先推送，进入阶段4”，授权本阶段提交推送并在完成后进入Phase4**。

## 1. 授权与基线

用户在 Phase2 报告后指示“进入阶段3”。按原阶段流程，Phase2 已记录验收、提交并推送 `97aa8ad831b4f01a4b03a35a7d9d4c30932c33c1`；远端一致，分支 `feat/scheduled-items`，开始时工作区干净。主分支未合并。

遵循 [CLI PRD](../prd/cli.md)、[Phase0 架构](PHASE-00-ARCHITECTURE.md)、[命令草案](COMMANDS-DRAFT.md)、[当前检查点](CURRENT_TASK.md)。原任务附件：`C:/Users/KB/.codex/attachments/9d6f9e9e-144a-4c02-807b-646248f67f1b/已粘贴的文本.txt`。压缩后优先回读上述任务来源。

本阶段只读；没有提前实现 Phase4 写入或 Phase5 完整日报文本、批量和作业。用户已将安装、原生 GUI 与实机连接统一留 CLI Phase6。本阶段与 MCP P0–P9 的验收记录分别保存。

## 2. 独立授权和安全导入

- 沿用设置中的客户端授权界面，新增 Codex（CLI，独立授权）与 PowerShell（CLI，独立授权）。每次授权生成不同客户端 ID、Token 和 profile，不继承现有 Codex MCP 授权、环境变量或运行时刷新 Provider。CLI 客户端分类不证明 Codex 配置已重载。
- 同目录未找到 CLI 时不允许签发；当前阶段仅开放读取权限，前端禁用 CLI 写权限，GUI 后端拒绝新发或更新 CLI 写权限。完整读取仍须全局、客户端双方启用，并沿用首次敏感权限提示、部门/类型范围、暂停及撤销规则。
- GUI 通过现有受保护的本地授权入口签发独立 `kind=cli` 身份。CLI 没有授权、增权、修改权限或审批入口。
- 接入包独立存放于私有安全根 `cli-onboarding`，DPAPI 加密，机器与 Windows 用户绑定，10 分钟有效。提示词只有程序路径、一次性包 ID 和后续显式 profile 名，不含长期凭证。
- `client import --package <id> --format json` 无需用户手填 Token、环境变量或 profile。程序核对包用途、所选客户端、授权库中的独立身份、机器/SID、CLI 绝对路径/二进制 SHA256/版本、有效期与旧 profile 摘要。
- 使用私有 ACL、路径/reparse 检查、独占导入锁、一次尝试标记、加密备份、原子替换和文件回读。受保护 IPC 的实际 `get_capabilities` 还须核对客户端、版本/契约、有效权限、范围及授权修订；导入完成前再次核对授权、有效期与文件内容。
- 失败仅在目标仍是本次候选字节时恢复原密文；并发编辑保留。不明结果保留 `import_in_doubt`，禁止自动重放。重复使用、过期、损坏、用途/身份不符均拒绝。包目录有 100 个上限，7 天后仅清理已核对直接子目录中的已知文件，不递归删除未知内容。
- 轮换立即使旧 Token 失效。失败回退恢复的是 profile 文件，不恢复旧授权有效性，也不会自动增权或复活旧 Token。
- 成功输出分别记录 `configurationVerified=true`、`cliConnectionVerified=true`、`currentMcpSessionVerified=false`。这是 CLI 自身真实 IPC 连接验证，**不代表本聊天 MCP 自动导入、重载或认证成功**。

实现位于 `src-tauri/src/cli/authorization.rs` 和 `profile.rs`；原 MCP Codex 包格式、导入器、16 工具、schema 和受保护 IPC 协议保持原有契约。

## 3. 只读命令与服务映射

所有业务查询经现有受保护 IPC 调用主程序，CLI 不打开业务数据库、不调用 MCP 子进程、不自行计算另一套统计。只接受下表白名单，不提供通用工具分发、SQL、任意数据库路径或写入命令。

| CLI 命令 | 已复用服务和口径 |
| --- | --- |
| `status`、`capabilities`、`client status` | 有效显式 CLI profile 的 `get_capabilities`；不列举其他身份，不回退其他 profile |
| `client import --package <id>` | GUI 已授权的一次性 CLI 包导入；不能创建授权 |
| `task list`、`task search <text>` | `query_tasks`，原范围/过滤/投影；默认无回收站和自由文本 |
| `task get <id>` | 精确 ID 过滤；不存在及无权均返回范围内 `not_found`，没有越权补查 |
| `task ticket <id>` | 原编号、计划、队列及轮次字段固定投影；不生成图片 |
| `task children <id>` | 原 `parentTaskId` 过滤；父子可见性不扩大 |
| `task history <id>` | `query_task_history`，保留自由文本脱敏、作废记录权限等规则 |
| `queue list` | `activeQueue=true`，不按“今天计划日”缩窄跨日有效队列 |
| `queue future` | `scheduled=true`，不把未来预占事项计作办理 |
| `queue history <id>` | 历史服务固定 `kinds=[queue]` |
| `calendar list --from --to` | `query_work_calendar`，真实办理记录口径 |
| `stats summary --from --to` | 原 `get_report_summary` 及共享统计应用服务 |
| `report daily/weekly/monthly --from --to` | 当前基础入口均须显式起止日期，返回同一结构化摘要；不猜期间、不生成四字段文本 |
| `report items --from --to` | 原 `list_report_items`，保留原分页、快照、范围和字段规则 |
| `data health` | 固定 `inspect_data/health`；拒绝 action 覆盖及 trace/recompute 等无关字段 |
| `diagnostic get <error-id>` | 仅原服务中当前客户端自身的脱敏诊断 |

### 参数、输出与完整性

- 继续支持 UTF-8 单 Envelope JSON、无 ANSI、table、人类错误 stderr、帮助/版本离线、不更改 PATH。JSON stdout 不混日志或进度。
- `--input <file|->` 使用对应既有请求字段；保持 1MiB/10 秒边界、UTF-8 BOM 兼容。拒绝未知字段、重复 JSON 键、非法 UTF-8、尾随 JSON、重复/未知 flag 和 flag/JSON 同字段冲突，不回显输入正文。
- 常用 flag 支持 IDs、永久编号、文字、状态、部门、类型、优先级、工作量、加急、未来、有效队列、父子结构、归档、计划/创建/截止/办理日期范围；时间与投影的服务端校验、fullRead 要求保持不变。完整列表见 CLI 帮助。
- `--limit` 明确限制 1–100；cursor/snapshot 原样交给既有服务，不将游标输出到装饰提示中。历史另支持 kinds、include-voided，回收站仍须显式选择及原权限。
- `--all` 保持同一快照及每页重新鉴权，返回原始 `pages` 与独立 `pagination` 完整性元数据，不伪造单页 limit/offset。最大 100 页、10000 条、16MiB；初始 cursor/offset 与 all 互斥。任一页过期、越权、限流、快照不一致或超限均返回错误，不返回部分成功。
- 服务端既有每客户端 4 个/全局 16 个活动快照、TTL、64MiB 查询内存及调用限流不放宽；`--all` 也可能先触发这些保护。CLI 不自动清除其他查询或增权重试。
- 导入参数错误退出 2、身份权限错误 3、范围内不存在 4、导入冲突 5、过期/已用/不明导入/环境或资源问题 6。已有服务具体 `error.code` 保留。所有本阶段成功均为完成的只读或本地导入，没有待批准业务写入。

## 4. 自动检查与证据

| 检查 | 最终结果 |
| --- | --- |
| 完整 Rust suite | **199 通过**：182 库、2 sidecar、15 集成；零忽略、零筛选 |
| 真实 CLI 子进程 | 8 通过，包含导入、失败回退/不覆盖、范围拒绝、跨进程 cursor/逐页读取、完整读取双重授权、统计与原 GUI 查询一致、业务事项及事件读前读后一致、未知诊断拒绝、旧基础设施回归 |
| CLI 单元回归 | 9 项：参数/输入、typed 只读白名单、重复字段、分页边界、GUI 签发身份隔离、包绑定/损坏/过期/轮换、状态契约、退出码等 |
| 前端 | **121 通过 / 21 文件**；新增独立 CLI 客户端选择及写权限禁用检查 |
| 构建及格式 | TypeScript/Vite、cargo fmt、git diff --check 通过 |
| 实际 MCP 二进制契约 | 16 工具名称、完整 input schema、协议注解均与已审定 v1 基线一致 |
| 匿名 stdio | 实际 sidecar 的 16 工具及 3 次重复调用均拒绝；前五次未认证、后续限流，纯 JSON，无 host/授权/存储创建 |
| 来源与保护回读 | 198 份产品/测试/构建输入摘要与冻结记录一致；开发前后已安装 GUI/sidecar、授权库和 Codex 配置四份 SHA256 一致 |

完整 Rust 命令采用原仓库外 target、jobs=1、test/dev debug info=0、test-threads=1，没有降低断言、忽略测试或过滤完整 suite。资源守卫未触发；总用时约 83 秒，编译约 33 秒，提交额度峰值 58%，最小余量 17.77GiB。没有通过关闭用户软件、更改清理工具或分页文件来跑测试。

证据为 `evidence/PHASE-03-*`，最终 Rust 主记录为 `PHASE-03-rust-complete.txt`，运行参数和内存采样为 `PHASE-03-rust-runtime.json`、`PHASE-03-rust-memory-samples.json`。

### 调试记录

首轮发现导入错误尚未映射退出码，已补齐；随后真实进程测试按单一身份连续建立过多不同查询，触发既有四快照上限，未放宽保护，改为按命令家族建立独立测试域。新增业务只读校验最初误用了 crate 私有测试接口，改用现有公开事项/事件只读接口核对，没有为测试扩大生产 API。首轮、退出码修复后的失败、编译诊断及定向复测记录均保留；最终完整 suite 全部通过。

## 5. 本地交付、限制与验收停止点

本地控制台副本：`release/CLI-Phase3-local-20261021/in-line-cli.exe`，4,683,776 字节，SHA256：

`C5BFE2B9768BE61DCDD3D5B07D0459100CC22BCAA5EAFF69EC5EAD261ACE2B63`

这是 **debug 控制台，不是安装包**；保留构建基线 `97aa8ad...`、`sourceDirty=true`、`exactCommit=null`，不标成尚未创建的提交。实际副本的帮助、版本、离线 doctor 和缺 profile 错误已回读；没有使用生产授权启动 host。来源、摘要与检查见 `evidence/PHASE-03-local-artifact.json`。

- GUI 签发核心、静态界面与真实 CLI 导入/IPC 已用隔离合成数据验证；未进行已安装 GUI 的点击验收或生产数据接入。
- 本轮不生成 NSIS/Actions 安装包，不安装，不改生产授权、Codex 配置或业务数据，不创建 Tag/Release。CLI 尚未进入现有安装包；三程序安装、混版、真实路径/升级与旧数据保留留 Phase6。
- 本聊天挂载的 MCP 没有重新验证；隔离进程和 CLI 成功不替代其认证与原生重载验收。
- 后续 Phase4 才增加写命令、执行意图/回执/幂等/版本/软件内批准及写入退出状态；Phase5 才完成全文日报和高级自动化。

**下一步先由用户独立验收 Phase3，通过后才提交推送；本报告不自动批准 Phase4。**


## 2026-10-22 用户确认

用户在阶段3报告及选项后回复“先推送，进入阶段4”。按阶段顺序记录阶段3通过并执行提交推送，再开始阶段4；实机及当前聊天MCP验收仍留阶段6，不创建安装包或Release。上述交付时待验收/未提交记录为历史状态，由本确认覆盖。原话见 evidence/PHASE-03-user-acceptance.json。
