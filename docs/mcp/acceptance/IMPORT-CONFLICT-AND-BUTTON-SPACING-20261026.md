# Codex 导入兼容与 MCP 按钮间距修复

日期：2026-10-26（北京时间）。基线：`feat/scheduled-items` / `4d9bb89258c9870fb70dc360b493b42430edf5c6`；本轮为未提交工作区补丁。原有三份交接材料保留。

## 实现

- 导入器仅增加 `default_tools_approval_mode`，按[官方 schema](https://learn.chatgpt.com/docs/config-schema.json)中的 `AppToolApproval` 校验 `auto | prompt | writes | approve`，复制原格式化键和 Item，保留策略值、引号、前置及行尾注释；无字段时不生成默认值。
- 继续拒绝未知字段、非法类型/值、非空 args、禁用服务、其他程序、不安全 env。未改并发检测、锁、原子写入、失败回滚、授权或自动接入门禁。
- MCP 接入回执、客户端操作、存储升级及通用接入 footer 使用动作容器；操作间距 10px，可换行。客户端说明可分行，长文本折行；按钮高度适应文字，标题关闭按钮不挤压。样式限于 MCP 区域，不修改其他设置按钮。

## 本轮验证

- 新增三项 Rust 回归：四种合法值及两种引号、前后注释和其他服务保持；非法值/类型与危险配置拒绝；含审批策略的导入失败后原字节及 ACL 恢复。补丁前新增测试失败，确认可复现；补丁后完整 Rust 套件 **239 项通过**（204 单元、2 CLI 单元、33 集成），无过滤或跳过。单编译任务、单测试线程，D 盘独立临时目录与内存保护。
- 前端 **22 文件 / 133 项通过**，TypeScript 与 Vite build 通过。
- `scripts/verify-mcp-button-spacing.mjs` 使用真实 MCP 组件及 SettingsPanel 的通用弹窗，合成 API；无关设置控件替换为空组件。读取 HEAD 源码的 `--baseline` 对照检查确实拦截 6px 间距。
- 修复版 **126 个布局场景通过**：1280/980/560 逻辑窗口宽度 × 100%/110%/150% 等效缩放 × 常规/150% 按钮字号 × 7 类场景；覆盖高级管理、批量/撤销/任务/故障/维护确认、接入权限、接入回执、通用说明、权限编辑和迁移。实际加载经 hash 校验的更纱黑体 400/600/700。按钮无重叠、横向溢出或文字容纳失败，动作组最小间距 10 CSS px。
- 产品实际使用原生 `set_zoom`；浏览器测试缩小有效 CSS 视口并设置 DPR，不用 CSS zoom。此模拟和截图不等于桌面 WebView 实机验收。
- 既有接入/偏好恢复浏览器回归通过，覆盖敏感权限、全局开启/解除暂停的明确确认、复制失败重试不重复授权、回执重开、脱敏诊断；存储升级浏览器回归通过，覆盖显式确认、来源变化失败、隔离来源拦截。均未调用真实原生写入。
- 三个修改过的 React 组件经 TypeScript AST 对照，事件处理、disabled 与 readOnly 属性和基线一致。Rust fmt、`git diff --check` 通过；验证后产品源码 hash 保持。

## 本地包

本地 NSIS 已构建并解包核验通过。主程序、MCP、CLI 的离线构建身份及版本一致，伴随程序暂存字节一致；解包载荷与构建产物逐字节比对通过（主程序仅包含已知 Tauri NSIS 标记转换）。包内 MCP 的 16 工具名称、完整参数 schema 和协议注解与基线一致；此独立无凭证契约检查只列工具，不查询业务或证明当前会话认证。

- 本地包：`D:\codex-data\projects\inline\In-Line\release\Import-Spacing-local-20261026\In-Line_0.5.0-import-spacing-local_x64-setup.exe`
- SHA-256：`DAC2D052E8A8E98E12D25FE31844C0372F2C8396A030B727A695527A103AF331`
- 本地未提交构建身份：`baselineCommit=4d9bb89258c9870fb70dc360b493b42430edf5c6`、`sourceDirty=true`、`exactCommit=null`。不将其表述为该提交的干净构建或此前 Action 包。
- 完整结果与源文件/三程序摘要：[检查记录](IMPORT-CONFLICT-AND-BUTTON-SPACING-20261026-checks.json)。截图：`release/ui/import-spacing-connection.png`、`release/ui/import-spacing-storage.png`；逐场景测量：`release/Import-Spacing-ui-check.json`。

安装包没有执行；构建前的旧本地包另存于同目录 `before-patch-setup.exe`。本轮没有创建 Action Artifact 或 Release。

## 实机边界

本轮没有读取真实 Codex 配置，也没有读取旧接入包、备份或长期凭证；未修改配置、真实授权或暂停状态，未重试旧命令。未安装、迁移、重新发起接入、提交推送、触发 Action、创建 Tag/Release。

当前会话直接调用挂载 `get_capabilities` 返回 `unauthenticated`，没有身份、权限或范围可供核对。`configurationVerified=false`（本轮未执行真实导入核验）、`currentSessionVerified=false`（当前调用未认证）。本地包检查和合成 sidecar 测试均不能替代此门禁。

仍待用户另行授权及实机执行：安装/升级、旧数据库和偏好保留核对、原生更纱黑体及放大/窄窗口验收、新接入及真实配置验证、当前挂载会话身份/权限/范围与工具集核对。

2026-10-27 后续：用户已明确授权安装，本机三程序与本包逐项核验一致，数据库主/WAL 摘要及授权元数据保留；当前挂载调用为 `Transport closed`，未重试接入。详见[安装记录](IMPORT-SPACING-NATIVE-INSTALL-20261027.md)。上文“未安装”为 2026-10-26 的检查快照。
