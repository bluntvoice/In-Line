# In-Line MCP 扩展：Phase 0 审查包

日期：2026-10-09。状态：**P0已获用户验收，C1–C4均选A；提交、推送、固定阶段Tag后进入P1**。

## 阅读顺序

1. [三份权威输入](inputs/03-In-Line-MCP-Codex-总执行提示词.md)及其配套 PRD、阶段方案。
2. [本轮现状、冲突和验收报告](acceptance/PHASE-00.md)。
3. [架构草案](architecture.md)与 [API 草案](api-draft.md)。
4. [144 项追踪矩阵](requirements-traceability.md)，另有 [CSV](requirements-traceability.csv)。
5. [P0–P9 阶段计划](phase-plan.md)与 [隔离测试设计](test-plan.md)。
6. [执行检查点与压缩恢复规则](CURRENT_TASK.md)。

所有新增 API、表、模块和测试用例均为**拟建**，不能理解为已经存在。现有测试通过不能替代新增需求验收。本轮只新增文档、输入副本及现有基线测试证据，不改产品功能、版本、数据库或发布流程；未 Commit / Push / Tag / Release。

## 权威输入

输入副本逐字保留原文件，索引及验收口径不自行修改：

- [01：144 项完整需求](inputs/01-In-Line-MCP-PRD-144项完整需求.md)
- [02：分阶段开发与验收](inputs/02-In-Line-MCP-分阶段开发与验收方案.md)
- [03：总执行提示词](inputs/03-In-Line-MCP-Codex-总执行提示词.md)

来源为 `D:/Downloads/` 同名文件；副本校验见 [输入清单](inputs/source-manifest.json)。这些是用户本次明确授权执行的任务输入；历史 PRD 中留存的旧开发提示词仅用于理解现有规则，不授权执行旧任务。

## 冻结门禁

只有用户明确宣布“当前阶段验收通过，可以提交并推送，进入下一阶段”后，才能 Commit → Push → 固定阶段 Tag → 下一阶段。设计冻结或意见讨论本身不授权提交。正式发布另行确认；本轮停止在 P0。
