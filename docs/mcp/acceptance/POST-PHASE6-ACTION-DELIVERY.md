# Phase6 安装反馈修复：Action 测试包交付

日期：2026-10-23。用户已明确授权提交、推送和运行开发 Action。

## 提交与构建

- 分支：`feat/scheduled-items`。
- 安装器中文显示：`35f072d`；字体/字号恢复：`723bc10`；MCP / CLI 接入弹窗及授权存储保护：`8784d51`。
- 构建完整提交：`8784d51c4f6b1d977250a8fcac20e1b70c98607f`。
- [CI 39913971529](https://github.com/bluntvoice/In-Line/actions/runs/39913971529)：成功。
- [开发 Action 39914041747](https://github.com/bluntvoice/In-Line/actions/runs/39914041747)：成功；测试、NSIS、MCP 契约/匿名认证、三程序及安装包解包、上传步骤均通过。

## 下载

- [GitHub Artifact 12483998522](https://github.com/bluntvoice/In-Line/actions/runs/39914041747/artifacts/12483998522)。保留至 2026-11-06 15:02:36（北京时间）。
- 本地交付文件：`release/Post-Phase6-Action-8784d51-20261023/In-Line_0.5.0-dev-8784d51_x64-setup.exe`。
- 安装包大小：4,332,399 字节。
- 安装包 SHA256：`163088A5E65CB4ADE2BA4C0BCB9BC8954B9659CD7DFD0BF1FED41506A89447AE`。
- Artifact ZIP SHA256：`92131D01EE84E5F3FAFF14899E0EC2B3FDC17182F1660DF8714D2C31C2676ABD`，与 GitHub digest 一致。

本地交付文件仅增加开发提交标识，字节与 Action 安装器一致，软件内部版本仍为开发中的 0.5.0。

## 独立核验

下载并校验归档摘要后，再次解出三个程序，逐一对比 Action 包内 hash 回执；随后执行离线身份、实际 16 工具完整契约和匿名认证拒绝检查。三个程序均为 `sourceDirty=false`、`exactCommit=8784d51...`、`clean_checkout_readback`；不使用此前未提交本地包作为交付包。

- [下载及交付回执](POST-PHASE6-action-receipt.json)
- [Action 实际安装包内容记录](POST-PHASE6-action-payload.json)
- [下载后独立检查](POST-PHASE6-action-independent-check.json)
- [修改范围、备份规则及本地验证](POST-PHASE6-ONBOARDING-AND-UPGRADE.md)

未执行安装器，未替换本机程序或修改现有客户端配置/授权；未创建 Tag 或 GitHub Release。安装文字的真实 DPI 显示、升级后的字体字号、新授权落盘及当前 Codex 会话认证仍须实机验收。授权列表与磁盘不一致的现场根因未标记解决，独立匿名检查不代表当前会话认证成功。
