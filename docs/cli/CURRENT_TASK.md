# CLI 当前任务：Phase2实现与自动检查完成，待用户验收

2026-10-21用户授权CLI Phase2，并在整机卡顿排查/正常重启后明确“继续inline开发”。当前分支`feat/scheduled-items`，HEAD及同名远端`1dcc1b02253f3bd80ad72a58cf3c3c7fab608aa5`，恢复时fetch后0/0；Phase1已验收并推送，Phase2未提交、未验收。完成后先交用户独立验收，之后才可提交推送，不进入Phase3。

## 权威与范围

先读[Phase2报告](PHASE-02-FOUNDATION.md)、[CLI PRD](../prd/cli.md)、[已确认架构](PHASE-00-ARCHITECTURE.md)、[命令草案](COMMANDS-DRAFT.md)。原任务附件路径见报告；压缩后先查这些文件，不与MCP P1/P8混淆。

- 独立`in-line-cli.exe`及`cli/`：--help、--version、离线doctor、显式CLI profile的status、UTF-8 JSON/table、退出码、1MiB/10秒输入边界。
- 仅lib.rs注册cli；没有更改领域、数据库schema、MCP工具/契约/安全/IPC/通知、依赖、版本或安装器。保留单Rust package，profile绑定用途/独立身份/机器用户/路径/二进制hash，不继承MCP环境变量或Codex Provider。
- GUI授权包/profile生产签发、client import及业务只读命令留Phase3；事项写入留Phase4。当前阶段不能让用户手工拼Token接入。

## 最终验证

- Rust首轮在系统提交额度耗尽时编译OOM，尚未跑测试；保留`evidence/PHASE-02-rust-first.txt`。用户重启恢复后再明确继续，补齐输入期限与完整权限/范围校验后以同一仓库外target、jobs1、test/dev调试信息0、test-threads1运行完整suite，**191 Rust通过（178库+2sidecar+11集成），零忽略/筛选**。
- 五项CLI单元、四项真实子进程通过；既有真实host/stdio/导入、领域事务等全部回归通过。计入隔离安全域，不代表生产连接。
- 资源守卫未触发，提交额度峰值58%、最小余量17.65GiB；总用时约2分38秒。会话70883已结束exit0，不继续等待或重跑。
- 119项前端/21文件、TS/Vite、fmt通过；后续未改前端输入，不重复无关大范围测试。
- 新构建的实际MCP sidecar完整16工具schema/注解、匿名拒绝/限流/纯JSON通过。
- 194份产品输入已冻结并回读；本地CLI独立副本帮助/版本/doctor及缺profile错误均通过。恢复开发前后已安装两exe/授权库/Codex配置摘要一致。未提交或暂存代码。

## 本地交付及停止边界

本地`release/CLI-Phase2-local-20261021/in-line-cli.exe`，4,160,512字节，SHA256 `4DCA0C80ADC13FFF36E712BE507B6E4A69A0094BF0BAFBDD571113E449D38032`；独立debug控制台，**不是安装包**。构建基线1dcc1b0、dirty、exactCommit=null，不使用未来提交SHA。完整证据见报告及evidence/PHASE-02-*。

不新建NSIS包、不安装、不改生产授权/业务数据、不启动另一个服务冒充本聊天MCP验证、不生成Actions包/Tag/Release。安装/原生GUI/真实部署留CLI Phase6，MCP P8当前会话接入待验状态独立保留。按原用户任务门禁，下一步仅等待CLI Phase2验收及提交推送授权；不得自动进入Phase3。

## 最新用户指示

用户在Phase2报告后指示“进入阶段3”。按阶段顺序，先记录Phase2通过并提交推送，再开展Phase3；当前检查点在Phase3开始时更新。该后续指示替代本文此前的等待验收停止点，保留历史记录及实机Phase6边界。
