# In Line 版本发布与测试规范

> **适用项目：In Line｜排着呢**
> **文档类型：开发与发布规范**
> **状态：启用**
> **最后更新：2026-08-28**

## 1. 目的

本规范用于统一 In Line 的：

* 开发测试构建；
* 测试版本发布；
* 正式版本发布；
* 版本号命名；
* GitHub Actions 构建；
* GitHub Releases 管理；
* 应用内检查更新范围。

核心目标是：

> 将“开发测试”“测试版本”“正式版本”明确区分，避免测试安装包污染正式发布记录，同时尽可能由 GitHub 自动完成 Windows 安装包构建。

---

# 2. 发布层级

In Line 使用三级发布机制：

1. 开发构建；
2. 测试版本；
3. 正式版本。

三者用途不同，不得混用。

---

# 3. 开发构建

## 3.1 定位

开发构建用于：

> 功能修改完成后，由开发者自行安装、测试和验收。

开发构建不属于正式的软件版本发布。

---

## 3.2 生成方式

开发构建通过 GitHub Actions 自动生成 Windows x64 NSIS 安装包。

建议支持：

> `workflow_dispatch`

即开发者可以在 GitHub：

> Actions → 对应构建 Workflow → Run workflow

手动触发安装包构建。

必要时也可以在指定分支 Push 后自动运行测试，但不要求每次 Push 都生成可下载安装包。

---

## 3.3 存储方式

开发构建仅作为：

> GitHub Actions Artifact

保存。

不得自动创建 GitHub Release。

不得自动创建 Git Tag。

---

## 3.4 用途

典型流程：

> Codex / 开发者修改代码
> → Push 到 GitHub
> → GitHub Actions 测试
> → 生成 Windows 安装包 Artifact
> → 下载到测试电脑
> → 安装并实际验收

如果验收失败：

> 继续修改代码

不留下正式版本记录。

---

# 4. 测试版本

## 4.1 定位

测试版本用于：

> 功能已经基本完成，但仍需要经过一段实际使用或集中测试后才能进入正式版本。

测试版本发布到 GitHub Releases，但必须标记为：

> **Pre-release**

---

# 5. 测试版本类型

测试版本主要分为：

## Beta

用于功能基本完成后的实际测试。

例如：

`v0.3.0-beta.1`

`v0.3.0-beta.2`

---

## RC

RC 即 Release Candidate，中文可理解为：

> 正式版候选版本

适用于：

* 功能已冻结；
* 不再新增主要功能；
* 重点检查 Bug 和升级兼容性；
* 如果没有重大问题，可直接进入正式版本。

例如：

`v0.3.0-rc.1`

`v0.3.0-rc.2`

---

## Alpha

原则上 In Line 日常开发无需频繁发布 Alpha。

如果未来出现大规模架构重构，可使用：

`v1.0.0-alpha.1`

普通功能迭代优先使用：

> 开发 Artifact → Beta → RC → 正式版

即可。

---

# 6. 正式版本

正式版本用于：

> 面向普通 In Line 用户长期使用。

正式版本不得设置：

> Pre-release

GitHub 应将最新正式版本识别为：

> Latest

例如：

`v0.3.0`

`v0.3.1`

`v0.4.0`

`v1.0.0`

---

# 7. 版本号规范

In Line 原则上遵循 Semantic Versioning 思路：

`MAJOR.MINOR.PATCH`

例如：

`0.3.2`

---

## MAJOR

表示存在：

* 明显的不兼容变化；
* 产品架构重大升级；
* 数据模型或使用方式发生根本变化。

例如：

`1.0.0 → 2.0.0`

---

## MINOR

表示新增：

* 主要功能；
* 新模块；
* 明显的新能力。

但原则上保持现有数据和主要使用方式兼容。

例如：

`0.2.2 → 0.3.0`

---

## PATCH

表示：

* Bug 修复；
* UI 微调；
* 性能优化；
* 小范围功能完善；
* 不改变主要产品能力的改动。

例如：

`0.3.0 → 0.3.1`

---

# 8. 测试版本号规范

测试版本必须基于目标正式版本命名。

例如计划最终发布：

`v0.4.0`

则测试版本可以依次为：

`v0.4.0-beta.1`

`v0.4.0-beta.2`

`v0.4.0-rc.1`

最终：

`v0.4.0`

不要使用：

* `test1`
* `final`
* `final2`
* `正式版`
* `最新版`
* `new`
* `dev-final`

等非标准版本名称。

---

# 9. Git Tag 规范

所有 Release 均通过 Git Tag 触发。

Tag 格式：

正式版：

`v0.3.0`

Beta：

`v0.3.0-beta.1`

RC：

`v0.3.0-rc.1`

Tag 必须以：

> `v`

开头。

---

# 10. 正式版与测试版自动识别

GitHub Actions 应根据 Tag 自动判断 Release 类型。

当 Tag 包含：

* `-alpha`
* `-beta`
* `-rc`

时：

> `prerelease = true`

当 Tag 不包含预发布标识时：

> `prerelease = false`

例如：

`v0.3.0-beta.1`

自动创建：

> GitHub Pre-release

而：

`v0.3.0`

自动创建：

> 正式 GitHub Release

开发者不应每次人工修改 Workflow 中的 `prerelease` 配置。

---

# 11. 版本一致性

正式构建和测试版构建发布前，必须检查以下版本一致：

* Git Tag；
* `package.json`；
* `src-tauri/Cargo.toml`；
* `src-tauri/tauri.conf.json`。

例如：

Tag：

`v0.4.0-beta.1`

则项目中的实际版本策略必须与当前构建机制兼容。

如果项目配置无法使用预发布 SemVer，应在 Workflow 中明确：

> Release Tag 与应用内部基础版本如何映射。

不得出现：

> Tag 是 v0.4.0，但安装程序实际显示 v0.3.2

的情况。

如果版本不一致：

> Workflow 必须失败，禁止创建 Release。

---

# 12. GitHub Actions 发布流程

正式版及测试版统一采用：

> Git Tag
> → GitHub Actions
> → 安装依赖
> → 运行测试
> → 构建 Windows x64 NSIS
> → 创建 GitHub Release
> → 上传 setup.exe

任何测试失败时：

> 不允许继续发布安装包。

---

# 13. 开发构建 Workflow

建议单独提供开发安装包构建入口。

该 Workflow：

* 可以手动运行；
* 不需要 Git Tag；
* 不创建 Release；
* 构建 Windows x64 NSIS 安装包；
* 通过 Artifact 保存；
* Artifact 名称应明确带有开发属性。

例如：

`In-Line-dev-windows-x64`

或者：

`In-Line-test-build`

不要让开发 Artifact 看起来像正式 Release 安装包。

---

# 14. Artifact 保留

开发 Artifact 只是临时测试文件。

无需长期保存。

可以采用 GitHub Actions 默认保留策略或设置合理的短期保留时间。

正式历史版本应通过：

> GitHub Releases

长期保存，而不是依赖 Actions Artifact。

---

# 15. Release 内容

每个正式版和测试版 Release 应至少包含：

* 版本名称；
* 对应 Tag；
* Windows x64 NSIS 安装包；
* 版本说明。

正式版名称建议：

> In Line v0.4.0

测试版：

> In Line v0.4.0-beta.1

---

# 16. Release Notes

正式版本应包含相对完整的 Release Notes。

推荐结构：

* 版本亮点；
* 新增；
* 变更；
* 修复；
* 升级说明；
* 已知问题；
* 下载信息。

Beta / RC 可以适当简化，但应明确：

> 当前属于测试版本，可能存在未发现问题。

---

# 17. CHANGELOG

`CHANGELOG.md` 用于记录长期版本历史。

原则：

* 已发布版本历史不得随意修改；
* 新版本内容加入当前待发布区域；
* 发布时确定版本号和日期；
* 测试版可以记录关键变化，但不需要把每个内部开发构建都写进 CHANGELOG。

开发 Artifact 不属于正式版本，不要求进入 CHANGELOG。

---

# 18. 应用内「检查更新」

In Line 应用内普通“检查更新”只检查：

> 最新正式 Release

不得向普通用户推荐：

* Alpha；
* Beta；
* RC；
* Draft。

因此：

> Pre-release 与正式更新通道必须保持隔离。

---

# 19. 测试版本安装

Beta / RC 主要供：

* 开发者本人；
* 主动参与测试的用户；
* 有明确测试需求的环境；

手动从 GitHub Releases 页面下载安装。

普通用户通过应用内：

> 检查更新

不应收到测试版本提示。

---

# 20. 正式版发布前最低验收要求

正式版发布前至少确认：

* 现有自动化测试通过；
* TypeScript 构建通过；
* Rust 构建通过；
* Tauri Windows 安装包成功生成；
* 软件可以正常安装；
* 原有数据库可以正常打开；
* 升级安装不会主动删除用户数据；
* 主要功能可以正常使用；
* 关于页面版本号正确；
* Release 安装包版本正确；
* MCP sidecar 可以正常安装和启动；
* 已知重大问题已经排除。

如果本版本涉及数据库 migration，还必须测试：

> 旧版本数据库 → 新版本升级

---

# 21. 推荐发布流程

一个典型较大版本：

`开发中`

↓

GitHub Actions Artifact

↓

本地实际测试

↓

`v0.4.0-beta.1`

↓

继续测试和修复

↓

`v0.4.0-beta.2`

↓

功能冻结

↓

`v0.4.0-rc.1`

↓

最终验证

↓

`v0.4.0`

↓

正式 Release / Latest

↓

普通用户通过应用内检查更新获得正式版本。

---

# 22. 小版本流程

如果只是非常小的 Bug 修复，可以简化。

例如：

`v0.4.0`

存在一个小 Bug。

修复后：

> Actions Artifact 验证
> → v0.4.1

可以不经过 Beta 和 RC。

是否需要测试版本，应根据风险决定，而不是机械要求每个 Patch 都走完整流程。

---

# 23. 高风险改动

以下改动原则上至少应经过 Beta 或 RC：

* 数据库结构升级；
* 数据迁移；
* 自动更新机制；
* 备份/恢复机制；
* 删除或永久清理数据；
* 安装程序改动；
* MCP sidecar 改动；
* 大规模状态逻辑调整；
* 事项历史记录处理方式调整。

---

# 24. 分支建议

当前项目如果主要由个人维护，可以继续采用较轻量的：

> `main` 为主要开发与发布分支

不强制建立复杂 Git Flow。

对于较大的功能，可以临时使用：

`feature/...`

分支。

完成后合并回：

`main`

再进入 Artifact / Beta / RC / Release 流程。

---

# 25. 发布权限

原则上：

* Release 由 GitHub Actions 自动生成；
* 不建议频繁手工上传正式安装包；
* 不建议直接修改已经发布的同版本安装包；
* 同一个版本出现重大错误时，应发布新的 Patch，而不是悄悄替换旧安装包。

例如：

不要：

> 替换 v0.4.0 的 EXE，但仍叫 v0.4.0

应改为：

> v0.4.1

这样版本历史和用户问题更容易追踪。

---

# 26. 失败处理

如果 GitHub Actions 构建失败：

> 不创建 Release。

如果 Release 已创建但安装包异常：

应：

1. 停止将该版本推荐给用户；
2. 必要时标记说明；
3. 修复代码；
4. 发布新的版本号。

不要反复覆盖同一个 Tag 下的不同二进制程序。

---

# 27. 推荐最终结构

In Line 的版本发布体系应保持：

```text
开发代码
   │
   ├─ GitHub Actions Artifact
   │    └─ 开发者自行测试
   │
   ├─ vX.Y.Z-beta.N
   │    └─ GitHub Pre-release
   │
   ├─ vX.Y.Z-rc.N
   │    └─ GitHub Pre-release
   │
   └─ vX.Y.Z
        └─ GitHub正式 Release / Latest
             │
             └─ In Line 应用内检查更新
```

---

# 28. 核心原则

整个发布机制遵循以下原则：

> **开发构建不污染 Release。**

> **测试版本不进入正式更新通道。**

> **正式版本必须可以追溯。**

> **安装包尽量由 GitHub Actions 自动生成。**

> **同一个版本号对应唯一确定的发布内容。**

> **高风险功能先测试，再正式发布。**
