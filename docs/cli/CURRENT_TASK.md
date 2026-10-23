# CLI 当前任务：Phase6开发及Action交付完成，实机验收待安排

更新：2026-10-23。用户已独立确认Phase6开发与本地检查，授权提交推送并立即运行开发Action；实机安装及验收另安排，无需重复询问提交/构建许可。

## 压缩恢复

优先读PHASE-06-DELIVERY.md第6节及evidence/PHASE-06-action-delivery.json，再对照USER-GUIDE、AGENT-GUIDE、docs/prd/cli.md、PHASE-00-ARCHITECTURE、COMMANDS-DRAFT及用户原CLI附件。CLI阶段与MCP P0–P9分开记录，不能将开发检查当作安装或当前会话接入验收。

## 已完成

- 阶段6开发e0c2087、LF修复9327d66、NSIS严格预期字节校验09f6144、探针前字节核对f39cdb5均已提交并推送feat/scheduled-items。
- 安装包构建SHA为f39cdb5c3e72d49d99cec9e4d7f070642cfafac8；后续交付文档提交不改变包身份。
- CI 39885248586与开发Action 39885271373最终均success。219 Rust、133前端/22文件、TS/Vite、fmt、16工具完整契约/匿名限流、三程序干净精确提交、伴随暂存及实际NSIS解包检查通过。
- Artifact 12472598944已上传并下载，归档digest匹配；本地再次独立解出三exe，SHA及离线身份回读通过。Action链接：https://github.com/bluntvoice/In-Line/actions/runs/39885271373 。
- 交付包release/CLI-Phase6-Action-20261023/In-Line_0.5.0-dev-f39cdb5_x64-setup.exe，4,326,442字节，SHA256 ABA3ED844AA79CD0685729AFFB0060A2B714C0189A0688A4E536E676B94477E8。同目录有SHA256SUMS、云端报告、独立回读及delivery-receipt。Artifact于2026-11-06 09:58:26北京时间过期，本地副本保留。

## 失败历史及处理

- 首次开发run 39737746725失败于干净提交检查：Tauri把Cargo.toml写为LF，core.autocrlf=true误报修改；隔离复现后仅用.gitattributes固定该文件LF，不放宽dirty校验。
- 第二次run 39882659422通过干净提交、实际解包后GUI比较失败：Tauri对GUI执行唯一UNK→NSS bundle marker变换再还原源文件。现重建完整预期字节比较，不忽略字节区间；CLI/MCP仍逐字节相同，source/payload SHA分记。先验证解包字节，再执行离线探针。
- 中间09f6144的run 39884961327由最新同分支开发构建替代，不作为最终产物。
- 10月22日本地dirty旧包仍保持4194f2b基线、exactCommit=null及原SHA，不改标为Action包；原final-checks和所有失败证据保留当时状态。10月23日已从官方7-Zip MSI仅提取工具到ignored release并核对官方asset digest，未安装工具或执行MSI；本地实际解包检查现已完成。
- 最终完整日志在ignored release/CLI-Phase6-job-*.log；持久提交只保留摘要及无凭证报告。不要等待已结束的旧构建/测试会话。

## 仍待实机验收

用户明确要求实机安装另行安排，不能自行升级或中断In-Line/MCP。安装/旧库数据保留/GUI真实授权审批/CLI真实profile/回退/季度趋势及字体胶囊等按报告清单统一验收。当前聊天MCP未重测，不能用配置写入或另起sidecar冒充当前会话连接成功。长期凭证仍只在本机，不进聊天/提示词/启动参数。

未创建Tag或Release，未运行安装器，未操作生产业务库；不自动进入新阶段或把整体人工验收标为通过。
