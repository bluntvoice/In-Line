# Changelog

本项目的所有重要变更均记录于此。格式参考 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，版本号遵循 [Semantic Versioning](https://semver.org/lang/zh-CN/)。

## [Unreleased]

## [0.0.2] - 2026-07-17

### Added

- 新增持久化人工队列顺位，支持相邻事项上移和下移。
- 新增 1200×800 横版取号卡片，主列表和悬浮窗单击事项即可复制。
- 新增结构化状态历史、加急历史、数据库版本迁移和迁移前自动备份。
- 新增备份列表、安全恢复、系统级 `Ctrl + Alt + N` 快捷键及单实例运行。
- 新增透明叫号铃品牌资源，以及复制、上移、下移三枚统一 SVG / PNG 图标。

### Changed

- 桌面架构由 Electron 迁移至 Tauri 2，保留 React、TypeScript 和 SQLite。
- 队列改为人工顺位优先；加急、优先级和逾期仅作醒目标识，不再自动打乱顺序。
- 行内操作统一为复制、上移、下移，其余操作集中到右键菜单。
- 统一 Windows 中文字体、字号、行高、色板和 Lucide 界面图标。
- 应用定位和默认文案改为通用取号与事项队列工具，不再限定法务场景。
- 开源协议统一为 MIT，并移除 Electron 运行时、旧源码和无效的“300 DPI”描述。

### Fixed

- 修复 125%–200% Windows 缩放下字体错位、中文基线偏移和内容裁切。
- 修复复制图片入口和实现重复的问题，保证一次操作只生成一次图片。
- 修复悬浮窗迷你模式的透明剩余区域遮挡桌面鼠标操作。
- 修复取号和每日备份使用 UTC 日期造成的本地日期边界错误。
- 修复应用窗口、托盘、安装包和快捷方式未使用统一透明图标的问题。

### Security

- 前端不再执行任意 SQL，仅调用带输入长度、枚举和路径校验的受控命令。
- Tauri capability 仅开放实际所需权限；应用默认无遥测、无外部网络访问。
- 备份恢复前执行 SQLite 完整性校验，并先备份当前数据库；日志不记录完整敏感文本。

[Unreleased]: https://github.com/bluntvoice/in-line/compare/v0.0.2...HEAD
[0.0.2]: https://github.com/bluntvoice/in-line/releases/tag/v0.0.2
