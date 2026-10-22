# In Line CLI 使用指南

适用：v0.5.0开发构建。CLI与MCP分别授权；以下命令示例需安装包含CLI的新测试包后使用。开发包、安装成功、真实连接和人工验收分别记录。

## 1. 安装及首次接入

安装目录应同时包含 `in-line.exe`、`in-line-mcp.exe`、`in-line-cli.exe`。三者必须来自同一构建，不能单独覆盖其中一个。首版不修改PATH，始终使用完整路径。升级前结束正在执行的CLI命令并保存软件内工作；安装窗口另行安排。

1. 在软件设置的“AI MCP 接入”中选择所需CLI用途（Codex CLI或PowerShell CLI），选择权限、部门/类型范围，授权并复制提示词。
2. 将软件生成的提示词交给所选AI，或执行软件提供的本地导入命令。一次性包不是长期Token，不把Token、授权文件或环境变量内容发送到聊天。
3. 按导入结果中的实际profile继续操作；不要猜profile名称，不回退使用其他身份。有效权限是全局开关与客户端独立授权的交集。
4. `status --profile <实际名称> --format json` 验证CLI自身当前IPC连接。CLI连接成功不能证明聊天中MCP自动导入、重载或认证成功。

完整读取会包含联系人等字段；日常写入须额外选择。默认只读授权不自动升权。可以在软件内管理、轮换或撤销独立授权。安装升级更换二进制后，旧profile/短期包可能因二进制绑定失效，应按软件当前授权流程轮换接入，不能手工编辑加密profile或恢复旧Token。

## 2. 离线检查与读取

```powershell
& 'D:\Application\In Line\in-line-cli.exe' --version --format json
& 'D:\Application\In Line\in-line-cli.exe' doctor --format json
& 'D:\Application\In Line\in-line-cli.exe' status --profile chosen --format json
& 'D:\Application\In Line\in-line-cli.exe' task list --limit 100 --profile chosen --format json
& 'D:\Application\In Line\in-line-cli.exe' task search '合同' --profile chosen --format json
& 'D:\Application\In Line\in-line-cli.exe' task get 123 --profile chosen --format json
& 'D:\Application\In Line\in-line-cli.exe' queue future --profile chosen --format json
& 'D:\Application\In Line\in-line-cli.exe' report daily --date 2026-10-22 --timezone-offset-minutes 480 --profile chosen --format json
```

`doctor`只检查本地存在性，不读取凭证/业务库，不启动服务，也不表示认证成功。包核对使用GUI及sidecar的显式 `--build-info`，同样不启动GUI/IPC。普通事项读取默认不返回联系人、详情、内部备注或办理自由文本；必须有效fullRead才能请求相关字段。

日历基于真实办理历史，不是未来排期。日报标题按已确认约定保留“今日计划完成事项”，但四字段文本是实际办理回顾，带明确口径说明。文本用 `report daily --render text`，须有效fullRead，不能同时指定JSON输出。周/月默认本周期至锚定日，周起始日沿用设置；也可给明确from/to。

## 3. 分页与稳定结果

每页最多100。保留服务端返回的snapshot/cursor和完全相同条件再读取；不要把cursor用于不同profile/范围/投影。普通分页命令的 `--all` 有100页/10000项/16MiB上限，超限或失效明确失败，不能当成全部结果。

`changes list`先完成全量同步，再持原cursor读取events/current增量；失效后重新全量同步，不拼凑“缺失的历史”。`jobs result`保留jobId，按nextOffset显式领取1–100项；高级入口不支持all，也不会自动等待或替用户无限重查。

## 4. 写入与原回执

写命令必须 `--execute`、显式profile、明确用户意图/原因、原幂等键及原版本或字段基准。创建先查询近似重复；多个候选先消歧，不猜联系人、截止时间或新增义务。先读返回版本，再按原请求JSON构造操作；不要在冲突后自动拉最新版本覆盖。

创建、编辑、状态、父子/队列/未来事项命令及JSON字段见[命令草案](COMMANDS-DRAFT.md)、[阶段4报告](PHASE-04-WRITES.md)；高级批次、命名查询/jobs、偏好及产物见[阶段5报告](PHASE-05-AUTOMATION.md)。命令已固定的action/target等字段不能在JSON重复。

批量执行强制先调用共享预演，再带基准执行；预演不占号。多事项改期仍需在软件内一次批准整批，不能拆成单项绕过。处理轮次/办理记录只在真实办理时使用；未来取号、单纯排队或计划不能记成已办理。恢复回收站事项会按现有规则加入今日队列；归档重开用reopen。历史撤销只可申请，批准必须在软件内完成。

```powershell
& 'D:\Application\In Line\in-line-cli.exe' batch preview --input '批次.json' --profile chosen --format json
& 'D:\Application\In Line\in-line-cli.exe' batch execute --execute --input '批次.json' --profile chosen --format json
& 'D:\Application\In Line\in-line-cli.exe' receipt get --idempotency-key '原键' --profile chosen --format json
& 'D:\Application\In Line\in-line-cli.exe' receipt get --kind artifact --idempotency-key '原产物键' --profile chosen --format json
```

备份/export仅返回软件受控产物引用，不接受任意目标路径，不将整份备份读给AI。产物原回执只查账本元数据，不打开文件、不补做产物，也不证明文件目前完好。

## 5. 输出和错误处理

| 退出码 | 处理 |
| --- | --- |
| 0 | 当前请求已完成。预演仍只是dry_run；写入要同时检查commitStatus/verificationStatus |
| 2 | 参数/输入错误；按帮助及原服务字段修正 |
| 3 | 认证/权限/明确意图不足；由用户在软件内处理，不能换更大权限profile |
| 4 | 授权范围内未找到；不越权补查 |
| 5 | 冲突、业务规则、重复或歧义；先读原结果并向用户确认，不自动覆盖 |
| 6 | 环境/版本/资源、快照或增量失效等；保留error.code判断，incompatible须升级完整三程序或重新接入 |
| 7 | 未预期错误；用errorId读取自身脱敏诊断，不索取完整日志/凭证 |
| 8 | 后台尚未完成或待软件内批准；不能向用户报告业务已完成 |
| 9 | 结果不明/核验失败；保留原键，只查原回执，不换键重发 |

JSON模式stdout恰好一个UTF-8 Envelope，必须同时看status/data/error与退出码。文本成功走stdout，失败走stderr。`--input`接受UTF-8/BOM，`-`只用于有结束标记的管道；上限1MiB/10秒。禁止Token启动参数、任意SQL、永久删除、附件、覆盖恢复、授权增权、审批approve/force等命令。

## 6. 人工验收尚待执行

安装/升级、旧库保留、GUI授权及软件内审批、实际CLI导入/撤销/轮换、中文路径、旧profile绑定变化、当前聊天MCP连接须在安排好的本机测试窗口逐项核验。自动测试和安装包产物核对不能替代这些结果；见[阶段6报告](PHASE-06-DELIVERY.md)。
