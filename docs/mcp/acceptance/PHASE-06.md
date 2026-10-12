# P6 数据质量、统计溯源和缓存（实施中，待P8统一验收）

## 授权与基线

继续用户“各阶段分别提交推送，P8统一验收”。P5独立开发提交`21e856a533b0b72d200355339361470df6656035`已推送、远端精确回读，初始工作区干净。147 Rust、115前端、真实stdio/合成UI/文档校验/冻结NSIS及双sidecar14工具包回读通过；P5包4,107,375字节，SHA256 313634A8D871D03F71071A8B520A0D942E0C459B28E6CD30954878FDD6B50D07。CI 38195294658已success，精确SHA回读。P1自动导入/当前会话重载入口继续关闭。

## 原始要求及实施安排

对照三份原件：079、083–087、113–119。已冻结产品规则不改。

1. 受控短期报告内存缓存，客户端/授权/范围/查询/当前基准和口径绑定；TTL、容量、LRU、显式重算例外，来源/计算及校验时间可见。复用现有只读快照与统计计算；缓存不能替代分页快照。
2. 没有明确磁盘缓存性能必要性，按P0架构只启用内存层；不额外落盘完整正文。既有P5检查点/元数据继续使用Windows当前用户ACL，不额外缓存加密选项；同账户进程访问的残余边界如实说明。
3. 轻量健康检查只读、仅技术状态；检查索引事件总数旁证、预期触发器、数据/游标/快照/缓存有效性，不返回全库业务计数或他客户端状态。
4. 必要时仅事务重建派生索引/触发器，先校验再提交，变更索引代际强制旧游标重同步；不重建虚构历史，不修改业务记录、号码、审计或凭证。
5. 按原报告快照分页追溯指标：真实事项、有效事件ID与办理/办结不同分母、口径、条件和覆盖范围。与GUI共用SQL来源，不另写一套统计公式。
6. 差异诊断先比较同客户端的范围、时区、口径、权限/条件、数据基准和明细；只读重算缓存，不改原记录。领域动作合并进inspect_data的健康/溯源/比较/缓存操作，避免增加多个相近工具；草案对应能力保留。
7. 设置提供轻量空间/来源状态及一键安全清理；缓存清理不删除原数据库、正式导出、审计、任务检查点或分页快照。超限先自动淘汰，持续异常提示。
8. 定向越权/快照/缓存失效/基准差异/零残留索引事务与统计口径负面测试，真实stdio、全量前端/Rust/fmt/build，冻结NSIS及包回读后独立提交推送，继续P7。

## 已完成的开发验证

| 验证 | 结果/原始证据 |
| --- | --- |
| cargo test --locked -j1 --manifest-path src-tauri/Cargo.toml | 157通过（149库、2sidecar、6集成），零跳过；P6-rust-frozen.txt |
| npm.cmd test -- --maxWorkers=1 --no-file-parallelism | 115通过/20文件；P6-frontend-final.txt |
| npm.cmd run build | TypeScript/Vite通过；P6-build-first.txt；NSIS再次执行最终前端构建，index-DR8Ucvxu.js/index-CkoqT4sD.css |
| cargo fmt --manifest-path src-tauri/Cargo.toml -- --check | 通过；P6-format-final.txt |
| verify-mcp-cache-ui.mjs | 实际组件的空间/来源、仅一次明确清理、持续容量提示、只读刷新和明确点击派生重建；P6-cache-ui-first.txt |
| verify-mcp-jobs-ui.mjs | 完成计算时间/校验说明、取消审阅/收起不变更、仅一次取消、终态/保留/清理；P6-jobs-ui-first.txt |
| 原文档/144决定/版本及P0Tag校验 | P6-documents-first.txt通过；提交前再回读 |
| Actions运行时策略 | P6-actions-policy.txt通过 |
| P5 CI精确SHA | 38195294658 success，21e856a533b0b72d200355339361470df6656035；P6-prior-checkpoint-ci.json |
| 生产主程序/sidecar只读摘要 | P6-production-readback.json与P5相同，未安装替换 |

### 逐项覆盖

| 编号 | 实现与关键验证 |
| --- | --- |
| 079、113 | cache::key/get/put + jobs::fingerprint/execute；同客户端/授权/当前基准复用，显式重算与嵌套explicitRequery独立；真实stdio报告/trace验证 |
| 083 | quality::health、query::snapshot_health、Changes::health；仅技术状态、客户端绑定，损坏事件与触发器检测，不返回业务正文/他客户端数据 |
| 084 | quality::repair；事件丢失/缺触发器/错误派生索引检测，事务内30个触发器及权威基准校验；DDL中途失败全部回滚；原所有业务表/版本/审计/号码/序号保持；epoch使真实旧游标/快照/任务失效 |
| 085 | query::meta/trace覆盖与快照；计算完整性和未知历史完整性分开，分页及现值归属/原记录缺口明确，无虚构历史 |
| 086 | GUI共用mcp_metrics::RANGED_CTE/ELIGIBLE_COUNT_SQL；全摘要指标、两种比率、部门/类型/趋势、有效ID、作废/回收站/范围外排除，101条跨页原快照及指标/客户端绑定 |
| 087 | inspect::compare/recompute；范围/时区/权限条件/口径/基准/统计差异，原明细快照可继续追溯；不写业务记录，最大响应边界 |
| 114 | 原任务/快照、计算时间、校验时间及状态；McpCacheSetting/McpJobsSetting实际合成UI验证 |
| 115 | 60秒/8MiB/16条及每客户端4条、单条2MiB、LRU/过期/时钟倒退；数据/口径/权限/暂停失效；默认时区解析后绑定，启动已有联系人无变化不再推进基准 |
| 116–117 | 内存优先、无无必要磁盘报告缓存、冷启动不复用；既有私有检查点仅当前用户ACL，同账户残余访问说明，DPAPI凭证规则保持 |
| 118–119 | 只清RAM缓存、空间/来源及连续大条目旁路提示；实际审计及分页快照、真实任务检查点及正式CSV字节保留；P7补维护故障记录 |

完整13项映射见requirements-traceability.csv。本阶段新建mcp_metrics/mcp_quality、cache/inspect/inspect_types、McpCacheSetting及浏览器脚本；修改database共享统计关系/无变化字典操作，DataVersion/indexEpoch查询与任务失效，Security/service/sidecar15工具、lib/api本地入口、任务来源显示、协议及负面回归。业务schema仍14，无新增业务表迁移；DataVersion新增可选indexEpoch，旧持久基准缺此字段时不盲目续用。

### 保留的失败及修复

P6-check-trace-first为rate局部变量作用域编译错误，已提取共同上下文。P6-rust-first：3个新健康测试夹具缺department，补齐真实TaskInput必填字段，不改变生产可见性。P6-rust-second：完整源表读回证明旧启动联系人ensure_master会重复更新已启用值，推进基准及AUTOINCREMENT；现在先判断已启用项直接跳过，仍保留原未启用项重新激活规则，并新增稳定性/原规则验证。P6-rust-third全量155通过。P6-rust-final为新作废夹具误把record_work_event的()返回视为事件，改用真实list_work_events读取ID。最终P6-rust-frozen157全部通过。未删/隔离测试，原始安全/数据断言保持。

### 性能与待验收边界

报告复用60秒内有效；原分页租期600秒、SQLite32MiB/QueryState总64MiB及每客户端4个快照保持；溯源每个快照最多32个指标上下文，来源100000条/16MiB、每页100及响应7MiB限制；比较双方输出也受7MiB限制。统计来源构建使用同一SQL关系，存于原快照受配额内，不生成独立无界缓存。派生修复不重构旧事件，只从当前水位开始新代际。

冻结NSIS及双sidecar回读已通过，独立提交推送准备中。真实安装/升级、原生观感、Codex当前会话自动接入及统一用户验收仍到P8。P1门禁关闭，不建accepted Tag，不进入P9或Release。

## 冻结安装包回读

P6-nsis-first通过：sidecar3m40s、主程序5m45s；冻结Rust/Cargo/build.rs摘要全部一致。包4,152,687字节、SHA256 `6B003EB798ED4A3B2BABD10051958242880BB71410A14FC182ECF8F7C24D0664`，保留release/In-Line-P6-local-20261012.exe（P6-retained-package.json）。P6-package-readback.txt实际验证主程序/NSIS版本0.5.0、两个既有sidecar输入各15工具匿名/限流/纯stdout协议。两输入摘要分别C45BE60F632A1B6E1A5BD893199F0842D5A17DC9B79B4F55A7B0148AB6C73991及141823A665F1FF9E4A2752F7191F596929DA8C5A6876237D4A87CBE060F9A679；此为包输入证据，不是安装后文件提取证明。生产双exe摘要与P5相同。

安全fetch后HEAD与远端21e856a5一致、0/0，main及P0固定Tag未变。执行P6独立开发Commit/Push，之后继续P7，到P8等待统一用户验收；不建accepted Tag/Release。
