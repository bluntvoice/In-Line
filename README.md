# In Line｜排着呢

In Line 是一款通用的 Windows 本地取号与事项队列工具。它适合个人或内部支持岗位记录任务、展示排队顺序、发送取号卡片，并不限定法务场景。

<img src=".github/assets/readme-icon.png" alt="In Line 图标" width="180">

当前稳定代码版本：`v0.0.2`（2026-07-17）。本次重构的完整变更见 [CHANGELOG.md](CHANGELOG.md)。

## v0.0.2 已实现

- 每日自动取号，跨天显示 A、B … AA 前缀
- 主窗口与桌面悬浮窗共用同一人工队列
- 单击事项即复制 1200×800 取号卡片
- 行内仅保留复制、上移、下移三个操作
- 右键查看、编辑、改状态、加急、完成、归档或移入回收站
- 部门 / 团队、对接人、事项类型、状态、优先级和截止时间
- 本地 SQLite 数据库、结构化状态历史和加急历史
- 迁移前备份、手动备份、备份列表和安全恢复
- 真正的系统级 Ctrl + Alt + N 快捷键
- 系统托盘、开机启动、透明悬浮窗迷你模式
- 透明应用图标和统一 Lucide 界面图标
- 单实例、无遥测、无外部网络、受控 Tauri 命令

## 数据兼容与位置

v0.0.2 会继续使用：

`%APPDATA%\in-line\inline.db`

首次迁移旧数据库时，会先在 `backups` 目录生成 `pre-tauri-migration-时间.db`。应用更新或卸载不会主动删除用户数据库。

### 从旧版升级

1. 正常退出旧版 In Line。
2. 安装并启动 v0.0.2；程序会先备份旧数据库，再在事务中执行迁移。
3. 确认原有事项、号码和日志正常后即可继续使用。

迁移失败时会回滚数据库，不会按新项目清空原有数据。首次升级前仍建议保留一份手动备份。

## 构建产物

Windows x64 安装包采用 NSIS 格式，使用系统 WebView2，不捆绑完整浏览器运行时。源码仓库不提交安装包二进制；正式安装包通过 GitHub Releases 提供。

v0.0.2 本地验收构建结果：

- 安装包：`In Line_0.0.2_x64-setup.exe`
- 文件大小：约 1.97 MB
- SHA-256：`A2979FD3C346DAF084181E637F429E2442DA23FE5DE82EC7E77A32707EC6F989`

## 开发

推荐环境：

- Windows 10 / 11 x64
- Node.js 20+
- Rust stable-msvc
- Visual Studio 2022 Build Tools（Desktop development with C++）
- Microsoft Edge WebView2 Runtime

```powershell
npm.cmd install
npm.cmd test
npm.cmd run build
npm.cmd run dist
```

前端为 React + TypeScript + Vite；桌面层为 Tauri 2 + Rust；数据层为 rusqlite + SQLite。

## 安全和隐私

- 默认不上传数据，不包含遥测或第三方统计。
- 前端不能执行任意 SQL，只能调用白名单命令。
- 日志不记录完整事项详情和内部备注。
- 备份恢复前会校验 SQLite 完整性，并自动备份当前数据。

## 暂未实现

统计中心、完整提醒体系、草稿、自动归档、原生 Excel、多用户、云同步和数据库加密计划在后续版本评估。本版本不将这些功能标记为已完成。

## 许可

本项目采用 GNU General Public License v3.0（`GPL-3.0-only`），详见 [LICENSE](LICENSE)。
