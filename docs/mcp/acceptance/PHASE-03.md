# P3 单事项写入与业务审计（待统一验收）

## 最新开发授权（覆盖下文历史提交限制）

2026-10-10，用户授权各阶段分别提交推送，并指定P8统一验收。本报告自动验证通过后允许P3开发提交，P1–P3仍待用户验收，不创建accepted阶段Tag或Release，不执行生产安装。P2源码已用隔离暂存树独立提交9e252a5，当前P3文件保持不变。

## 基线与授权

2026-10-10，feat/scheduled-items，基线76ef34c1b843aa4ec845cd828562561f3e78b2a0。用户明确“先进入下一阶段吧，后面我统一验收”；本轮进入P3，不将P1/P2视为验收通过，不创建验收Tag、不提交推送或发布。本轮原有P2改动完整保存到忽略目录release/p3-baseline-20261010.zip。

三份原始需求重新对照，144项决定不改；P3主体18项：004–005、012–016、018–021、034、048、072、074–077。004的复杂写入保留P4待实施，005审计导出UI补全仍属P7。详见[写入指南](../write-guide.md)和[完整矩阵](../requirements-traceability.csv)。

## 文件与实现

- database/domain.rs：五个现有GUI领域函数提取，同一事务供GUI/MCP复用，原业务函数主体保留。
- database/mcp_schema.rs、mcp_write.rs：schema12迁移前备份；事项/字段/偏好版本触发器；审计、幂等、撤销申请；字段冲突、ABA、后置核验、范围检查及用户批准补偿。
- mcp/write_types.rs、service.rs、contract.rs、sidecar：真实十工具，三个新增工具，明确意图、白名单、受保护IPC；禁止自动撤销、任意命令和P4动作。
- mcp/ipc.rs：写请求可能发送后传输失败明确result_unknown，不盲目重放；主程序写后轻量刷新UI。
- query.rs、database/mcp_read.rs：授权版本和audit时间线摘要，不回显越权自由文本。
- McpAuditSetting.tsx、mcp-audit.ts、api.ts及本机Tauri命令：紧凑审计与批准/拒绝入口，用户双步确认统计影响，不抢焦点。

## 验证与首次失败

- 前端首轮115项、TypeScript/Vite通过，见P3-frontend-first/P3-build-first。
- P3-check-first：序列化消耗task后再次访问ID导致借用编译错误；改为借用序列化，P3-check-second通过。
- P3-write-first：新断言需要DataVersion Debug，补derive保留相等断言。
- P3-write-second：4通过3失败；新夹具写不存在的light工作量，按真实枚举改simple；幂等回执错误顺序先比hash，改为先检查当前范围，保留越权拒绝断言。
- P3-write-third/fourth：七项通过；后续新增偏好版本、未来撤销、相邻范围及迁移用例。
- P3-write-fifth：10通过1失败；顶层reason和加急reason同名导致flatten反序列化缺字段，改为独立urgentReason并补契约。
- 几次patch上下文匹配失败均在写入前拒绝，读取现格式后应用，没有跳过关键断言。
- P3-rust-first/second：旧schema11与空writeTools断言按真实schema12/十工具更新，原编号/数据/权限断言保留；P3-rust-third及P3-rust-frozen全量116通过。
- P3-rust-delivery：偏好撤销审计补强后，后台静默启动断言发现偶发焦点PID相同。核实所有窗口本已visible:false，Tauri WindowConfig缺省focus:true；后台路径创建前显式同时设visible:false/focus:false，保留不可见和不抢焦点原断言，重新定向与全量验证。
- 浏览器脚本先遇Windows ESM路径、虚拟入口/依赖优化、端口竞争与React开发前导问题；改为明确文件URL、独立缓存/唯一扫描入口、普通自动JSX测试配置。P3-undo-ui-sixth的精确文本定位含字段标签，改为检查可见审阅区域内原值和新值；第七轮真实交互通过，不代表原生WebView验收。一次PowerShell引号解析失败在执行前拒绝，改用固定Python辅助脚本；未执行文件变更或产品操作。
- P3-nsis-first在最后审计/焦点补强前开始，保留日志，但不作为冻结源码最终交付；最终安装包必须另行重新构建。

所有测试使用合成SQLite、独立安全根和真实stdio子进程；不触碰用户真实库/配置/授权。提交前失败测试保留完整日志；尚在进行的验证不能记通过。

## 关键负面用例

同键异请求、跨客户端和限域回执、匿名/暂停/写权限关闭、同字段/ABA及GUI并发冲突、整段替换无明确指令、真实办理未确认、歧义/重复创建、审计失败后业务/字典/版本/回执全回滚、相邻事项越权、撤销现状冲突、未来号码不回收、偏好越权/ABA、旧库迁移前备份。

## 本轮结果

冻结源码自动化验证、本地NSIS和包输入回读均已完成：

| 命令/验证 | 结果与证据 |
|---|---|
| `npm.cmd test -- --maxWorkers=1 --no-file-parallelism` | 退出0，20文件115项通过：[前端测试](evidence/P3-frontend-final.txt) |
| `npm.cmd run build` | 退出0，TypeScript/Vite通过：[前端构建](evidence/P3-build-final.txt) |
| `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` | 退出0：[格式检查](evidence/P3-format-final.txt) |
| `cargo test --locked -j 1 --manifest-path src-tauri/Cargo.toml` | 退出0，108库+2sidecar+6集成，共116项通过：[最终Rust回归](evidence/P3-rust-final.txt) |
| 后台启动定向回归 | 创建前禁用初始焦点后，三次独立运行及最终全量均通过；[首次修复](evidence/P3-background-focus-fix.txt)、[复测1](evidence/P3-background-focus-round-1.txt)、[复测2](evidence/P3-background-focus-round-2.txt)。保留原不可见/不抢焦点/单实例断言 |
| `node scripts/verify-mcp-undo-ui.mjs`（指定Playwright和Edge路径） | 退出0，隔离浏览器审阅、明确批准、冲突保留、独立拒绝通过：[交互证据](evidence/P3-undo-ui-seventh.txt)；截图`release/ui/mcp-p3/undo-review.png` |
| 已安装程序独立回读 | 主程序与sidecar摘要与本轮开始一致：[生产边界](evidence/P3-installed-boundary.txt) |
| `npm.cmd run dist`（`CARGO_BUILD_JOBS=1`） | 退出0，冻结源码最终NSIS成功：[最终构建](evidence/P3-nsis-final.txt) |
| `docs/mcp/verify-package.ps1` | 退出0，版本0.5.0，main及两份sidecar输入均晚于冻结Rust源码；两份sidecar各十工具匿名拒绝/第六次限流/纯stdout通过：[包回读](evidence/P3-package-readback.txt)。两份输入哈希不同，均已分别记录测试；未验证安装后提取哈希 |
| `docs/mcp/verify-phase1-documents.ps1` | 退出0，原三份输入及副本未变、144决定/13字段/版本/固定P0 Tag/本地链接通过：[最终文档检查](evidence/P3-documents-final.txt) |
| `scripts/check-github-actions-runtime.ps1` | 退出0，运行时策略通过：[策略记录](evidence/P3-actions-policy.txt)。本轮没有运行远端Actions |
| `git -c core.safecrlf=false diff --check` | 退出0：[差异检查](evidence/P3-diff-check.txt) |

本地安装包路径：`src-tauri/target/release/bundle/nsis/In Line_0.5.0_x64-setup.exe`，3,925,946字节，SHA256 `8B7CB86CFF400010AD12651140E5D5D096B30E607C449CF5F7F49EF38211D754`。另存到忽略目录`release/In-Line-P3-local-20261010.exe`供后续统一测试；不是GitHub Release或Actions安装包。没有安装本轮包，也没有提交、推送或建立P1/P2/P3验收Tag。

幂等测试同时覆盖重启后回执和四线程同请求，未出现重复事项；审计故障注入复核业务、字典、版本及回执一起回滚。真实stdio集成模拟提交后响应丢失，确认返回result_unknown，并用原键核验，不生成第二事项。性能证据仅限合成夹具和本地测试耗时，尚未证明大规模真实库下的写入延迟；不将本地通过等同实机验收。

## 留待统一验收

P1 Codex自动导入/重载认证仍未通过，入口关闭。P1/P2/P3生产安装、旧数据升级与保留、授权客户端真实操作和软件内批准撤销观感待用户统一验收；没有自动安装或中断用户程序。没有进入P4，队列/复杂子任务/批次/Dry Run/备份工具尚未实现；P5任务/增量/P6健康/P7诊断不冒称提前完成。

后续统一验收建议使用隔离数据，按以下顺序执行，当前均为**未执行**：

1. 安装/升级：旧库自动备份、原事项及永久编号保留、历史和统计一致；升级前后独立核验安装文件及版本。
2. P1接入：目标Codex当前会话自动导入、自动重载、认证和读写授权/范围通过；结果未证明前保持自动入口关闭。
3. P2读取与观感：事项/历史/日历/报表口径和分页一致；更纱黑体下侧栏数字及红色逾期胶囊的原生WebView对齐。
4. P3真实写入：只修改指定字段，重复请求不重复新增，GUI同时编辑出现冲突时明确拒绝；未授权客户端不能写入。
5. P3审计撤销：原值、新值、来源和理由正确；仅提出申请不会撤销，软件内明确批准才执行；冲突保留待处理，无关字段的新修改保留；办理历史和统计变化有据可查。

用户已选择后续统一验收，本轮不要求立即操作；阶段通过、提交/推送、阶段Tag和正式发布继续分别记录授权。
