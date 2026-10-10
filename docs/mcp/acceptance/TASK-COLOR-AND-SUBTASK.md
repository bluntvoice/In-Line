# 单事项编号配色与未来子任务状态：开发检查点

日期：2026-10-10。用户选择：提交范围B（全部当前改动，P1仍未验收、不进入P2）；颜色优先级B；未来子任务修复A。测试安装包按推荐的Actions Artifact方式构建，未授权阶段验收Tag或正式Release。

实现开发提交：`4d27a0bd752a1ba8486b59b2aa0dd613584b6d7f`。P1验收Tag仍不存在，P0固定Tag不变。本轮本地最终NSIS：3,710,018字节，SHA-256 `5F954F432D97B5D50AECE1C52CC097630BE38ADD8ACECFEDD989BF5A5B0D47CA`；未安装，Actions产物另按对应运行记录核验。两个sidecar输入哈希不同，分别验证通过，不声称字节一致或已解包校验安装文件。

## 本次变化

- 主窗口及悬浮窗右键“编号配色”：16候选色、3/6位HEX、失败重试、恢复自动配色。共享编号组件用于列表、详情、父子关联和子任务浮层；复制取号卡片保留原样式。
- 单事项`ticket_color`独立持久化，schema v10事务迁移并自动创建升级前备份；编辑及队列变化不清除颜色。新增恢复事项保留颜色，同内容合并事项保留本地颜色，非法值回退。
- 固定红色警示优先级最高（逾期或critical），解除后恢复自选色；加急和未来状态允许自选色。自选色为空时保留全局三类颜色；黑白字色自动适配。
- 未来子任务显示“自动预占未来日期号码 / 已开启”，取消禁用且关闭的误导开关；今天保留可操作开关。未来事项后端预占规则未变。
- 同批包含P1独立授权/限域/stdio/受保护IPC及软件内导入器改动；Codex自动接入仍关闭。没有业务写工具，P1尚未验收。

## 验证

| 命令 | 结果 | evidence文件 |
| --- | --- | --- |
| `npm.cmd test -- --maxWorkers=1 --no-file-parallelism` | 19文件112测试通过 | P1-task-color-frontend-first.txt；最终回读P1-task-color-frontend-final.txt |
| `npm.cmd run build`及dist前置构建 | TypeScript/Vite通过 | P1-task-color-build-final.txt；P1-task-color-and-subtask-nsis.txt |
| `cargo test --locked --manifest-path src-tauri/Cargo.toml -j 1` | 91测试通过（84库、2sidecar、5集成） | P1-task-color-rust-verified.txt |
| `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` | 通过 | P1-task-color-format.txt |
| `node scripts/verify-task-ticket-colors.mjs` | 隔离API/Edge无头浏览器交互通过 | P1-task-color-and-subtask-browser.txt |
| `node scripts/verify-ticket-colors.mjs` | 全局三类设置、浅色、失败重试、重开/跨窗、恢复默认及陈旧读取回归通过 | P1-task-color-global-regression-verified.txt |
| `npm.cmd run dist` | 追加子任务修复后的最终NSIS构建通过 | P1-task-color-and-subtask-nsis.txt |
| `docs/mcp/verify-package.ps1` | 当前包版本/输入、两个sidecar真实stdio/认证/限流均通过 | P1-task-color-package-readback.txt |
| 已安装程序哈希及release导入门禁 | 已安装两exe保持原哈希，自动导入仍拒绝 | P1-task-color-installed-boundary.txt |

浏览器覆盖主窗口/悬浮窗右键、16色、无效编码、保存失败/重试、其他事项不变、窗口同步、重开、加急/未来色、警示覆盖和恢复、重置、窄窗/Escape；未来子任务继承日期、今天/未来往返切换、今天开关及提交参数。使用合成API，不能称为实际Windows安装GUI验收。

SQLite覆盖关闭重开、单事项独立设置、非法输入保留原值、编辑保留颜色、备份导入ID变化后颜色跟随、清除及v9→v10升级前备份。MCP既有隔离安全/真实stdio回归均通过。未读取或修改用户实际业务数据库，也未安装本轮包。

## 首失败与修复

- Rust首轮3项：旧迁移夹具把版本调低但保留ticket_color列，迁移重复添加；增加实际列存在性检查。第二轮1项仍断言旧schema9，更新为schema10；关键升级前备份和旧数据断言保留。
- 浏览器首轮导航超时，随后发现Vite监听Rust编译产物触发EBUSY；测试服务排除src-tauri/生成目录。
- 跨窗口模拟多轮失败保留日志。诊断证明BroadcastChannel到达时接收页的localStorage复制尚未提交，bootstrap读到旧值；模拟桥增加实际storage提交事件后通知，最终全流程通过。未以强行刷新或放宽颜色断言放行。
- 旧全局颜色脚本遗漏P1安全状态模拟，打开设置时失败；补入当前返回结构。旧用例同时把critical当普通加急测试，按用户新规则改为非警示加急（不含三角标）；critical固定红色及三角标由新用例独立验证，未保留冲突的旧颜色期望。
- patch准备中两次上下文不匹配均在修改前拒绝，按真实文件片段重做。
- 首次暂存检查发现26个证据日志EOF多余空行，仅去掉末尾空行，全部文字输出保持；未跳过提交或测试门禁。

## 交付边界

本次开发提交与推送由最新用户明确授权，覆盖此前P1不提交的限制；用户未接受P1阶段，不建mcp-phase-01-accepted、不进入P2。三份原权威输入及144项决定保持冻结，仅追加本轮用户授权。当前生产安装和Codex配置不变，自动接入待当前会话重载实测成功后开放。

本轮安装升级、生产数据打开、主观视觉实机及当前Codex会话接入均未执行。schema v10不能直接由旧程序打开，回退需升级前备份。Actions运行与最终产物回读在完成后补充独立交付记录，不用历史包冒称当前产物。
