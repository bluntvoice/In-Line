# In Line 项目协作规则

本文件适用于仓库根目录及全部子目录。执行任务时同时遵守用户机器上的全局 `AGENTS.md`；本文件只补充 In Line 项目约束。

## 1. 产品与文档来源

- 修改功能前，先阅读对应的 `docs/prd/*.md`，不得把 PRD 中“本版本不包含”或“后续可评估”内容当作当前需求实现。
- 功能、安装、更新或发布行为发生变化时，同步检查 `README.md`、`CHANGELOG.md`、`RELEASE_NOTES.md` 和对应 PRD 是否需要更新。
- 当前关键 PRD：
  - `docs/prd/work-calendar.md`：工作日历是基于真实历史的只读回顾视图，不是未来排期工具。
  - `docs/prd/app-update.md`：只有用户主动检查时访问 GitHub；只接受正式 Release，并强制校验 GitHub Asset 的 SHA-256 digest。
  - `docs/prd/release-testing.md`：开发构建、预发布和正式发布必须分层，开发安装包不得污染 Release。

## 2. Git 与任务边界

- 开始前检查当前分支、`git status`、本地 `HEAD` 与 `origin/main`；同步只允许安全快进，存在未提交改动时先说明并保护改动。
- “同步到本地”不包含提交、推送、打 Tag 或创建 Release。
- “提交代码到 GitHub”仅表示检查范围、提交并推送当前分支；不得推断为创建 Tag、GitHub Release 或上传安装包。
- 禁止未经明确授权执行强制推送、硬重置、清理未跟踪文件或覆盖用户改动。

## 3. 版本与发布分层

- 版本号遵循 SemVer。测试 Release 仅使用 `-alpha.N`、`-beta.N`、`-rc.N`；不得使用 `test1`、`final`、`new` 等名称作为 Tag 或 Release 版本。
- 发布前确保以下版本一致：Git Tag、`package.json`、`package-lock.json`、`src-tauri/Cargo.toml`、`src-tauri/Cargo.lock`、`src-tauri/tauri.conf.json`。
- 开发构建：手动运行 `.github/workflows/dev-build.yml`，只生成短期 Actions Artifact，不创建 Tag 或 Release。
- Beta / RC：由标准预发布 Tag 触发 `.github/workflows/release.yml`，必须标记为 GitHub Pre-release，不进入应用内普通更新通道。
- 正式版本：由 `vMAJOR.MINOR.PATCH` Tag 触发，发布为 Latest；应用内检查更新只读取这一正式通道。
- 高风险改动（数据库迁移、更新机制、备份恢复、永久删除、安装器、MCP sidecar）原则上至少经过 Beta 或 RC。
- 同一版本号只能对应一个确定的发布二进制；发现问题时发布新 Patch，不得静默替换既有 Release Asset。

## 4. GitHub Actions 责任

- `.github/workflows/ci.yml`：验证 Push/PR，不生成可下载安装包。
- `.github/workflows/dev-build.yml`：手动生成开发 NSIS Artifact，不创建 Release。
- `.github/workflows/release.yml`：只由 Tag 触发；版本或测试不一致时必须失败；正式资产统一命名为 `In.Line_<version>_x64-setup.exe`，确保应用内更新器能够唯一选择并校验。
- 所有第三方 Actions 必须通过 `scripts/check-github-actions-runtime.ps1` 的运行时策略检查。

## 5. 本地验证

- Windows 命令使用全局规则指定的 PowerShell 7.6.4+ 与 UTF-8；本地 Tauri/NSIS 构建前确认 `cl.exe`、`link.exe` 和 `kernel32.lib` 来自 Visual Studio / Windows SDK。
- 常规代码改动至少运行：
  - `npm.cmd test`
  - `npm.cmd run build`
  - `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
  - `cargo test --locked --manifest-path src-tauri/Cargo.toml`
- 安装、更新、数据库迁移或 MCP 相关改动还应生成 NSIS 安装包，并实际验证安装/升级、旧数据库打开、用户数据保留、MCP sidecar 与安装包版本。
- 未执行的验证必须在交付说明中明确列出，不得把未验证结果描述为通过。

## 6. 发布确认

- 本地构建成功不等于授权发布。
- 提交、推送、Tag、Pre-release、正式 Release 分别属于独立动作；只执行用户明确授权的范围。
- 公开发布前，先向用户提供可编辑的 README / CHANGELOG / Release Notes 更新稿，并确认版本、发布层级和文案后再操作。
