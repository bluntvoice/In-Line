# P2 授权只读、检索和报表验收（待用户验收）

## 最新开发授权

2026-10-10，用户授权各阶段分别提交推送，并指定P8统一验收。P2独立开发提交9e252a5786a7b5bf62ac2c4e798a5de76c3bdd2c已推送feat/scheduled-items；本报告测试和原始失败记录保留。P1/P2未验收，未创建accepted阶段Tag。下文旧提交限制仅为历史。

2026-10-10，开发分支feat/scheduled-items，基线76ef34c。用户指示“先进入下一阶段吧”，本轮按最新授权进入P2；P1当前会话自动导入/重载明确仍待验证，入口关闭，没有P1验收Tag，不声明P1通过。没有自动扩大权限。

## 实现范围

| 需求 | 实现/验证 |
| --- | --- |
| MCP-011 | query_tasks全历史、query_task_history已有完整来源，1—100固定快照分页；101事项并发更新跨页仍原内容/顺序 |
| MCP-027 | 白名单条件AND/字段候选OR、结构/时间/事件筛选及共享统计；拒绝SQL和未知字段，权限范围交集 |
| MCP-028 | 日/周/月/任意日期区间结构化统计、时区/范围/口径/缺口和真实数据版本；统计与同snapshot明细一致 |
| MCP-029 | 默认回收站/作废排除、归档有效历史包含；显式完整读回收站和作废时间线；字段脱敏 |
| MCP-030 | 独立客户端命名查询，持久化、明确意图/写权限检查、当前权限复核；模板无授权旁路 |
| MCP-031 | 共用现有只读工作日历，包括未办理队列区间；无计划日历引擎，P3/P4未来事项写操作尚未实施 |
| MCP-033 | 状态、队列进入/结束、办理、加急、原日志时间线，按绝对时间与稳定来源ID排序；尚无P3业务审计来源 |
| MCP-037 | 授权证据检索及分析政策，事实/推断分开；不保存关系 |
| MCP-078 | 内存SQLite冻结，客户端/查询/投影/时区/授权绑定cursor，代际/当前限域复核、过期/篡改/资源限制；不持有跨页生产锁 |

完整参数/限制与迁移规则：[P2查询指南](../query-guide.md)。144行原始决定及主阶段保持不变，9条P2主体记录更新为待验收，跨阶段行分别披露尚待后续阶段完成。

## 文件摘要

- database/mcp_read.rs及database.rs：schema11前置备份、持久UUID/事务内单调行变化序号、九表触发器、32MiB内存备份、隔离范围裁剪和时间线；原统计与日历函数复用，GUI日历范围限制保持。
- mcp/query_types.rs、query.rs、service.rs、contract.rs、security.rs及sidecar：七个工具、投影、快照/分页/过期/资源预算、命名查询、授权返回前复核。
- styles.css：侧栏数字和逾期胶囊统一22px高度、line-height:1、flex双向居中、等宽数字；保留推荐字体及图标含义。
- query_tests.rs、database内测试、mcp_queries.rs：合成SQLite/安全目录、真实stdio和受保护Named Pipe；不触碰用户授权/配置/数据库。

## 自动验证证据

- [前端](evidence/P2-frontend.txt)：112项通过；[构建](evidence/P2-build.txt)：TypeScript/Vite通过。
- [胶囊](evidence/P2-capsule-final.txt)：读取既有推荐字体缓存，大小/摘要匹配，CDP确认实际加载custom WOFF2；更纱UI SC、微软雅黑UI、Segoe UI、Arial，100%/150%布局的横向文字居中、纵向行框、图标居中和相同高度通过。截图位于忽略目录release/ui/capsules，仅合成事项；字体字面光学差异不靠固定像素平移掩盖。
- Rust、fmt、本地NSIS、包回读及文档检查最终结果见本报告末尾“交付结果”。

## 首次失败及修正

- P2-rust-first：PowerShell正则替换把分组和11合并成不存在的$111，导致四条schema断言语法失败；恢复原断言主体，仅更新预期schema11。没有改产品逻辑规避测试。
- P2-rust-second/P2-rust-fourth：既有常规读测试期望空note，P2按规范改为字段省略；保留内容不泄露断言，增加字段确实不存在检查。
- P2-rust-third：新合成夹具漏必填对接人，领域校验拒绝；补合成对接人，未放松领域校验。
- P2-rust-final：既有并发循环反复新建相同快照导致额度耗尽；相同查询只在数据身份/代际/变化序号及授权均相同时复用映像。保留并发原循环及统计隔离断言，新额度测试改用不同条件触发上限，并验证相同条件复用及数据改变后基准更新。
- P2-capsule-first：CSS inline-flex在flex父容器中被标准blockification计算成flex；改按实际布局语义验证，同时保留高度、水平/垂直居中及图标断言。
- 几次patch上下文未匹配在写入前被拒绝；根据当前格式定位后再改，没有删除或跳过业务/安全测试。
- P2-rust-verified：旧stdio测试固定三工具和全部readOnly；按真实七工具及命名查询非只读注解更新断言，原报表、权限变化与敏感数据断言全部保留。P2-rust-delivery随后全量通过。
- 包构建P2-nsis首次恰逢补强范围外关联日志脱敏，接口增加参数的两个调用点尚未同时更新，编译拒绝；补齐调用点后冻结源码，重新全量测试和完整NSIS构建，首次日志保留，不使用中间包。

## 尚未验收及限制

P1当前Codex会话认证/自动重载仍未通过；P2生产授权实机、安装升级、真实旧数据完整性及多屏字体观感没有执行。合成测试和本地NSIS不代表用户安装通过。没有进入P3、没有提交/推送/Tag/Release或发起新Actions构建。旧历史缺口如实披露，P3审计/P5增量与可靠重启恢复/P6健康与指标溯源没有宣称提前完成。

请用户在授权后的实际客户端验证只读查询和分页，并检查胶囊观感。当前待验收项不能由自动测试代替；下一阶段仍等待新的用户指示。

## 交付结果

- 前端112项、Rust104项（96项库测试、2项sidecar及6项集成测试）全部通过；最后一次冻结源码测试见[Rust证据](evidence/P2-rust-frozen.txt)。[fmt](evidence/P2-format-final.txt)、TypeScript/Vite构建及四字体/两档布局验证通过。
- [本地NSIS构建](evidence/P2-nsis-final.txt)完成，[包输入回读](evidence/P2-package-readback.txt)确认主程序版本0.5.0、所有二进制新于冻结源码、两份sidecar输入均通过七工具真实stdio匿名认证及限流检查；没有执行安装/升级或解包后安装文件摘要验证。
- 安装包`src-tauri/target/release/bundle/nsis/In Line_0.5.0_x64-setup.exe`，3,785,885字节，SHA256 `BE7C72A20434EC98AFD682A7773968F43F442AB8009F316917E2F374DA482B5D`。这是本地验收构建，没有发布或上传。
- [原始文档/144项映射校验](evidence/P2-documents-final.txt)、[Actions运行时策略](evidence/P2-actions-policy.txt)通过；[当前安装程序边界回读](evidence/P2-installed-boundary.txt)确认主程序和sidecar摘要保持不变。
- P2处于待用户验收状态；P1当前会话认证与自动重载仍待验证，自动接入入口保持关闭。本轮没有提交、推送、Tag、Release、新Actions构建或生产程序替换。
